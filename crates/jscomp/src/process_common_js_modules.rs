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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/NodeUtil.java,
//   src/com/google/javascript/jscomp/ProcessCommonJSModules.java.

//! Rewrites a CommonJS module http://wiki.commonjs.org/wiki/Modules/1.1.1 into a form that can be
//! safely concatenated. Does not add a function around the module body but instead adds suffixes
//! to global variables to avoid conflicts. Calls to require are changed to reference the required
//! module directly.
//!
//! Rust shape (DESIGN.md section 6): the pass keeps no compiler; the Java instance methods that
//! read `compiler` are associated functions taking it. The inner classes `FindImportsAndExports`
//! and `RewriteModule` are their own callbacks. `CompilerInput#getPath` never returns null in this
//! port, so Java's `modulePath == null` early returns cannot happen and are omitted.
use crate::{
    abstract_compiler::AbstractCompiler,
    change_tracker::ChangeTracker,
    check_level::CheckLevel,
    compiler_input::{CompilerInput, ModuleType},
    compiler_pass::CompilerPass,
    deps::module_loader::{ModuleLoader, ModulePath, ResolutionMode, SharedErrorHandler},
    diagnostic_type::DiagnosticType,
    error_handler::ErrorHandler,
    function_to_block_mutator::FunctionToBlockMutator,
    js_error::JSError,
    module_identifier::ModuleIdentifier,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    scope::ScopeId,
    var::VarId,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jsdoc_info::{Builder as JSDocInfoBuilder, JSDocInfo},
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock, Mutex};

// port: ProcessCommonJSModules#UNKNOWN_REQUIRE_ENSURE
pub static UNKNOWN_REQUIRE_ENSURE: DiagnosticType = DiagnosticType::warning(
    "JSC_COMMONJS_UNKNOWN_REQUIRE_ENSURE_ERROR",
    "Unrecognized require.ensure call: {0}",
);

// port: ProcessCommonJSModules#SUSPICIOUS_EXPORTS_ASSIGNMENT
pub static SUSPICIOUS_EXPORTS_ASSIGNMENT: DiagnosticType = DiagnosticType::warning(
    "JSC_COMMONJS_SUSPICIOUS_EXPORTS_ASSIGNMENT",
    "Suspicious re-assignment of \"exports\" variable. Did you actually intend to export something?",
);

// port: ProcessCommonJSModules#EXPORTS
const EXPORTS: &str = "exports";
// port: ProcessCommonJSModules#MODULE
const MODULE: &str = "module";
// port: ProcessCommonJSModules#REQUIRE
const REQUIRE: &str = "require";
// port: ProcessCommonJSModules#WEBPACK_REQUIRE
const WEBPACK_REQUIRE: &str = "__webpack_require__";
// Webpack transpiles import() statements to __webpack_require__.t(modulePath)
// Always imports the module namespace regardless of module type
// port: ProcessCommonJSModules#WEBPACK_REQUIRE_NAMESPACE
const WEBPACK_REQUIRE_NAMESPACE: &str = "__webpack_require__.t";
// port: ProcessCommonJSModules#EXPORT_PROPERTY_NAME
const EXPORT_PROPERTY_NAME: &str = "default";

// port: ProcessCommonJSModules#PROMISE_ALL
static PROMISE_ALL: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("Promise.all"));

// port: ProcessCommonJSModules#DEFINE_AMD
static DEFINE_AMD: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("define.amd"));
// port: ProcessCommonJSModules#WINDOW_DEFINE
static WINDOW_DEFINE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("window.define"));
// port: ProcessCommonJSModules#WINDOW_DEFINE_AMD
static WINDOW_DEFINE_AMD: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("window.define.amd"));
// port: ProcessCommonJSModules#REQUIRE_ENSURE
static REQUIRE_ENSURE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("require.ensure"));
// port: ProcessCommonJSModules#GOOG_PROVIDE
static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));
// port: ProcessCommonJSModules#GOOG_MODULE
static GOOG_MODULE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.module"));
// port: ProcessCommonJSModules#MODULE_EXPORTS
static MODULE_EXPORTS: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("module.exports"));

/// `compiler.getInput(n.getInputId())` (the input object is shared, so a clone is the same input).
fn input_of(compiler: &AbstractCompiler, n: NodeId) -> CompilerInput {
    let input_id = n.get_input_id(compiler);
    compiler
        .get_input(check_not_null!(input_id.as_ref()))
        .expect("CompilerInput")
        .clone()
}

/// `t.getInput()` (the input object is shared, so a clone is the same input).
fn traversal_input(t: &mut NodeTraversal<'_>) -> CompilerInput {
    t.get_input().expect("CompilerInput").clone()
}

/// `t.getInput().getPath()`.
fn traversal_input_path(t: &mut NodeTraversal<'_>) -> ModulePath {
    let input = traversal_input(t);
    input.get_path(t.get_compiler())
}

/// `JSDocInfo.builder().parseDocumentation()` followed by `recordConstancy()` and `build()`.
fn const_jsdoc() -> Option<Arc<JSDocInfo>> {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_constancy();
    builder.build()
}

#[derive(Default)]
pub struct ProcessCommonJSModules;

impl ProcessCommonJSModules {
    /// Creates a new ProcessCommonJSModules instance which can be used to rewrite CommonJS
    /// modules to a concatenable form.
    // port: ProcessCommonJSModules#ProcessCommonJSModules
    pub fn new() -> Self {
        Self
    }

    // port: ProcessCommonJSModules#getModuleName(CompilerInput)
    pub fn get_module_name(compiler: &AbstractCompiler, input: &CompilerInput) -> String {
        let module_path = input.get_path(compiler);
        Self::get_module_name_from_path(&module_path)
    }

    // port: ProcessCommonJSModules#getModuleName(ModulePath)
    pub fn get_module_name_from_path(input: &ModulePath) -> String {
        input.to_module_name()
    }

    // port: ProcessCommonJSModules#getBasePropertyImport(String,Node)
    fn get_base_property_import_for_call(
        compiler: &AbstractCompiler,
        module_name: &str,
        require_call: NodeId,
    ) -> String {
        check_argument!(require_call.is_call(compiler));
        if compiler.get_options().get_module_resolution_mode() == ResolutionMode::WEBPACK
            && require_call
                .get_first_child(compiler)
                .unwrap()
                .matches_qualified_name(compiler, WEBPACK_REQUIRE_NAMESPACE)
        {
            return module_name.to_string();
        }

        Self::get_base_property_import(compiler, module_name)
    }

    // port: ProcessCommonJSModules#getBasePropertyImport(String)
    pub fn get_base_property_import(compiler: &AbstractCompiler, module_name: &str) -> String {
        let module_type = compiler.get_module_type_by_name(module_name);
        if module_type.is_some_and(|module_type| module_type != ModuleType::COMMONJS) {
            return module_name.to_string();
        }

        format!("{module_name}.{EXPORT_PROPERTY_NAME}")
    }

    // port: ProcessCommonJSModules#isCommonJsImport(Node)
    pub fn is_common_js_import_with_options(
        compiler: &AbstractCompiler,
        require_call: NodeId,
    ) -> bool {
        Self::is_common_js_import(
            compiler,
            require_call,
            compiler.get_options().get_module_resolution_mode(),
        )
    }

    /// Recognize if a node is a module import. We recognize two forms:
    ///
    /// - require("something");
    /// - __webpack_require__(4); // only when the module resolution is WEBPACK
    /// - __webpack_require__.t(4); // only when the module resolution is WEBPACK
    // port: ProcessCommonJSModules#isCommonJsImport(Node, ResolutionMode)
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    pub fn is_common_js_import(
        ast: &Ast,
        require_call: NodeId,
        resolution_mode: ResolutionMode,
    ) -> bool {
        if require_call.is_call(ast) && require_call.has_two_children(ast) {
            let first = require_call.get_first_child(ast).unwrap();
            let second = require_call.get_second_child(ast).unwrap();
            if resolution_mode == ResolutionMode::WEBPACK
                && (first.matches_qualified_name(ast, WEBPACK_REQUIRE)
                    || first.matches_qualified_name(ast, WEBPACK_REQUIRE_NAMESPACE))
                && (second.is_number(ast) || second.is_string_lit(ast))
            {
                return true;
            } else if first.matches_qualified_name(ast, REQUIRE) && second.is_string_lit(ast) {
                return true;
            }
        } else if require_call.is_call(ast)
            && require_call.has_x_children(ast, 3)
            && resolution_mode == ResolutionMode::WEBPACK
            && require_call
                .get_first_child(ast)
                .unwrap()
                .matches_qualified_name(ast, format!("{WEBPACK_REQUIRE}.bind"))
            && require_call.get_second_child(ast).unwrap().is_null(ast)
            && (require_call.get_last_child(ast).unwrap().is_number(ast)
                || require_call.get_last_child(ast).unwrap().is_string_lit(ast))
        {
            return true;
        }
        false
    }

    // port: ProcessCommonJSModules#getCommonJsImportPath(Node)
    pub fn get_common_js_import_path_with_options(
        compiler: &AbstractCompiler,
        require_call: NodeId,
    ) -> JsString {
        Self::get_common_js_import_path(
            compiler,
            require_call,
            compiler.get_options().get_module_resolution_mode(),
        )
    }

    // port: ProcessCommonJSModules#getCommonJsImportPath(Node, ResolutionMode)
    pub fn get_common_js_import_path(
        ast: &Ast,
        require_call: NodeId,
        resolution_mode: ResolutionMode,
    ) -> JsString {
        if resolution_mode == ResolutionMode::WEBPACK {
            let path_argument = if require_call.get_child_count(ast) >= 3 {
                require_call.get_child_at_index(ast, 2).unwrap()
            } else {
                require_call.get_second_child(ast).unwrap()
            };
            if path_argument.is_number(ast) {
                return JsString::from((path_argument.get_double(ast) as i32).to_string());
            } else {
                return path_argument.get_string(ast);
            }
        }

        require_call.get_second_child(ast).unwrap().get_string(ast)
    }

    // port: ProcessCommonJSModules#getImportedModuleName(NodeTraversal,Node)
    fn get_imported_module_name(t: &mut NodeTraversal<'_>, require_call: NodeId) -> String {
        let import_path =
            Self::get_common_js_import_path_with_options(t.get_compiler(), require_call);
        Self::get_imported_module_name_for_path(t, require_call, &import_path.to_string())
    }

    // port: ProcessCommonJSModules#getImportedModuleName(NodeTraversal,Node,String)
    fn get_imported_module_name_for_path(
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        import_path: &str,
    ) -> String {
        let source_file_name = n.get_source_file_name(t);
        let lineno = n.get_lineno(t);
        let charno = n.get_charno(t);
        let module_path = traversal_input_path(t).resolve_js_module(
            import_path,
            source_file_name.as_deref(),
            lineno,
            charno,
        );
        // The module loader reported to the compiler (its ErrorHandler) during the resolution.
        t.get_compiler().flush_module_errors();

        let Some(module_path) = module_path else {
            return ModuleIdentifier::for_file(import_path)
                .module_name()
                .to_string();
        };
        module_path.to_module_name()
    }

    /// Recognize if a node is a module export. We recognize several forms:
    ///
    /// - module.exports = something;
    /// - module.exports.something = something;
    /// - exports.something = something;
    ///
    /// In addition, we only recognize an export if the base export object is not defined or is
    /// defined in externs.
    // port: ProcessCommonJSModules#isCommonJsExport(NodeTraversal, Node, ResolutionMode)
    pub fn is_common_js_export(
        t: &mut NodeTraversal<'_>,
        export: NodeId,
        _resolution_mode: ResolutionMode,
    ) -> bool {
        if export.matches_qualified_name(t, format!("{MODULE}.{EXPORTS}"))
            || (export.is_get_elem(t)
                && export
                    .get_first_child(t)
                    .unwrap()
                    .matches_qualified_name(t, MODULE)
                && export.get_second_child(t).unwrap().is_string_lit(t)
                && export.get_second_child(t).unwrap().get_string(t) == EXPORTS)
        {
            let scope = t.get_scope();
            let v = scope.get_var(t.get_compiler(), MODULE);
            if v.is_none_or(|v| v.is_extern(t.get_compiler())) {
                return true;
            }
        } else if export.is_name(t) && export.get_string(t) == EXPORTS {
            let scope = t.get_scope();
            let name = export.get_string(t);
            let v = scope.get_var(t.get_compiler(), name);
            if v.is_none_or(|v| v.is_global(t.get_compiler())) {
                return true;
            }
        }
        false
    }

    // port: ProcessCommonJSModules#isCommonJsExport(NodeTraversal,Node)
    fn is_common_js_export_with_options(t: &mut NodeTraversal<'_>, export: NodeId) -> bool {
        let resolution_mode = t.get_compiler().get_options().get_module_resolution_mode();
        Self::is_common_js_export(t, export, resolution_mode)
    }

    /// Recognize if a node is a dynamic module import. Currently only the webpack dynamic import
    /// is recognized:
    ///
    /// - __webpack_require__.e(0).then(function() { return __webpack_require__(4);})
    /// - Promise.all([__webpack_require__.e(0)]).then(function() { return
    ///   __webpack_require__(4);})
    // port: ProcessCommonJSModules#isCommonJsDynamicImportCallback
    pub fn is_common_js_dynamic_import_callback(
        ast: &Ast,
        n: Option<NodeId>,
        resolution_mode: ResolutionMode,
    ) -> bool {
        let Some(n) = n else {
            return false;
        };
        if resolution_mode != ResolutionMode::WEBPACK {
            return false;
        }
        if n.is_function(ast) && Self::is_webpack_require_ensure_callback(ast, n) {
            return true;
        }

        false
    }

    /// Recognizes __webpack_require__ calls that are the .then callback of a
    /// __webpack_require__.e call. Example:
    ///
    /// - __webpack_require__.e(0).then(function() { return __webpack_require__(4); })
    /// - Promise.all([__webpack_require__.e(0)]).then(function() { return
    ///   __webpack_require__(4);})
    // port: ProcessCommonJSModules#isWebpackRequireEnsureCallback
    fn is_webpack_require_ensure_callback(ast: &Ast, fnc: NodeId) -> bool {
        check_argument!(fnc.is_function(ast));
        let Some(call_parent) = fnc.get_parent(ast) else {
            return false;
        };

        if !call_parent.is_call(ast) {
            return false;
        }

        if !(call_parent.has_children(ast)
            && call_parent.get_first_child(ast).unwrap().is_get_prop(ast)
            && call_parent.get_first_first_child(ast).unwrap().is_call(ast))
        {
            return false;
        }
        let call_parent_target = call_parent
            .get_first_first_child(ast)
            .unwrap()
            .get_first_child(ast)
            .unwrap();

        if call_parent_target.matches_qualified_name(ast, format!("{WEBPACK_REQUIRE}.e"))
            && call_parent
                .get_first_child(ast)
                .unwrap()
                .get_string_ref(ast)
                == "then"
        {
            return true;
        } else if PROMISE_ALL.matches(ast, call_parent_target)
            && call_parent_target.get_next(ast).is_some()
            && call_parent_target.get_next(ast).unwrap().is_array_lit(ast)
        {
            let mut all_elements_are_dynamic_imports = false;
            let mut array_item = call_parent_target
                .get_next(ast)
                .unwrap()
                .get_first_child(ast);
            while let Some(item) = array_item {
                if !(item.is_call(ast)
                    && item.has_two_children(ast)
                    && item
                        .get_first_child(ast)
                        .unwrap()
                        .matches_qualified_name(ast, format!("{WEBPACK_REQUIRE}.e")))
                {
                    return false;
                }
                all_elements_are_dynamic_imports = true;
                array_item = item.get_next(ast);
            }
            return all_elements_are_dynamic_imports;
        }
        false
    }

    // port: ProcessCommonJSModules#getBaseQualifiedNameNode
    fn get_base_qualified_name_node(ast: &Ast, n: NodeId) -> NodeId {
        let mut ref_parent = n;
        while ref_parent.has_parent(ast)
            && ref_parent.get_parent(ast).unwrap().is_qualified_name(ast)
        {
            ref_parent = ref_parent.get_parent(ast).unwrap();
        }

        ref_parent
    }

    /// UMD modules are often wrapped in an IIFE for cases where they are used as scripts instead
    /// of modules. Remove the wrapper.
    ///
    /// Returns whether an IIFE wrapper was found and removed.
    // port: ProcessCommonJSModules#removeIIFEWrapper
    fn remove_iife_wrapper(compiler: &mut AbstractCompiler, root: NodeId) -> bool {
        check_state!(root.is_script(compiler));
        let mut n = root.get_first_child(compiler);

        // Sometimes scripts start with a semicolon for easy concatenation.
        // Skip any empty statements from those
        while n.is_some_and(|n| n.is_empty(compiler)) {
            n = n.unwrap().get_next(compiler);
        }

        // An IIFE wrapper must be the only non-empty statement in the script,
        // and it must be an expression statement.
        let Some(mut n) = n else {
            return false;
        };
        if !n.is_expr_result(compiler) || n.get_next(compiler).is_some() {
            return false;
        }

        // Function expression can be forced with !, just skip !
        // TODO(ChadKillingsworth):
        //   Expression could also be forced with: + - ~ void
        //   ! ~ void can be repeated any number of times
        if n.has_children(compiler) && n.get_first_child(compiler).unwrap().is_not(compiler) {
            n = n.get_first_child(compiler).unwrap();
        }

        let call = n.get_first_child(compiler);
        let Some(call) = call.filter(|call| call.is_call(compiler)) else {
            return false;
        };

        // Find the IIFE call and function nodes
        let fnc;
        if call
            .get_first_child(compiler)
            .unwrap()
            .is_function(compiler)
        {
            fnc = n.get_first_first_child(compiler).unwrap();
        } else if call
            .get_first_child(compiler)
            .unwrap()
            .is_get_prop(compiler)
            && call
                .get_first_first_child(compiler)
                .unwrap()
                .is_function(compiler)
            && call
                .get_first_child(compiler)
                .unwrap()
                .get_string_ref(compiler)
                == "call"
        {
            fnc = call.get_first_first_child(compiler).unwrap();

            // We only support explicitly binding "this" to the parent "this" or "exports"
            if !(call.get_second_child(compiler).is_some()
                && (call.get_second_child(compiler).unwrap().is_this(compiler)
                    || call
                        .get_second_child(compiler)
                        .unwrap()
                        .matches_qualified_name(compiler, EXPORTS)))
            {
                return false;
            }
        } else {
            return false;
        }

        if NodeUtil::does_function_reference_own_arguments_object(compiler, fnc) {
            return false;
        }

        let ci = input_of(compiler, root);
        let module_path = ci.get_path(compiler);

        let iife_label = format!(
            "{}_iifeWrapper",
            Self::get_module_name_from_path(&module_path)
        );

        let mutator = FunctionToBlockMutator::new(compiler, compiler.get_unique_name_id_supplier());
        let block = mutator.mutate_without_renaming(
            compiler,
            &JsString::from(iife_label),
            fnc,
            call,
            None,
            false,
            false,
        );
        root.remove_children(compiler);
        let children = block.remove_children(compiler);
        root.add_children_to_front(compiler, children);
        Self::report_nested_scopes_deleted(compiler, fnc);
        compiler.report_change_to_enclosing_scope(root);

        true
    }

    /// For AMD wrappers, webpack adds a shim for the "module" variable. We need that to be a free
    /// var so we remove the shim.
    // port: ProcessCommonJSModules#removeWebpackModuleShim
    fn remove_webpack_module_shim(compiler: &mut AbstractCompiler, root: NodeId) {
        check_state!(root.is_script(compiler));
        let mut n = root.get_first_child(compiler);

        // Sometimes scripts start with a semicolon for easy concatenation.
        // Skip any empty statements from those
        while n.is_some_and(|n| n.is_empty(compiler)) {
            n = n.unwrap().get_next(compiler);
        }

        // An IIFE wrapper must be the only non-empty statement in the script,
        // and it must be an expression statement.
        let Some(n) = n else {
            return;
        };
        if !n.is_expr_result(compiler) || n.get_next(compiler).is_some() {
            return;
        }

        let call = n.get_first_child(compiler);
        let Some(call) = call.filter(|call| call.is_call(compiler)) else {
            return;
        };

        // Find the IIFE call and function nodes

        let call_target = call.get_first_child(compiler).unwrap();
        if !call_target.is_function(compiler) {
            return;
        }

        let fnc = call_target;

        let params = NodeUtil::get_function_parameters(compiler, fnc);
        let mut module_param = None;
        let mut param = params.get_first_child(compiler);
        let mut param_number = 0;
        while let Some(p) = param {
            param_number += 1;
            if p.is_name(compiler) && p.get_string(compiler) == MODULE {
                module_param = Some(p);
                break;
            }
            param = p.get_next(compiler);
        }
        if module_param.is_none() {
            return;
        }
        let param = param.unwrap();

        let is_free_call = call.get_boolean_prop(compiler, Prop::FREE_CALL);
        let arg = call.get_child_at_index(
            compiler,
            if is_free_call {
                param_number
            } else {
                param_number + 1
            },
        );
        let Some(arg) = arg else {
            return;
        };

        let arg_call_target = arg.get_first_child(compiler);
        if arg.is_call(compiler)
            && arg_call_target.unwrap().is_call(compiler)
            && Self::is_common_js_import_with_options(compiler, arg_call_target.unwrap())
            && arg_call_target
                .unwrap()
                .get_next(compiler)
                .unwrap()
                .matches_name(compiler, MODULE)
        {
            let arg_call_target = arg_call_target.unwrap();
            let import_path =
                Self::get_common_js_import_path_with_options(compiler, arg_call_target);

            let source_file_name = arg.get_source_file_name(compiler);
            let module_path = input_of(compiler, root)
                .get_path(compiler)
                .resolve_js_module(
                    &import_path.to_string(),
                    source_file_name.as_deref(),
                    arg.get_lineno(compiler),
                    arg.get_charno(compiler),
                );
            // The module loader reported to the compiler (its ErrorHandler) during the resolution.
            compiler.flush_module_errors();
            let Some(module_path) = module_path else {
                // The module loader will issue an error
                return;
            };

            if module_path.to_string().contains("/buildin/module.js") {
                arg.detach(compiler);
                param.detach(compiler);
                compiler.report_change_to_change_scope(fnc);
                compiler.report_change_to_enclosing_scope(fnc);
            }
        }
    }

    // port: ProcessCommonJSModules#reportNestedScopesDeleted
    fn report_nested_scopes_deleted(compiler: &mut AbstractCompiler, n: NodeId) {
        let mut functions = Vec::new();
        NodeUtil::visit_pre_order(compiler, n, &mut |ast: &mut Ast, n: NodeId| {
            if n.is_function(ast) {
                functions.push(n);
            }
        });
        // The visitor only reads the tree; the compiler reports each function in visit order.
        for n in functions {
            compiler.report_function_deleted(n);
        }
    }

    // port: ProcessCommonJSModules#reportNestedScopesChanged
    fn report_nested_scopes_changed(compiler: &mut AbstractCompiler, n: NodeId) {
        let mut functions = Vec::new();
        NodeUtil::visit_pre_order(compiler, n, &mut |ast: &mut Ast, n: NodeId| {
            if n.is_function(ast) {
                functions.push(n);
            }
        });
        // The visitor only reads the tree; the compiler reports each function in visit order.
        for n in functions {
            compiler.report_change_to_change_scope(n);
        }
    }

    // port: ProcessCommonJSModules#findUmdPattern
    fn find_umd_pattern(umd_patterns: &[UmdPattern], n: NodeId) -> Option<UmdPattern> {
        umd_patterns.iter().find(|umd| umd.if_root == n).copied()
    }
}

impl CompilerPass for ProcessCommonJSModules {
    // port: ProcessCommonJSModules#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        NodeTraversal::traverse(compiler, externs, self);
    }
}

impl Callback for ProcessCommonJSModules {
    // port: ProcessCommonJSModules#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_root(t) {
            return true;
        } else if n.is_script(t) {
            let compiler = t.get_compiler();
            if compiler.get_options().get_module_resolution_mode() == ResolutionMode::WEBPACK {
                Self::remove_webpack_module_shim(compiler, n);
            }

            let mut finder = FindImportsAndExports::new();
            let module_loader_error_handler = compiler.get_module_loader().get_error_handler();
            compiler
                .get_module_loader()
                .set_error_handler(Some(finder.error_handler()));
            NodeTraversal::traverse(compiler, n, &mut finder);

            let module_type = input_of(compiler, n).get_js_module_type();

            let force_module_detection = module_type == ModuleType::IMPORTED_SCRIPT;
            let mut default_export_is_const = true;

            let is_common_js_module = finder.is_common_js_module();
            let mut exports = Vec::new();
            if is_common_js_module || force_module_detection {
                if !finder.umd_patterns.is_empty() {
                    let mut needs_retraverse = false;
                    if finder.replace_umd_patterns(compiler) {
                        needs_retraverse = true;
                    }
                    // Removing the IIFE rewrites vars. We need to re-traverse
                    // to get the new references.
                    if Self::remove_iife_wrapper(compiler, n) {
                        needs_retraverse = true;
                    }

                    if needs_retraverse {
                        finder = FindImportsAndExports::new();
                        compiler
                            .get_module_loader()
                            .set_error_handler(Some(finder.error_handler()));
                        NodeTraversal::traverse(compiler, n, &mut finder);
                    }
                }

                default_export_is_const = finder.initialize_module(compiler);

                exports.extend(finder.get_module_exports());
                exports.extend(finder.get_exports());
            }
            finder.report_module_errors(compiler);
            compiler
                .get_module_loader()
                .set_error_handler(Some(module_loader_error_handler));

            NodeTraversal::traverse(
                compiler,
                n,
                &mut RewriteModule::new(
                    is_common_js_module || force_module_detection,
                    exports,
                    default_export_is_const,
                ),
            );
        }
        false
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

/// Information on a Universal Module Definition A UMD is an IF statement and a reference to which
/// branch contains the commonjs export
#[derive(Clone, Copy)]
struct UmdPattern {
    if_root: NodeId,
    active_branch: Option<NodeId>,
}

impl UmdPattern {
    // port: ProcessCommonJSModules.UmdPattern#UmdPattern
    fn new(if_root: NodeId, active_branch: Option<NodeId>) -> Self {
        Self {
            if_root,
            active_branch,
        }
    }
}

/// Java compares ExportInfo objects by identity; each one is created for a distinct node of its
/// list, so comparing all fields finds the same object.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ExportInfo {
    node: NodeId,
    scope: ScopeId,
    is_in_supported_scope: bool,
}

impl ExportInfo {
    // port: ProcessCommonJSModules.ExportInfo#ExportInfo
    fn new(ast: &Ast, node: NodeId, scope: ScopeId) -> Self {
        let disqualifying_parent = NodeUtil::get_enclosing_node(ast, node, &|ast, n| {
            n.is_if(ast) || n.is_hook(ast) || n.is_function(ast) || n.is_arrow_function(ast)
        });
        Self {
            node,
            scope,
            is_in_supported_scope: disqualifying_parent.is_none(),
        }
    }
}

/// The `ErrorHandler` half of FindImportsAndExports: the module loader keeps a shared handle, so
/// the `errors` list is shared with the finder.
struct FinderErrorHandler {
    errors: Arc<Mutex<Vec<JSError>>>,
}

impl ErrorHandler for FinderErrorHandler {
    // port: ProcessCommonJSModules.FindImportsAndExports#report
    fn report(&mut self, _ignored_level: CheckLevel, error: JSError) {
        self.errors.lock().unwrap().push(error);
    }
}

/// Traverse the script. Find all references to CommonJS require (import) and module.exports or
/// export statements. Rewrites any require calls to reference the rewritten module name.
struct FindImportsAndExports {
    has_goog_provide_or_module: bool,
    script: Option<NodeId>,
    umd_patterns: Vec<UmdPattern>,
    module_exports: Vec<ExportInfo>,
    exports: Vec<ExportInfo>,
    errors: Arc<Mutex<Vec<JSError>>>,
}

struct UmdTestInfo {
    enclosing_if: NodeId,
    active_branch: NodeId,
}

impl UmdTestInfo {
    // port: ProcessCommonJSModules.FindImportsAndExports.UmdTestInfo#UmdTestInfo
    fn new(enclosing_if: NodeId, active_branch: NodeId) -> Self {
        Self {
            enclosing_if,
            active_branch,
        }
    }
}

impl FindImportsAndExports {
    fn new() -> Self {
        Self {
            has_goog_provide_or_module: false,
            script: None,
            umd_patterns: Vec::new(),
            module_exports: Vec::new(),
            exports: Vec::new(),
            errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The finder as the module loader's ErrorHandler.
    fn error_handler(&self) -> SharedErrorHandler {
        Arc::new(Mutex::new(FinderErrorHandler {
            errors: self.errors.clone(),
        }))
    }

    // port: ProcessCommonJSModules.FindImportsAndExports#isCommonJsModule
    fn is_common_js_module(&self) -> bool {
        (!self.exports.is_empty() || !self.module_exports.is_empty())
            && !self.has_goog_provide_or_module
    }

    // port: ProcessCommonJSModules.FindImportsAndExports#getModuleExports
    fn get_module_exports(&self) -> Vec<ExportInfo> {
        self.module_exports.clone()
    }

    // port: ProcessCommonJSModules.FindImportsAndExports#getExports
    fn get_exports(&self) -> Vec<ExportInfo> {
        self.exports.clone()
    }
}

impl Callback for FindImportsAndExports {
    // port: ProcessCommonJSModules.FindImportsAndExports#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(node_traversal) {
            check_state!(self.script.is_none());
            self.script = Some(n);
        }
        true
    }

    // port: ProcessCommonJSModules.FindImportsAndExports#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // Check for goog.provide or goog.module statements
        if NodeUtil::is_shallow_statement_tree(t, parent) && n.is_expr_result(t) {
            let maybe_get_prop = n.get_first_first_child(t);
            if let Some(maybe_get_prop) = maybe_get_prop
                && (GOOG_PROVIDE.matches(t, maybe_get_prop)
                    || GOOG_MODULE.matches(t, maybe_get_prop))
            {
                self.has_goog_provide_or_module = true;
            }
        }

        // Find require.ensure calls
        if n.is_call(t) && REQUIRE_ENSURE.matches(t, n.get_first_child(t).unwrap()) {
            self.visit_require_ensure_call(t, n);
        }

        if n.matches_qualified_name(t, format!("{MODULE}.{EXPORTS}"))
            || (n.is_get_elem(t)
                && n.get_first_child(t)
                    .unwrap()
                    .matches_qualified_name(t, MODULE)
                && n.get_second_child(t).unwrap().is_string_lit(t)
                && n.get_second_child(t).unwrap().get_string(t) == EXPORTS)
        {
            if ProcessCommonJSModules::is_common_js_export_with_options(t, n) {
                let scope = t.get_scope();
                self.module_exports.push(ExportInfo::new(t, n, scope));

                // If the module.exports statement is nested in an if statement,
                // and the test of the if checks for "module" or "define,
                // assume the if statement is an UMD pattern with a common js export in the then
                // branch
                let umd_test_ancestor = self.get_outermost_umd_test(t, parent);
                if let Some(umd_test_ancestor) = umd_test_ancestor
                    && !self.is_in_if_test(t, Some(n))
                {
                    let existing_pattern = ProcessCommonJSModules::find_umd_pattern(
                        &self.umd_patterns,
                        umd_test_ancestor.enclosing_if,
                    );
                    if existing_pattern.is_none() {
                        self.umd_patterns.push(UmdPattern::new(
                            umd_test_ancestor.enclosing_if,
                            Some(umd_test_ancestor.active_branch),
                        ));
                    }
                }
            }
        } else if DEFINE_AMD.matches(t, n) || WINDOW_DEFINE_AMD.matches(t, n) {
            // If a define.amd statement is in the test of an if statement,
            // and the statement has no "else" block, we simply want to remove
            // the entire if statement.
            let umd_test_ancestor = self.get_outermost_umd_test(t, parent);
            if let Some(umd_test_ancestor) = umd_test_ancestor
                && self.is_in_if_test(t, Some(n))
            {
                let existing_pattern = ProcessCommonJSModules::find_umd_pattern(
                    &self.umd_patterns,
                    umd_test_ancestor.enclosing_if,
                );
                if existing_pattern.is_none()
                    && umd_test_ancestor
                        .enclosing_if
                        .get_child_at_index(t, 2)
                        .is_none()
                {
                    self.umd_patterns
                        .push(UmdPattern::new(umd_test_ancestor.enclosing_if, None));
                }
            }
        }

        if n.is_name(t) && n.get_string(t) == EXPORTS {
            let scope = t.get_scope();
            let v = scope.get_var(t.get_compiler(), EXPORTS);
            if v.is_none_or(|v| v.is_global(t.get_compiler())) {
                let q_name_root = ProcessCommonJSModules::get_base_qualified_name_node(t, n);
                if q_name_root.matches_qualified_name(t, EXPORTS)
                    && NodeUtil::is_l_value(t, q_name_root)
                {
                    let next = n.get_next(t);
                    // Match the special assignment
                    // exports = module.exports
                    if n.get_grandparent(t).unwrap().is_expr_result(t)
                        && next.is_some()
                        && ((next.unwrap().is_get_prop(t)
                            && next
                                .unwrap()
                                .matches_qualified_name(t, format!("{MODULE}.{EXPORTS}")))
                            || (next.unwrap().is_assign(t)
                                && next
                                    .unwrap()
                                    .get_first_child(t)
                                    .unwrap()
                                    .matches_qualified_name(t, format!("{MODULE}.{EXPORTS}"))))
                    {
                        let scope = t.get_scope();
                        self.exports.push(ExportInfo::new(t, n, scope));

                        // Ignore inlining created identity vars
                        // var exports = exports
                    } else if !self.has_goog_provide_or_module
                        && (v.is_none() || {
                            let v = v.unwrap();
                            // Java dereferences the null name node here (a
                            // NullPointerException); `unwrap` keeps that.
                            v.get_name_node(t.get_compiler()).is_none()
                                && v.get_name_node(t.get_compiler())
                                    .unwrap()
                                    .get_first_child(t)
                                    != Some(n)
                        })
                    {
                        self.errors.lock().unwrap().push(JSError::make(
                            t,
                            q_name_root,
                            &SUSPICIOUS_EXPORTS_ASSIGNMENT,
                            &[],
                        ));
                    }
                } else {
                    let scope = t.get_scope();
                    self.exports.push(ExportInfo::new(t, n, scope));

                    // If the exports statement is nested in the then branch of an if statement,
                    // and the test of the if checks for "module" or "define,
                    // assume the if statement is an UMD pattern with a common js export in the
                    // then branch
                    let umd_test_ancestor = self.get_outermost_umd_test(t, parent);
                    if let Some(umd_test_ancestor) = umd_test_ancestor
                        && !self.is_in_if_test(t, Some(n))
                    {
                        let existing_pattern = ProcessCommonJSModules::find_umd_pattern(
                            &self.umd_patterns,
                            umd_test_ancestor.enclosing_if,
                        );
                        if existing_pattern.is_none() {
                            self.umd_patterns.push(UmdPattern::new(
                                umd_test_ancestor.enclosing_if,
                                Some(umd_test_ancestor.active_branch),
                            ));
                        }
                    }
                }
            }
        } else if n.is_this(t) && n.get_parent(t).unwrap().is_get_prop(t) && t.in_global_scope() {
            let scope = t.get_scope();
            self.exports.push(ExportInfo::new(t, n, scope));
        }

        if ProcessCommonJSModules::is_common_js_import_with_options(t.get_compiler(), n) {
            self.visit_require_call(t, n, parent.unwrap());
        }
    }
}

impl FindImportsAndExports {
    /// Visit require calls.
    // port: ProcessCommonJSModules.FindImportsAndExports#visitRequireCall
    fn visit_require_call(&mut self, t: &mut NodeTraversal<'_>, require: NodeId, parent: NodeId) {
        // When require("name") is used as a standalone statement (the result isn't used)
        // it indicates that a module is being loaded for the side effects it produces.
        // In this case the require statement should just be removed as the dependency
        // sorting will insert the file for us.
        if !NodeUtil::is_expression_result_used(t, require)
            && parent.is_expr_result(t)
            && NodeUtil::is_statement_block(t, parent.get_parent(t).unwrap())
        {
            // Attempt to resolve the module so that load warnings are issued
            let import_path = ProcessCommonJSModules::get_common_js_import_path_with_options(
                t.get_compiler(),
                require,
            );
            let source_file_name = require.get_source_file_name(t);
            let lineno = require.get_lineno(t);
            let charno = require.get_charno(t);
            traversal_input_path(t).resolve_js_module(
                &import_path.to_string(),
                source_file_name.as_deref(),
                lineno,
                charno,
            );
            // The module loader reported to the compiler (its ErrorHandler) during the resolution.
            t.get_compiler().flush_module_errors();
            let grandparent = parent.get_parent(t).unwrap();
            parent.detach(t);
            t.get_compiler()
                .report_change_to_enclosing_scope(grandparent);
        }
    }

    /// Visit require.ensure calls. Replace the call with an IIFE. Require.ensure must always be
    /// of the form:
    ///
    /// require.ensure(['module1', ...], function(require) {})
    // port: ProcessCommonJSModules.FindImportsAndExports#visitRequireEnsureCall
    fn visit_require_ensure_call(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        if !call.has_x_children(t, 3) {
            let error = JSError::make(
                t,
                call,
                &UNKNOWN_REQUIRE_ENSURE,
                &[
                    "Expected the function to have 2 arguments but instead found {0}",
                    &call.get_child_count(t).to_string(),
                ],
            );
            t.get_compiler().report(error);
            return;
        }

        let dependencies = call.get_second_child(t).unwrap();
        if !dependencies.is_array_lit(t) {
            let error = JSError::make(
                t,
                dependencies,
                &UNKNOWN_REQUIRE_ENSURE,
                &["The first argument must be an array literal of string literals."],
            );
            t.get_compiler().report(error);
            return;
        }

        let mut dep = dependencies.get_first_child(t);
        while let Some(d) = dep {
            if !d.is_string_lit(t) {
                let error = JSError::make(
                    t,
                    d,
                    &UNKNOWN_REQUIRE_ENSURE,
                    &["The first argument must be an array literal of string literals."],
                );
                t.get_compiler().report(error);
                return;
            }
            dep = d.get_next(t);
        }
        let callback = dependencies.get_next(t).unwrap();
        if !(callback.is_function(t)
            && callback.get_second_child(t).unwrap().has_one_child(t)
            && callback
                .get_second_child(t)
                .unwrap()
                .get_first_child(t)
                .unwrap()
                .is_name(t)
            && callback
                .get_second_child(t)
                .unwrap()
                .get_first_child(t)
                .unwrap()
                .get_string(t)
                == "require")
        {
            let error = JSError::make(
                t,
                callback,
                &UNKNOWN_REQUIRE_ENSURE,
                &[
                    "The second argument must be a function whose first argument is named \"require\".",
                ],
            );
            t.get_compiler().report(error);
            return;
        }

        callback.detach(t);

        // Remove the "require" argument from the parameter list.
        callback.get_second_child(t).unwrap().remove_children(t);
        call.remove_children(t);
        call.put_boolean_prop(t, Prop::FREE_CALL, true);
        call.add_child_to_front(t, callback);

        t.report_code_change();
    }

    // port: ProcessCommonJSModules.FindImportsAndExports#reportModuleErrors
    fn report_module_errors(&self, compiler: &mut AbstractCompiler) {
        let errors = self.errors.lock().unwrap().clone();
        for error in errors {
            compiler.report(error);
        }
    }

    /// If the export is directly assigned more than once, or the assignments are not global,
    /// declare the module name variable.
    ///
    /// If all of the assignments are simply property assignments, initialize the module name
    /// variable as a namespace.
    ///
    /// Returns whether the default export can be declared constant
    // port: ProcessCommonJSModules.FindImportsAndExports#initializeModule
    fn initialize_module(&mut self, compiler: &mut AbstractCompiler) -> bool {
        let script = self.script.unwrap();
        let ci = input_of(compiler, script);

        let module_name = ProcessCommonJSModules::get_module_name(compiler, &ci);

        let mut exports_to_remove: Vec<ExportInfo> = Vec::new();
        for export in self.exports.clone() {
            if NodeUtil::get_enclosing_script(compiler, export.node).is_none() {
                exports_to_remove.push(export);
                continue;
            }
            let q_name_base =
                ProcessCommonJSModules::get_base_qualified_name_node(compiler, export.node);
            if export.node == q_name_base
                && export
                    .node
                    .get_parent(compiler)
                    .unwrap()
                    .is_assign(compiler)
                && export
                    .node
                    .get_grandparent(compiler)
                    .unwrap()
                    .is_expr_result(compiler)
                && export.node.get_previous(compiler).is_none()
                && export.node.get_next(compiler).is_some()
            {
                let next = export.node.get_next(compiler).unwrap();
                // Find any identity assignments and just remove them
                // exports = module.exports;
                if next.is_get_prop(compiler)
                    && next.matches_qualified_name(compiler, format!("{MODULE}.{EXPORTS}"))
                {
                    if let Some(index) = self
                        .module_exports
                        .iter()
                        .position(|module_export| module_export.node == next)
                    {
                        self.module_exports.remove(index);
                    }

                    let grandparent = export.node.get_grandparent(compiler).unwrap();
                    let change_root = grandparent.get_parent(compiler).unwrap();
                    grandparent.detach(compiler);
                    exports_to_remove.push(export);
                    compiler.report_change_to_enclosing_scope(change_root);

                    // Find compound identity assignments and remove the exports = portion
                    // exports = module.exports = foo;
                } else if next.is_assign(compiler)
                    && next
                        .get_first_child(compiler)
                        .unwrap()
                        .matches_qualified_name(compiler, format!("{MODULE}.{EXPORTS}"))
                {
                    let assign = next;
                    let detached = assign.detach(compiler);
                    export
                        .node
                        .get_parent(compiler)
                        .unwrap()
                        .replace_with(compiler, detached);
                    exports_to_remove.push(export);
                    compiler.report_change_to_enclosing_scope(assign);
                }
                // Find babel transpiled interop assignment
                // module.exports = exports['default'];
            } else if export
                .node
                .get_parent(compiler)
                .unwrap()
                .is_get_elem(compiler)
                && export
                    .node
                    .get_next(compiler)
                    .unwrap()
                    .is_string_lit(compiler)
                && export.node.get_next(compiler).unwrap().get_string(compiler)
                    == EXPORT_PROPERTY_NAME
                && export
                    .node
                    .get_grandparent(compiler)
                    .unwrap()
                    .is_assign(compiler)
                && export
                    .node
                    .get_parent(compiler)
                    .unwrap()
                    .get_previous(compiler)
                    .is_some()
                && export
                    .node
                    .get_parent(compiler)
                    .unwrap()
                    .get_previous(compiler)
                    .unwrap()
                    .matches_qualified_name(compiler, format!("{MODULE}.{EXPORTS}"))
            {
                let parent = export.node.get_parent(compiler).unwrap();
                let grandparent = parent.get_parent(compiler).unwrap();
                let prop = export.node.get_next(compiler).unwrap();
                let target = export.node.detach(compiler);
                let prop_string = prop.detach(compiler).get_string(compiler);
                let getprop = IR::getprop(compiler, target, prop_string).srcref(compiler, parent);
                parent.replace_with(compiler, getprop);

                compiler.report_change_to_enclosing_scope(grandparent);
            }
        }

        self.exports
            .retain(|export| !exports_to_remove.contains(export));
        exports_to_remove.clear();
        // LinkedHashMap<ExportInfo, ExportInfo> in insertion order.
        let mut exports_to_replace: Vec<(ExportInfo, ExportInfo)> = Vec::new();
        for export in self.module_exports.clone() {
            if NodeUtil::get_enclosing_script(compiler, export.node).is_none() {
                exports_to_remove.push(export);
            } else if export.node.is_get_elem(compiler) {
                let prop = export
                    .node
                    .get_second_child(compiler)
                    .unwrap()
                    .detach(compiler);
                let target = export.node.remove_first_child(compiler).unwrap();
                let prop_string = prop.get_string(compiler);
                let getprop = IR::getprop(compiler, target, prop_string);
                let new_export = ExportInfo::new(compiler, getprop, export.scope);
                export.node.replace_with(compiler, new_export.node);
                compiler.report_change_to_enclosing_scope(new_export.node);
                exports_to_replace.push((export, new_export));
            }
        }
        self.module_exports
            .retain(|export| !exports_to_remove.contains(export));
        for (old_export, new_export) in exports_to_replace {
            let old_index = self
                .module_exports
                .iter()
                .position(|export| *export == old_export)
                .expect("indexOf");
            self.module_exports.remove(old_index);
            self.module_exports.insert(old_index, new_export);
        }

        // If we assign to the variable more than once or all the assignments
        // are properties, initialize the variable as well.
        let mut direct_assignments = 0;
        for export in &self.module_exports {
            if NodeUtil::get_enclosing_script(compiler, export.node).is_none() {
                continue;
            }

            let base = ProcessCommonJSModules::get_base_qualified_name_node(compiler, export.node);
            if base == export.node
                && export
                    .node
                    .get_parent(compiler)
                    .unwrap()
                    .is_assign(compiler)
            {
                let r_value = NodeUtil::get_r_value_of_l_value(compiler, export.node);
                if r_value.is_none_or(|r_value| !r_value.is_object_lit(compiler)) {
                    direct_assignments += 1;
                } else if r_value.unwrap().is_object_lit(compiler) {
                    let mut key = r_value.unwrap().get_first_child(compiler);
                    while let Some(k) = key {
                        if (!k.is_string_key(compiler) || k.is_quoted_string_key(compiler))
                            && !k.is_member_function_def(compiler)
                        {
                            direct_assignments += 1;
                            break;
                        }
                        key = k.get_next(compiler);
                    }
                }
            }
        }

        let name = IR::name(compiler, module_name.as_str());
        let objectlit = IR::objectlit(compiler, &[]);
        let init_module = IR::var_with_value(compiler, name, objectlit);
        let init_module_name = init_module.get_first_child(compiler).unwrap();
        init_module_name.put_boolean_prop(compiler, Prop::MODULE_EXPORT, true);
        init_module_name.make_non_indexable(compiler);
        init_module.set_jsdoc_info(compiler, const_jsdoc());
        if direct_assignments == 0 || (!self.exports.is_empty() && !self.module_exports.is_empty())
        {
            let default_prop = IR::string_key(compiler, EXPORT_PROPERTY_NAME);
            default_prop.put_boolean_prop(compiler, Prop::MODULE_EXPORT, true);
            let objectlit = IR::objectlit(compiler, &[]);
            default_prop.add_child_to_front(compiler, objectlit);
            init_module
                .get_first_first_child(compiler)
                .unwrap()
                .add_child_to_front(compiler, default_prop);
            if self.exports.is_empty() || self.module_exports.is_empty() {
                default_prop.set_jsdoc_info(compiler, const_jsdoc());
            }
        }
        let init_module = init_module.srcref_tree(compiler, script);
        script.add_child_to_front(compiler, init_module);
        compiler.report_change_to_enclosing_scope(script);

        direct_assignments < 2 && (self.exports.is_empty() || self.module_exports.is_empty())
    }

    /// Find the outermost if node ancestor for a node without leaving the function scope. To
    /// match, the test class of the "if" statement must reference "module" or "define" names.
    // port: ProcessCommonJSModules.FindImportsAndExports#getOutermostUmdTest
    fn get_outermost_umd_test(&self, ast: &Ast, n: Option<NodeId>) -> Option<UmdTestInfo> {
        let n = n?;
        if NodeUtil::is_top_level(ast, n) || n.is_function(ast) {
            return None;
        }
        let parent = n.get_parent(ast)?;

        // When walking up ternary operations (hook), don't check if parent is the condition,
        // because one ternary operation can be then/else branch of another.
        if parent.is_if(ast) || parent.is_hook(ast) {
            let umd_test_info = self.get_outermost_umd_test(ast, Some(parent));
            if let Some(umd_test_info) = umd_test_info {
                // If this is the then block of an else-if statement, set the active branch to
                // the then block rather than the "if" itself.
                if parent.is_if(ast)
                    && parent.get_second_child(ast) == Some(n)
                    && parent.has_parent(ast)
                    && parent.get_parent(ast).unwrap().is_block(ast)
                    && parent.get_next(ast).is_none()
                    && Some(umd_test_info.active_branch) == parent.get_parent(ast)
                {
                    return Some(UmdTestInfo::new(umd_test_info.enclosing_if, n));
                }

                return Some(umd_test_info);
            }

            let mut umd_tests: Vec<NodeId> = Vec::new();
            Self::collect_umd_tests(ast, parent.get_first_child(ast).unwrap(), &mut umd_tests);

            // Webpack replaces tests of `typeof module !== 'undefined'` with `true`
            if umd_tests.is_empty() && parent.get_first_child(ast).unwrap().is_true(ast) {
                umd_tests.push(parent.get_first_child(ast).unwrap());
            }

            if !umd_tests.is_empty() {
                return Some(UmdTestInfo::new(parent, n));
            }

            return None;
        }

        self.get_outermost_umd_test(ast, Some(parent))
    }

    /// `NodeUtil.visitPreOrder(root, visitor)` with getOutermostUmdTest's anonymous Visitor; the
    /// visitor only reads the tree, so this walks it in the same pre-order.
    // port: ProcessCommonJSModules.FindImportsAndExports#getOutermostUmdTest (NodeUtil.Visitor#visit)
    fn collect_umd_tests(ast: &Ast, node: NodeId, umd_tests: &mut Vec<NodeId>) {
        let matches = match node.get_token(ast) {
            Token::NAME => node.get_string(ast) == MODULE || node.get_string_ref(ast) == "define",
            Token::GETPROP => WINDOW_DEFINE.matches(ast, node),
            Token::STRINGLIT => {
                node.get_parent(ast).unwrap().is_in(ast) && node.get_string_ref(ast) == "amd"
            }
            _ => false,
        };
        if matches {
            umd_tests.push(node);
        }
        let mut child = node.get_first_child(ast);
        while let Some(c) = child {
            Self::collect_umd_tests(ast, c, umd_tests);
            child = c.get_next(ast);
        }
    }

    /// Return whether the node is within the test portion of an if statement
    // port: ProcessCommonJSModules.FindImportsAndExports#isInIfTest
    fn is_in_if_test(&self, ast: &Ast, n: Option<NodeId>) -> bool {
        let Some(n) = n else {
            return false;
        };
        if NodeUtil::is_top_level(ast, n) || n.is_function(ast) {
            return false;
        }
        let Some(parent) = n.get_parent(ast) else {
            return false;
        };

        if (parent.is_if(ast) || parent.is_hook(ast)) && parent.get_first_child(ast) == Some(n) {
            return true;
        }

        self.is_in_if_test(ast, Some(parent))
    }
}

impl FindImportsAndExports {
    /// Remove a Universal Module Definition and leave just the commonjs export statement
    // port: ProcessCommonJSModules.FindImportsAndExports#replaceUmdPatterns
    fn replace_umd_patterns(&mut self, compiler: &mut AbstractCompiler) -> bool {
        let mut needs_retraverse = false;
        let mut change_scope;
        for umd_pattern in self.umd_patterns.clone() {
            if NodeUtil::get_enclosing_script(compiler, umd_pattern.if_root).is_none() {
                ProcessCommonJSModules::report_nested_scopes_deleted(compiler, umd_pattern.if_root);
                continue;
            }

            let parent = umd_pattern.if_root.get_parent(compiler).unwrap();
            let mut new_node = umd_pattern.active_branch;

            let Some(active_branch) = new_node else {
                umd_pattern.if_root.detach(compiler);
                ProcessCommonJSModules::report_nested_scopes_deleted(compiler, umd_pattern.if_root);
                compiler.report_change_to_enclosing_scope(parent);
                needs_retraverse = true;
                continue;
            };

            // Remove redundant block node. Not strictly necessary, but makes tests more legible.
            if active_branch.is_block(compiler) && active_branch.has_one_child(compiler) {
                new_node = active_branch.remove_first_child(compiler);
            } else {
                active_branch.detach(compiler);
            }
            let new_node = new_node.unwrap();
            needs_retraverse = true;
            umd_pattern.if_root.replace_with(compiler, new_node);
            ProcessCommonJSModules::report_nested_scopes_deleted(compiler, umd_pattern.if_root);
            change_scope = ChangeTracker::get_enclosing_change_scope_root(compiler, Some(new_node));
            if change_scope.is_some() {
                compiler.report_change_to_enclosing_scope(new_node);
            }

            let mut block = parent;
            if block.is_expr_result(compiler) {
                block = block.get_parent(compiler).unwrap();
            }

            // Detect UMD Factory Patterns and inline the functions
            if block.is_block(compiler)
                && block.get_parent(compiler).unwrap().is_function(compiler)
                && block.get_grandparent(compiler).unwrap().is_call(compiler)
                && parent.has_one_child(compiler)
            {
                let enclosing_fn_call = block.get_grandparent(compiler).unwrap();
                let r#fn = block.get_parent(compiler).unwrap();

                let enclosing_script = NodeUtil::get_enclosing_script(compiler, enclosing_fn_call);
                if enclosing_script.is_none() {
                    continue;
                }
                let ci = input_of(
                    compiler,
                    NodeUtil::get_enclosing_script(compiler, enclosing_fn_call).unwrap(),
                );
                let module_path = ci.get_path(compiler);
                needs_retraverse = true;
                let factory_label = JsString::from(format!(
                    "{}_factory{}",
                    module_path.to_module_name(),
                    compiler.get_unique_id_supplier().get_unique_id(&ci)
                ));

                let mutator =
                    FunctionToBlockMutator::new(compiler, compiler.get_unique_name_id_supplier());
                let mut new_statements = mutator.mutate_without_renaming(
                    compiler,
                    &factory_label,
                    r#fn,
                    enclosing_fn_call,
                    None,
                    false,
                    false,
                );

                // Check to see if the returned block is of the form:
                // {
                //   var jscomp$inline = function() {};
                //   jscomp$inline();
                // }
                //
                // or
                //
                // {
                //   var jscomp$inline = function() {};
                //   module.exports = jscomp$inline();
                // }
                //
                // If so, inline again
                if new_statements.is_block(compiler)
                    && new_statements.has_two_children(compiler)
                    && new_statements
                        .get_first_child(compiler)
                        .unwrap()
                        .is_var(compiler)
                    && new_statements
                        .get_first_first_child(compiler)
                        .unwrap()
                        .has_one_child(compiler)
                    && new_statements
                        .get_first_first_child(compiler)
                        .unwrap()
                        .get_first_child(compiler)
                        .unwrap()
                        .is_function(compiler)
                    && new_statements
                        .get_second_child(compiler)
                        .unwrap()
                        .is_expr_result(compiler)
                {
                    let inlined_fn = new_statements
                        .get_first_first_child(compiler)
                        .unwrap()
                        .get_first_child(compiler)
                        .unwrap();
                    let expr = new_statements
                        .get_second_child(compiler)
                        .unwrap()
                        .get_first_child(compiler)
                        .unwrap();
                    let mut call = None;
                    let mut assigned_name = None;
                    if expr.is_assign(compiler)
                        && expr.get_second_child(compiler).unwrap().is_call(compiler)
                    {
                        call = expr.get_second_child(compiler);
                        assigned_name = Some(JsString::from(format!(
                            "{}_iife{}",
                            module_path.to_module_name(),
                            compiler.get_unique_id_supplier().get_unique_id(&ci)
                        )));
                    } else if expr.is_call(compiler) {
                        call = Some(expr);
                    }

                    if let Some(call) = call {
                        new_statements = mutator.mutate_without_renaming(
                            compiler,
                            &factory_label,
                            inlined_fn,
                            call,
                            assigned_name.as_ref(),
                            false,
                            false,
                        );
                        if let Some(assigned_name) = assigned_name {
                            let original_name = expr
                                .get_first_child(compiler)
                                .unwrap()
                                .get_qualified_name(compiler)
                                .unwrap();
                            let name = NodeUtil::new_name_with_basis(
                                compiler,
                                assigned_name.clone(),
                                r#fn,
                                original_name,
                            );
                            let new_name = IR::var(compiler, name).srcref_tree(compiler, r#fn);
                            if new_statements.has_children(compiler)
                                && new_statements
                                    .get_first_child(compiler)
                                    .unwrap()
                                    .is_expr_result(compiler)
                                && new_statements
                                    .get_first_first_child(compiler)
                                    .unwrap()
                                    .is_assign(compiler)
                                && new_statements
                                    .get_first_first_child(compiler)
                                    .unwrap()
                                    .get_first_child(compiler)
                                    .unwrap()
                                    .is_name(compiler)
                                && new_statements
                                    .get_first_first_child(compiler)
                                    .unwrap()
                                    .get_first_child(compiler)
                                    .unwrap()
                                    .get_string(compiler)
                                    == assigned_name
                            {
                                let value = new_statements
                                    .get_first_first_child(compiler)
                                    .unwrap()
                                    .get_second_child(compiler)
                                    .unwrap()
                                    .detach(compiler);
                                new_name
                                    .get_first_child(compiler)
                                    .unwrap()
                                    .add_child_to_front(compiler, value);
                                new_statements
                                    .get_first_child(compiler)
                                    .unwrap()
                                    .replace_with(compiler, new_name);
                            } else {
                                new_statements.add_child_to_front(compiler, new_name);
                            }
                            let name_clone = new_name
                                .get_first_child(compiler)
                                .unwrap()
                                .clone_node(compiler);
                            expr.get_second_child(compiler)
                                .unwrap()
                                .replace_with(compiler, name_clone);
                            let expr_parent = expr.get_parent(compiler).unwrap().detach(compiler);
                            new_statements.add_child_to_back(compiler, expr_parent);
                        }
                    }
                }

                let mut call_root = enclosing_fn_call.get_parent(compiler).unwrap();
                if call_root.is_not(compiler) {
                    call_root = call_root.get_parent(compiler).unwrap();
                }
                if call_root.is_expr_result(compiler) {
                    let call_root_parent = call_root.get_parent(compiler).unwrap();
                    let children = new_statements.remove_children(compiler);
                    call_root_parent.add_children_after(compiler, children, Some(call_root));
                    call_root.detach(compiler);
                    ProcessCommonJSModules::report_nested_scopes_changed(
                        compiler,
                        call_root_parent,
                    );
                    compiler.report_change_to_enclosing_scope(call_root_parent);
                    ProcessCommonJSModules::report_nested_scopes_deleted(
                        compiler,
                        enclosing_fn_call,
                    );
                } else {
                    umd_pattern.if_root.replace_with(compiler, new_node);
                    compiler.report_change_to_enclosing_scope(new_node);
                    ProcessCommonJSModules::report_nested_scopes_deleted(
                        compiler,
                        umd_pattern.if_root,
                    );
                }
            }
        }
        needs_retraverse
    }
}

/// `a + b` on Java Strings.
fn concat(a: &JsString, b: impl Into<JsString>) -> JsString {
    a.concat(&b.into())
}

/// Traverse a file and rewrite all references to imported names directly to the targeted module
/// name.
///
/// If a file is a CommonJS module, rewrite export statements. Typically exports create an alias
/// - the rewriting tries to avoid such aliases.
struct RewriteModule {
    allow_full_rewrite: bool,
    exports: Vec<ExportInfo>,
    imports: Vec<NodeId>,
    rewritten_class_expressions: Vec<NodeId>,
    functions_to_hoist: Vec<NodeId>,
    default_export_is_const: bool,
}

impl RewriteModule {
    // port: ProcessCommonJSModules.RewriteModule#RewriteModule
    fn new(
        allow_full_rewrite: bool,
        exports: Vec<ExportInfo>,
        default_export_is_const: bool,
    ) -> Self {
        Self {
            allow_full_rewrite,
            exports,
            imports: Vec::new(),
            rewritten_class_expressions: Vec::new(),
            functions_to_hoist: Vec::new(),
            default_export_is_const,
        }
    }

    /// `getModuleName(t.getInput())`.
    fn current_module_name(t: &mut NodeTraversal<'_>) -> String {
        let input = traversal_input(t);
        ProcessCommonJSModules::get_module_name(t.get_compiler(), &input)
    }

    /// `getBasePropertyImport(getModuleName(t.getInput()))`.
    fn current_base_property_import(t: &mut NodeTraversal<'_>) -> JsString {
        let module_name = Self::current_module_name(t);
        JsString::from(ProcessCommonJSModules::get_base_property_import(
            t.get_compiler(),
            &module_name,
        ))
    }
}

impl Callback for RewriteModule {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ProcessCommonJSModules.RewriteModule#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        'switch: {
            match n.get_token(t) {
                Token::SCRIPT => {
                    // Class names can't be changed during the middle of a traversal. Unlike
                    // functions, the name can be the EMPTY token rather than just a zero length
                    // string.
                    for clazz in self.rewritten_class_expressions.clone() {
                        let first = clazz.get_first_child(t).unwrap();
                        let empty = IR::empty(t).srcref(t, first);
                        first.replace_with(t, empty);
                        t.report_code_change();
                    }

                    let ci = input_of(t.get_compiler(), n);
                    let module_name =
                        ProcessCommonJSModules::get_module_name(t.get_compiler(), &ci);

                    // If a function is the direct module export, move it to the top.
                    let base_property_import = ProcessCommonJSModules::get_base_property_import(
                        t.get_compiler(),
                        &module_name,
                    );
                    for i in 1..self.functions_to_hoist.len() {
                        if self.functions_to_hoist[i]
                            .get_first_first_child(t)
                            .unwrap()
                            .matches_qualified_name(t, base_property_import.as_str())
                        {
                            let fnc_var = self.functions_to_hoist.remove(i);
                            self.functions_to_hoist.insert(0, fnc_var);
                            break;
                        }
                    }

                    // Hoist functions in reverse order so that they maintain the same relative
                    // order after hoisting.
                    for i in (0..self.functions_to_hoist.len()).rev() {
                        let function_expr = self.functions_to_hoist[i];
                        let scope_root = t.get_closest_hoist_scope_root().unwrap();
                        let mut insertion_point = scope_root.get_first_child(t);
                        if insertion_point.is_none_or(|insertion_point| {
                            !(insertion_point.is_var(t)
                                && insertion_point.get_first_child(t).unwrap().get_string(t)
                                    == module_name.as_str())
                        }) {
                            insertion_point = None;
                        }

                        match insertion_point {
                            None => {
                                if scope_root.get_first_child(t) != Some(function_expr) {
                                    let detached = function_expr.detach(t);
                                    scope_root.add_child_to_front(t, detached);
                                }
                            }
                            Some(insertion_point) => {
                                if insertion_point != function_expr
                                    && insertion_point.get_next(t) != Some(function_expr)
                                {
                                    function_expr.detach(t).insert_after(t, insertion_point);
                                }
                            }
                        }
                    }

                    for export in self.exports.clone() {
                        self.visit_export(t, export);
                    }

                    for require in self.imports.clone() {
                        self.visit_require_call(t, require);
                    }
                }
                Token::CALL => {
                    if ProcessCommonJSModules::is_common_js_import_with_options(t.get_compiler(), n)
                    {
                        self.imports.push(n);
                    }
                }
                Token::VAR | Token::LET | Token::CONST => {
                    let parent = parent.unwrap();
                    // Multiple declarations need split apart so that they can be refactored into
                    // property assignments or removed altogether. Don't split declarations for
                    // ES module export calls as it breaks AST and ES modules shouldn't be
                    // affected at all by this pass.
                    if n.has_more_than_one_child(t)
                        && !NodeUtil::is_any_for(t, parent)
                        && !parent.is_export(t)
                    {
                        let vars = Self::split_multiple_declarations(t, n);
                        t.report_code_change();
                        for var in vars {
                            let first = var.get_first_child(t).unwrap();
                            self.visit(t, first, Some(var));
                        }
                    }

                    // UMD Inlining can shadow global variables - these are just removed.
                    //
                    // var exports = exports;
                    if n.get_first_child(t).unwrap().has_children(t)
                        && n.get_first_first_child(t).unwrap().is_name(t)
                        && n.get_first_child(t).unwrap().get_string(t)
                            == n.get_first_first_child(t).unwrap().get_string(t)
                    {
                        n.detach(t);
                        t.report_code_change();
                        return;
                    }
                }
                Token::NAME => {
                    // If this is a name declaration with multiple names, it will be split apart
                    // when the parent is visited and then revisit the children.
                    if NodeUtil::is_name_declaration(t, n.get_parent(t))
                        && n.get_parent(t).unwrap().has_more_than_one_child(t)
                        && !NodeUtil::is_any_for(t, n.get_grandparent(t).unwrap())
                    {
                        break 'switch;
                    }

                    let Some(q_name) = n.get_qualified_name(t) else {
                        break 'switch;
                    };
                    let scope = t.get_scope();
                    let name_declaration = scope.get_var(t.get_compiler(), q_name);
                    if let Some(name_declaration) = name_declaration {
                        if NodeUtil::is_lhs_by_destructuring(t, n) {
                            self.maybe_update_name(t, n, name_declaration);
                        } else if let Some(declaration_node) =
                            name_declaration.get_node(t.get_compiler())
                            && declaration_node.get_input_id(t) == n.get_input_id(t)
                        {
                            // Avoid renaming a shadowed global
                            //
                            // var angular = angular;  // value is global ref
                            let declaration_name_node =
                                name_declaration.get_name_node(t.get_compiler());
                            let enclosing_declaration =
                                NodeUtil::get_enclosing_node(t, n, &|_, node| {
                                    Some(node) == declaration_name_node
                                });

                            if enclosing_declaration.is_none()
                                || enclosing_declaration == Some(n)
                                || name_declaration.get_scope(t.get_compiler()) != t.get_scope()
                            {
                                self.maybe_update_name(t, n, name_declaration);
                            }
                        }
                        // Replace loose "module" references with an object literal
                    } else if self.allow_full_rewrite
                        && n.get_string(t) == MODULE
                        && !(n.get_parent(t).unwrap().is_get_prop(t)
                            || n.get_parent(t).unwrap().is_get_elem(t)
                            || n.get_parent(t).unwrap().is_type_of(t))
                    {
                        let object_lit = IR::objectlit(t, &[]).srcref(t, n);
                        n.replace_with(t, object_lit);
                        t.report_code_change_at_node(object_lit);
                    }
                }
                Token::GETPROP => {
                    if n.matches_qualified_name(t, format!("{MODULE}.id"))
                        || n.matches_qualified_name(t, format!("{MODULE}.filename"))
                    {
                        let scope = t.get_scope();
                        let v = scope.get_var(t.get_compiler(), MODULE);
                        if v.is_none_or(|v| v.is_extern(t.get_compiler())) {
                            let path = traversal_input_path(t).to_string();
                            let string = IR::string(t, path).srcref(t, n);
                            n.replace_with(t, string);
                        }
                    } else if self.allow_full_rewrite
                        && n.get_first_child(t).unwrap().is_name(t)
                        && n.get_first_child(t).unwrap().get_string(t) == MODULE
                        && !n.matches_qualified_name(t, format!("{MODULE}.{EXPORTS}"))
                    {
                        let scope = t.get_scope();
                        let v = scope.get_var(t.get_compiler(), MODULE);
                        if v.is_none_or(|v| v.is_extern(t.get_compiler())) {
                            let first = n.get_first_child(t).unwrap();
                            let object_lit = IR::objectlit(t, &[]).srcref(t, first);
                            first.replace_with(t, object_lit);
                        }
                    }
                }
                Token::TYPEOF => {
                    if self.allow_full_rewrite
                        && n.get_first_child(t).unwrap().is_name(t)
                        && (n.get_first_child(t).unwrap().get_string(t) == MODULE
                            || n.get_first_child(t).unwrap().get_string(t) == EXPORTS)
                    {
                        let scope = t.get_scope();
                        let name = n.get_first_child(t).unwrap().get_string(t);
                        let v = scope.get_var(t.get_compiler(), name);
                        if v.is_none_or(|v| v.is_extern(t.get_compiler())) {
                            let string = IR::string(t, "object");
                            n.replace_with(t, string);
                        }
                    }
                }
                _ => {}
            }
        }

        self.fix_type_annotations_for_node(t, n);
    }
}

impl RewriteModule {
    // port: ProcessCommonJSModules.RewriteModule#fixTypeAnnotationsForNode
    fn fix_type_annotations_for_node(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            for type_node in info.get_type_nodes() {
                self.fix_type_node(t, type_node);
            }
        }
    }

    /// Visit require calls. Rewrite require statements to be a direct reference to name of
    /// require module. By this point all references to the import alias should have already been
    /// renamed.
    // port: ProcessCommonJSModules.RewriteModule#visitRequireCall
    fn visit_require_call(&mut self, t: &mut NodeTraversal<'_>, require: NodeId) {
        let module_name = ProcessCommonJSModules::get_imported_module_name(t, require);
        let base_property_import = ProcessCommonJSModules::get_base_property_import_for_call(
            t.get_compiler(),
            &module_name,
            require,
        );
        let second = require.get_second_child(t).unwrap();
        let module_ref =
            NodeUtil::new_qname(t.get_compiler(), base_property_import).srcref_tree(t, second);
        require.replace_with(t, module_ref);

        t.report_code_change();
    }

    /// Visit export statements. Export statements can be either a direct assignment:
    /// module.exports = foo or a property assignment: module.exports.foo = foo; exports.foo = foo;
    // port: ProcessCommonJSModules.RewriteModule#visitExport
    fn visit_export(&mut self, t: &mut NodeTraversal<'_>, export: ExportInfo) {
        let mut root = ProcessCommonJSModules::get_base_qualified_name_node(t, export.node);
        let r_value = NodeUtil::get_r_value_of_l_value(t, root);

        // For object literal assignments to module.exports, convert them to
        // individual property assignments.
        //
        //     module.exports = { foo: bar};
        //
        // becomes
        //
        //     module.exports = {};
        //     module.exports.foo = bar;
        if MODULE_EXPORTS.matches(t, root)
            && let Some(r_value) = r_value
            && r_value.is_object_lit(t)
            && root.get_parent(t).unwrap().is_assign(t)
            && root.get_grandparent(t).unwrap().is_expr_result(t)
            && self.expand_object_lit_assignment(t, root, export.scope)
        {
            return;
        }

        let mut module_name = JsString::from(Self::current_module_name(t));
        let scope = t.get_scope();
        let module_initialization = scope.get_var(t.get_compiler(), module_name.clone());

        // If this is an assignment to module.exports or exports, renaming
        // has already handled this case. Remove the export.
        let mut r_value_var: Option<VarId> = None;
        if let Some(r_value) = r_value
            && r_value.is_qualified_name(t)
        {
            let q_name = r_value.get_qualified_name(t).unwrap();
            r_value_var = export.scope.get_var(t.get_compiler(), q_name);
        }

        if root.get_parent(t).unwrap().is_assign(t)
            && root.get_grandparent(t).unwrap().is_expr_result(t)
            && root
                .get_next(t)
                .is_some_and(|next| next.is_name(t) || next.is_get_prop(t))
            && root.get_grandparent(t).unwrap().is_expr_result(t)
            && let Some(r_value_var) = r_value_var
            && (r_value_var
                .get_name_node(t.get_compiler())
                .and_then(|name_node| NodeUtil::get_enclosing_script(t, name_node))
                .is_none()
                || (r_value_var
                    .get_name_node(t.get_compiler())
                    .unwrap()
                    .has_parent(t)
                    && !r_value_var.is_param(t.get_compiler())))
            && export.is_in_supported_scope
            && (r_value_var
                .get_name_node(t.get_compiler())
                .unwrap()
                .get_parent(t)
                .is_none()
                || !{
                    let name_node = r_value_var.get_name_node(t.get_compiler()).unwrap();
                    NodeUtil::is_lhs_by_destructuring(t, name_node)
                })
        {
            root.get_grandparent(t).unwrap().detach(t);
            t.report_code_change();
            return;
        }

        module_name = concat(&module_name, format!(".{EXPORT_PROPERTY_NAME}"));

        let export_q_name = export.node.get_qualified_name(t).unwrap();
        let updated_export = NodeUtil::new_qname_with_basis(
            t.get_compiler(),
            module_name,
            export.node,
            export_q_name,
        );
        updated_export.put_boolean_prop(t, Prop::MODULE_EXPORT, true);
        let base_property_import = Self::current_base_property_import(t);
        let export_is_const = self.default_export_is_const
            && updated_export.matches_qualified_name(t, base_property_import)
            && root == export.node
            && NodeUtil::is_l_value(t, export.node);

        let change_scope;

        if MODULE_EXPORTS.matches(t, root)
            && let Some(r_value) = r_value
            && export
                .scope
                .get_var(t.get_compiler(), "module.exports")
                .is_none()
            && root.get_parent(t).unwrap().is_assign(t)
            && root.get_grandparent(t).unwrap().is_expr_result(t)
            && module_initialization.is_none()
        {
            // Rewrite "module.exports = foo;" to "var moduleName = {default: foo};"
            let parent = root.get_parent(t).unwrap();
            let value = r_value.detach(t);
            let assign = IR::assign(t, updated_export, value);
            let export_name = IR::expr_result(t, assign);
            if export_is_const {
                let mut info = JSDocInfo::builder();
                info.record_constancy();
                let info = info.build();
                export_name
                    .get_first_child(t)
                    .unwrap()
                    .set_jsdoc_info(t, info);
            }
            let root_parent = root.get_parent(t).unwrap();
            let export_name = export_name.srcref_tree(t, root_parent);
            parent.get_parent(t).unwrap().replace_with(t, export_name);
            change_scope = ChangeTracker::get_enclosing_change_scope_root(t, Some(parent));
        } else if root.get_next(t).is_some_and(|next| next.is_name(t))
            && let Some(r_value_var) = r_value_var
            && r_value_var.is_global(t.get_compiler())
            && export.is_in_supported_scope
            && (r_value_var
                .get_name_node(t.get_compiler())
                .unwrap()
                .get_parent(t)
                .is_none()
                || !{
                    let name_node = r_value_var.get_name_node(t.get_compiler()).unwrap();
                    NodeUtil::is_lhs_by_destructuring(t, name_node)
                })
        {
            // This is a where a module export assignment is used in a complex expression.
            // Before: `SOME_VALUE !== undefined && module.exports = SOME_VALUE`
            // After: `SOME_VALUE !== undefined && module$name`
            let parent = root.get_parent(t).unwrap();
            root.detach(t);
            parent.replace_with(t, root);
            if root == export.node {
                root = updated_export;
            }
            export.node.replace_with(t, updated_export);
            change_scope = ChangeTracker::get_enclosing_change_scope_root(t, Some(root));
        } else {
            // Other references to "module.exports" are just replaced with the module name.
            export.node.replace_with(t, updated_export);
            let updated_export_parent = updated_export.get_parent(t).unwrap();
            if updated_export_parent.is_assign(t) && export_is_const {
                let parent_info = updated_export_parent.get_jsdoc_info(t);
                let mut info_builder = JSDocInfoBuilder::maybe_copy_from(parent_info.as_deref());
                info_builder.record_constancy();
                let info = info_builder.build();
                updated_export_parent.set_jsdoc_info(t, info);
            }

            change_scope = ChangeTracker::get_enclosing_change_scope_root(t, Some(updated_export));
        }

        if let Some(change_scope) = change_scope {
            t.get_compiler().report_change_to_change_scope(change_scope);
        }
    }

    /// Since CommonJS modules may have only a single export, it's common to see the export be an
    /// object pattern. We want to expand this to individual property assignments. If any
    /// individual property assignment has been renamed, it will be removed.
    ///
    /// We need to keep assignments which aren't names
    ///
    /// module.exports = { foo: bar, baz: function() {} }
    ///
    /// becomes
    ///
    /// module.exports.foo = bar; // removed later module.exports.baz = function() {};
    // port: ProcessCommonJSModules.RewriteModule#expandObjectLitAssignment
    fn expand_object_lit_assignment(
        &mut self,
        t: &mut NodeTraversal<'_>,
        export: NodeId,
        scope: ScopeId,
    ) -> bool {
        check_state!(export.get_parent(t).unwrap().is_assign(t));
        let mut insertion_ref = export.get_grandparent(t).unwrap();
        check_state!(insertion_ref.is_expr_result(t));
        let insertion_parent = insertion_ref.get_parent(t);
        check_not_null!(insertion_parent);

        let r_value = NodeUtil::get_r_value_of_l_value(t, export).unwrap();
        let mut key = r_value.get_first_child(t);

        let mut removed_nodes = false;
        while let Some(k) = key {
            if (!k.is_string_key(t) || k.is_quoted_string_key(t)) && !k.is_member_function_def(t) {
                key = k.get_next(t);
                continue;
            }

            let target = export.clone_tree(t);
            let key_string = k.get_string(t);
            let lhs = IR::getprop(t, target, key_string);
            let mut value = None;
            if k.is_string_key(t) || k.is_member_function_def(t) {
                value = k.remove_first_child(t);
            }

            let assign = IR::assign(t, lhs, value.unwrap());
            let expr = IR::expr_result(t, assign).srcref_tree_if_missing(t, k);
            expr.insert_after(t, insertion_ref);
            let new_export = ExportInfo::new(t, lhs.get_first_child(t).unwrap(), scope);
            self.visit_export(t, new_export);

            // Export statements can be removed in visitExport
            if expr.has_parent(t) {
                insertion_ref = expr;
            }

            let current_key = k;
            key = k.get_next(t);
            current_key.detach(t);
            removed_nodes = true;
        }

        if !r_value.has_children(t) {
            export.get_grandparent(t).unwrap().detach(t);
            return true;
        }

        if removed_nodes {
            t.report_code_change_at_node(r_value);
        }
        false
    }

    /// Given a name reference, check to see if it needs renamed.
    ///
    /// We handle 3 main cases: 1. References to an import alias. These are replaced with a direct
    /// reference to the imported module. 2. Names which are exported. These are rewritten to be
    /// the export assignment directly. 3. Global names: If a name is global to the script, add a
    /// suffix so it doesn't collide with any other global.
    ///
    /// Rewriting case 1 is safe to perform on all files. Cases 2 and 3 can only be done if this
    /// file is a commonjs module.
    // port: ProcessCommonJSModules.RewriteModule#maybeUpdateName
    fn maybe_update_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, var: VarId) {
        check_state!(n.is_name(t) || n.is_get_prop(t));
        check_state!(n.has_parent(t));
        let var_node = var.get_node(t.get_compiler()).unwrap();
        let imported_module_name = self.get_module_import_name(t, var_node);
        let name = n.get_qualified_name(t).unwrap();

        // Check if the name refers to a alias for a require('foo') import.
        if let Some(imported_module_name) = imported_module_name.clone()
            && n != var_node
        {
            // Reference the imported name directly, rather than the alias
            self.update_name_reference(t, n, &name, imported_module_name, false, false);
        } else if self.allow_full_rewrite {
            let exported_name = self.get_exported_name(t, n, var);

            // We need to exclude the alias created by the require import. We assume dead
            // code elimination will remove these later.
            if (n != var_node || n.get_parent(t).unwrap().is_class(t))
                && exported_name.is_none()
                && (var
                    .get_name_node(t.get_compiler())
                    .unwrap()
                    .get_parent(t)
                    .is_none()
                    || !{
                        let name_node = var.get_name_node(t.get_compiler()).unwrap();
                        NodeUtil::is_lhs_by_destructuring(t, name_node)
                    })
            {
                // The name is actually the export reference itself.
                // This will be handled later by visitExports.
                if n.get_parent(t).unwrap().is_class(t)
                    && n.get_parent(t).unwrap().get_first_child(t) == Some(n)
                {
                    self.rewritten_class_expressions
                        .push(n.get_parent(t).unwrap());
                }

                return;
            }

            // Check if the name is used as an export
            if imported_module_name.is_none()
                && let Some(exported_name) = exported_name.clone()
                && exported_name != name
                && !var.is_param(t.get_compiler())
                && (var
                    .get_name_node(t.get_compiler())
                    .unwrap()
                    .get_parent(t)
                    .is_none()
                    || !{
                        let name_node = var.get_name_node(t.get_compiler()).unwrap();
                        NodeUtil::is_lhs_by_destructuring(t, name_node)
                    })
            {
                let export_prop_is_const = self.default_export_is_const
                    && Self::current_base_property_import(t) == exported_name
                    && ProcessCommonJSModules::get_base_qualified_name_node(t, n) == n
                    && NodeUtil::is_l_value(t, n);
                self.update_name_reference(t, n, &name, exported_name, true, export_prop_is_const);

                // If it's a global name, rename it to prevent conflicts with other scripts
            } else if var.is_global(t.get_compiler()) {
                let current_module_name = Self::current_module_name(t);

                if name == current_module_name.as_str() {
                    return;
                }

                // refs to 'exports' are handled separately.
                if name == EXPORTS {
                    return;
                }

                // closure_test_suite looks for test*() functions
                if t.get_compiler()
                    .get_options()
                    .should_export_test_functions()
                    && current_module_name.starts_with("test")
                {
                    return;
                }

                let new_name = concat(&name, format!("$${current_module_name}"));
                self.update_name_reference(t, n, &name, new_name, false, false);
            }
        }
    }

    /// `name_ref` is the qualified name node, `original_name` the original name of nameRef,
    /// `new_name` the new name for nameRef, `require_function_expressions` whether named class or
    /// functions should be rewritten to variable assignments.
    // port: ProcessCommonJSModules.RewriteModule#updateNameReference
    #[allow(clippy::needless_late_init)] // Retain Java control flow.
    fn update_name_reference(
        &mut self,
        t: &mut NodeTraversal<'_>,
        name_ref: NodeId,
        original_name: &JsString,
        new_name: JsString,
        require_function_expressions: bool,
        qualified_name_is_const: bool,
    ) {
        let mut parent = check_not_null!(name_ref.get_parent(t));
        let new_name_is_qualified = new_name.index_of_char(u16::from(b'.')) >= 0;
        let new_name_is_module_export = new_name == Self::current_base_property_import(t);

        let scope = t.get_scope();
        let mut new_name_declaration = scope.get_var(t.get_compiler(), new_name.clone());

        'switch: {
            match parent.get_token(t) {
                Token::CLASS => {
                    if parent.get_index_of_child(t, name_ref) == 0
                        && (new_name_is_qualified || require_function_expressions)
                    {
                        // Refactor a named class to a class expression
                        // We can't remove the class name during a traversal, so save it for later
                        self.rewritten_class_expressions.push(parent);

                        let new_name_ref = NodeUtil::new_qname_with_basis(
                            t.get_compiler(),
                            new_name.clone(),
                            name_ref,
                            original_name.clone(),
                        );
                        if new_name_is_module_export {
                            new_name_ref.put_boolean_prop(t, Prop::MODULE_EXPORT, true);
                        }

                        let expr;
                        if !new_name_is_qualified && new_name_declaration.is_none() {
                            let null_node = IR::null_node(t);
                            expr = IR::let_with_value(t, new_name_ref, null_node)
                                .srcref_tree_if_missing(t, name_ref);
                        } else {
                            let null_node = IR::null_node(t);
                            let assign = IR::assign(t, new_name_ref, null_node);
                            expr = IR::expr_result(t, assign).srcref_tree_if_missing(t, name_ref);
                            let parent_info = parent.get_jsdoc_info(t);
                            let mut info =
                                JSDocInfoBuilder::maybe_copy_from(parent_info.as_deref());
                            parent.set_jsdoc_info(t, None);
                            if qualified_name_is_const {
                                info.record_constancy();
                            }
                            let info = info.build();
                            let expr_first = expr.get_first_child(t).unwrap();
                            expr_first.set_jsdoc_info(t, info);
                            self.fix_type_annotations_for_node(t, expr_first);
                        }
                        parent.replace_with(t, expr);
                        if expr.is_let(t) {
                            expr.get_first_first_child(t)
                                .unwrap()
                                .replace_with(t, parent);
                        } else {
                            expr.get_first_child(t)
                                .unwrap()
                                .get_second_child(t)
                                .unwrap()
                                .replace_with(t, parent);
                        }
                    } else if parent.get_index_of_child(t, name_ref) == 1 {
                        let new_name_ref = NodeUtil::new_qname_with_basis(
                            t.get_compiler(),
                            new_name.clone(),
                            name_ref,
                            original_name.clone(),
                        );
                        if new_name_is_module_export {
                            new_name_ref.put_boolean_prop(t, Prop::MODULE_EXPORT, true);
                        }
                        name_ref.replace_with(t, new_name_ref);
                    } else {
                        name_ref.set_string(t, new_name.clone());
                        name_ref.set_original_name(t, Some(original_name.clone()));
                    }
                }
                Token::FUNCTION => {
                    if new_name_is_qualified || require_function_expressions {
                        // Refactor a named function to a function expression
                        if NodeUtil::is_function_expression(t, parent) {
                            // Don't refactor if the parent is a named function expression.
                            // e.g. var foo = function foo() {};
                            return;
                        }
                        let new_name_ref = NodeUtil::new_qname_with_basis(
                            t.get_compiler(),
                            new_name.clone(),
                            name_ref,
                            original_name.clone(),
                        );
                        if new_name_is_module_export {
                            new_name_ref.put_boolean_prop(t, Prop::MODULE_EXPORT, true);
                        }
                        name_ref.set_string(t, "");

                        let expr;
                        if !new_name_is_qualified && new_name_declaration.is_none() {
                            let null_node = IR::null_node(t);
                            expr = IR::var_with_value(t, new_name_ref, null_node)
                                .srcref_tree_if_missing(t, name_ref);
                        } else {
                            let null_node = IR::null_node(t);
                            let assign = IR::assign(t, new_name_ref, null_node);
                            expr = IR::expr_result(t, assign).srcref_tree_if_missing(t, name_ref);
                        }
                        parent.replace_with(t, expr);
                        if expr.is_var(t) {
                            expr.get_first_first_child(t)
                                .unwrap()
                                .replace_with(t, parent);
                        } else {
                            expr.get_first_child(t)
                                .unwrap()
                                .get_second_child(t)
                                .unwrap()
                                .replace_with(t, parent);
                            let parent_info = parent.get_jsdoc_info(t);
                            let mut info =
                                JSDocInfoBuilder::maybe_copy_from(parent_info.as_deref());
                            parent.set_jsdoc_info(t, None);
                            if qualified_name_is_const {
                                info.record_constancy();
                            }
                            let info = info.build();
                            let expr_first = expr.get_first_child(t).unwrap();
                            expr_first.set_jsdoc_info(t, info);
                            self.fix_type_annotations_for_node(t, expr_first);
                        }
                        self.functions_to_hoist.push(expr);
                    } else {
                        name_ref.set_string(t, new_name.clone());
                        name_ref.set_original_name(t, Some(original_name.clone()));
                    }
                }
                Token::VAR | Token::LET | Token::CONST => {
                    // Multiple declaration - needs split apart.
                    if parent.has_more_than_one_child(t)
                        && !NodeUtil::is_any_for(t, parent.get_parent(t).unwrap())
                    {
                        Self::split_multiple_declarations(t, parent);
                        parent = name_ref.get_parent(t).unwrap();
                        let scope = t.get_scope();
                        new_name_declaration = scope.get_var(t.get_compiler(), new_name.clone());
                    }

                    if new_name_is_qualified {
                        // Var declarations without initialization can simply
                        // be removed if they are being converted to a property.
                        if !name_ref.has_children(t) && parent.get_jsdoc_info(t).is_none() {
                            parent.detach(t);
                            break 'switch;
                        }

                        // Refactor a var declaration to a getprop assignment
                        let get_prop = NodeUtil::new_qname_with_basis(
                            t.get_compiler(),
                            new_name.clone(),
                            name_ref,
                            original_name.clone(),
                        );
                        if new_name_is_module_export {
                            get_prop.put_boolean_prop(t, Prop::MODULE_EXPORT, true);
                        }
                        let info = parent.get_jsdoc_info(t);
                        parent.set_jsdoc_info(t, None);
                        if name_ref.has_children(t) {
                            let value = name_ref.remove_first_child(t).unwrap();
                            let assign = IR::assign(t, get_prop, value);
                            assign.set_jsdoc_info(t, info.clone());
                            let expr =
                                IR::expr_result(t, assign).srcref_tree_if_missing(t, name_ref);
                            parent.replace_with(t, expr);
                            let mut info_builder =
                                JSDocInfoBuilder::maybe_copy_from(info.as_deref());
                            parent.set_jsdoc_info(t, None);
                            if qualified_name_is_const {
                                info_builder.record_constancy();
                            }
                            let built = info_builder.build();
                            assign.set_jsdoc_info(t, built);
                            self.fix_type_annotations_for_node(t, assign);
                        } else {
                            get_prop.set_jsdoc_info(t, info);
                            let expr = IR::expr_result(t, get_prop).srcref(t, get_prop);
                            parent.replace_with(t, expr);
                        }
                    } else if let Some(new_name_declaration) = new_name_declaration
                        && new_name_declaration.get_name_node(t.get_compiler()) != Some(name_ref)
                    {
                        // Variable is already defined. Convert this to an assignment.
                        // If the variable declaration has no initialization, we simply
                        // remove the node. This can occur when the variable which is exported
                        // is declared in an outer scope but assigned in an inner one.
                        if !name_ref.has_children(t) {
                            parent.detach(t);
                            break 'switch;
                        }

                        let name = NodeUtil::new_name_with_basis(
                            t.get_compiler(),
                            new_name.clone(),
                            name_ref,
                            original_name.clone(),
                        );
                        let value = name_ref.remove_first_child(t).unwrap();
                        let assign = IR::assign(t, name, value);
                        let info = parent.get_jsdoc_info(t);
                        if info.is_some() {
                            parent.set_jsdoc_info(t, None);
                            assign.set_jsdoc_info(t, info);
                        }

                        let expr = IR::expr_result(t, assign).srcref_tree(t, name_ref);
                        parent.replace_with(t, expr);
                    } else {
                        name_ref.set_string(t, new_name.clone());
                        name_ref.set_original_name(t, Some(original_name.clone()));
                    }
                }
                _ => {
                    // Whenever possible, reuse the existing reference
                    if !new_name_is_qualified && name_ref.is_name(t) {
                        name_ref.set_string(t, new_name.clone());
                        name_ref.set_original_name(t, Some(original_name.clone()));
                    } else {
                        let name = if new_name_is_qualified {
                            NodeUtil::new_qname_with_basis(
                                t.get_compiler(),
                                new_name.clone(),
                                name_ref,
                                original_name.clone(),
                            )
                        } else {
                            NodeUtil::new_name_with_basis(
                                t.get_compiler(),
                                new_name.clone(),
                                name_ref,
                                original_name.clone(),
                            )
                        };

                        if new_name_is_module_export {
                            name.put_boolean_prop(t, Prop::MODULE_EXPORT, true);
                        }
                        let info = name_ref.get_jsdoc_info(t);
                        if info.is_some() {
                            name_ref.set_jsdoc_info(t, None);
                            name.set_jsdoc_info(t, info);
                        }
                        name.srcref_tree(t, name_ref);
                        name_ref.replace_with(t, name);
                        if name_ref.has_children(t) {
                            let children = name_ref.remove_children(t);
                            name.add_children_to_front(t, children);
                        }
                    }
                }
            }
        }

        t.report_code_change();
    }

    /// Determine whether the given name Node n is referenced in an export
    ///
    /// Returns its own name if the name is not used in an export; if the name node is actually
    /// the export target itself, returns null.
    // port: ProcessCommonJSModules.RewriteModule#getExportedName
    fn get_exported_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        var: VarId,
    ) -> Option<JsString> {
        if var.get_node(t.get_compiler()).unwrap().get_input_id(t) != n.get_input_id(t) {
            return n.get_qualified_name(t);
        }

        let base_export_name = Self::current_base_property_import(t);

        for export in self.exports.clone() {
            let export_base = ProcessCommonJSModules::get_base_qualified_name_node(t, export.node);
            let export_r_value = NodeUtil::get_r_value_of_l_value(t, export_base);

            let Some(export_r_value) = export_r_value else {
                continue;
            };

            let exported_name = Self::get_exported_name_node(t.get_compiler(), export);
            // We don't want to handle the export itself
            if export.is_in_supported_scope
                && (export_r_value == n
                    || ((NodeUtil::is_class_expression(t, export_r_value)
                        || NodeUtil::is_function_expression(t, export_r_value))
                        && exported_name == Some(n)))
            {
                return None;
            }

            let export_base_q_name = export_base.get_qualified_name(t);

            if export_r_value.is_object_lit(t) {
                if export_base_q_name
                    .as_ref()
                    .is_none_or(|q| *q != "module.exports")
                {
                    return n.get_qualified_name(t);
                }

                let mut key = export_r_value.get_first_child(t);
                let mut key_is_export = false;
                let language_in = t
                    .get_compiler()
                    .get_options()
                    .get_language_in()
                    .to_feature_set();
                while let Some(k) = key {
                    if k.is_string_key(t)
                        && !k.is_quoted_string_key(t)
                        && NodeUtil::is_valid_property_name(language_in, &k.get_string(t))
                    {
                        let key_first = k.get_first_child(t).unwrap();
                        if key_first.is_qualified_name(t) {
                            if key_first == n {
                                return None;
                            }

                            let scope = t.get_scope();
                            let key_q_name = key_first.get_qualified_name(t).unwrap();
                            let val_var = scope.get_var(t.get_compiler(), key_q_name);
                            if let Some(val_var) = val_var
                                && val_var.get_name_node(t.get_compiler())
                                    == var.get_name_node(t.get_compiler())
                            {
                                key_is_export = true;
                                break;
                            }
                        }
                    }

                    key = k.get_next(t);
                }
                if let Some(k) = key
                    && key_is_export
                {
                    if export.is_in_supported_scope {
                        return Some(concat(&concat(&base_export_name, "."), k.get_string(t)));
                    } else {
                        return n.get_qualified_name(t);
                    }
                }
            } else {
                if !export.is_in_supported_scope {
                    return n.get_qualified_name(t);
                }
                if var.get_name_node(t.get_compiler()) == exported_name {
                    let export_base_q_name = export_base_q_name.unwrap();
                    let export_prefix: JsString = if export_base_q_name.starts_with(&MODULE.into())
                    {
                        JsString::from(format!("{MODULE}.{EXPORTS}"))
                    } else {
                        JsString::from(EXPORTS)
                    };

                    if export_base_q_name.length() == export_prefix.length() {
                        return Some(base_export_name);
                    }

                    return Some(concat(
                        &base_export_name,
                        export_base_q_name.substring_from(export_prefix.length()),
                    ));
                }
            }
        }
        n.get_qualified_name(t)
    }

    // port: ProcessCommonJSModules.RewriteModule#getExportedNameNode
    fn get_exported_name_node(compiler: &mut AbstractCompiler, info: ExportInfo) -> Option<NodeId> {
        let q_name_base = ProcessCommonJSModules::get_base_qualified_name_node(compiler, info.node);
        let r_value = NodeUtil::get_r_value_of_l_value(compiler, q_name_base)?;

        if NodeUtil::is_function_expression(compiler, r_value) {
            return r_value.get_first_child(compiler);
        }

        if NodeUtil::is_class_expression(compiler, r_value)
            && r_value.get_first_child(compiler) == Some(q_name_base)
        {
            return r_value.get_first_child(compiler);
        }

        if !r_value.is_name(compiler) {
            return None;
        }

        let name = r_value.get_string(compiler);
        let var = info.scope.get_var(compiler, name)?;

        var.get_name_node(compiler)
    }

    /// Determine if the given Node n is an alias created by a module import.
    ///
    /// Returns null if it's not an alias or the imported module name
    // port: ProcessCommonJSModules.RewriteModule#getModuleImportName
    fn get_module_import_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> Option<JsString> {
        let mut r_value = None;
        let mut prop_suffix = JsString::from("");
        let parent = n.get_parent(t);
        if let Some(parent) = parent
            && parent.is_string_key(t)
        {
            let grandparent = parent.get_parent(t).unwrap();
            if grandparent.is_object_pattern(t)
                && grandparent.get_parent(t).unwrap().is_destructuring_lhs(t)
            {
                r_value = grandparent.get_next(t);
                prop_suffix = concat(&JsString::from("."), parent.get_string(t));
            }
        }
        if prop_suffix.is_empty() && parent.is_some() {
            r_value = NodeUtil::get_r_value_of_l_value(t, n);
        }

        let r_value = r_value?;
        // Read before the chain (a pure read) because the compiler is borrowed in it.
        let r_value_first = r_value.get_first_child(t);

        if r_value.is_call(t)
            && ProcessCommonJSModules::is_common_js_import_with_options(t.get_compiler(), r_value)
        {
            let imported = ProcessCommonJSModules::get_imported_module_name(t, r_value);
            let base = ProcessCommonJSModules::get_base_property_import_for_call(
                t.get_compiler(),
                &imported,
                r_value,
            );
            return Some(concat(&JsString::from(base), prop_suffix));
        } else if r_value.is_get_prop(t)
            && ProcessCommonJSModules::is_common_js_import_with_options(
                t.get_compiler(),
                r_value_first.unwrap(),
            )
        {
            // var foo = require('bar').foo;
            let call = r_value.get_first_child(t).unwrap();
            let imported = ProcessCommonJSModules::get_imported_module_name(t, call);
            let import_name =
                JsString::from(ProcessCommonJSModules::get_base_property_import_for_call(
                    t.get_compiler(),
                    &imported,
                    call,
                ));

            let suffix = r_value.get_string(t);

            return Some(concat(
                &concat(&concat(&import_name, "."), suffix),
                prop_suffix,
            ));
        }

        None
    }

    /// Update any type references in JSDoc annotations to account for all the rewriting we've
    /// done.
    // port: ProcessCommonJSModules.RewriteModule#fixTypeNode
    fn fix_type_node(&mut self, t: &mut NodeTraversal<'_>, type_node: NodeId) {
        if type_node.is_string_lit(t) {
            let name = type_node.get_string(t);
            // Type nodes can be module paths.
            if ModuleLoader::is_path_identifier(&name.to_string()) {
                let last_slash = name.last_index_of_char(u16::from(b'/'));
                let mut end_index = name.index_of_from(&".".into(), last_slash);
                let mut local_type_name = None;
                if end_index == -1 {
                    end_index = name.length() as i32;
                } else {
                    local_type_name = Some(name.substring_from(end_index as usize));
                }

                let module_name = name.substring(0, end_index as usize);
                let global_module_name = ProcessCommonJSModules::get_imported_module_name_for_path(
                    t,
                    type_node,
                    &module_name.to_string(),
                );
                let base_import_property =
                    JsString::from(ProcessCommonJSModules::get_base_property_import(
                        t.get_compiler(),
                        &global_module_name,
                    ));
                type_node.set_string(
                    t,
                    match local_type_name {
                        None => base_import_property,
                        Some(local_type_name) => concat(&base_import_property, local_type_name),
                    },
                );
            } else {
                // A type node can be a getprop. Any portion of the getprop
                // can be either an import alias or export alias. Check each
                // segment.
                let mut was_rewritten = false;
                let mut end_index: i32 = -1;
                while end_index < name.length() as i32 {
                    end_index = name.index_of_from(&".".into(), end_index + 1);
                    if end_index == -1 {
                        end_index = name.length() as i32;
                    }
                    let base_name = name.substring(0, end_index as usize);
                    let suffix = if end_index < name.length() as i32 {
                        name.substring_from(end_index as usize)
                    } else {
                        JsString::from("")
                    };
                    let scope = t.get_scope();
                    let type_declaration = scope.get_var(t.get_compiler(), base_name);

                    // Make sure we can find a variable declaration (and it's in this file)
                    if let Some(type_declaration) = type_declaration
                        && let Some(declaration_node) = type_declaration.get_node(t.get_compiler())
                        && declaration_node.get_input_id(t) == type_node.get_input_id(t)
                    {
                        let imported_module_name = self.get_module_import_name(t, declaration_node);

                        // If the name is an import alias, rewrite it to be a reference to the
                        // module name directly
                        if let Some(imported_module_name) = imported_module_name {
                            type_node.set_string(t, concat(&imported_module_name, suffix));
                            type_node.set_original_name(t, Some(name.clone()));
                            was_rewritten = true;
                            break;
                        } else if self.allow_full_rewrite {
                            // Names referenced in export statements can only be rewritten in
                            // commonjs modules.
                            let exported_name =
                                self.get_exported_name(t, type_node, type_declaration);
                            if let Some(exported_name) = exported_name
                                && exported_name != name
                            {
                                type_node.set_string(t, concat(&exported_name, suffix));
                                type_node.set_original_name(t, Some(name.clone()));
                                was_rewritten = true;
                                break;
                            }
                        }
                    }
                }

                // If the name was neither an import alias or referenced in an export,
                // We still may need to rename it if it's global
                if !was_rewritten && self.allow_full_rewrite {
                    end_index = name.index_of_char(u16::from(b'.'));
                    if end_index == -1 {
                        end_index = name.length() as i32;
                    }
                    let base_name = name.substring(0, end_index as usize);
                    let scope = t.get_scope();
                    let type_declaration = scope.get_var(t.get_compiler(), base_name.clone());
                    if let Some(type_declaration) = type_declaration
                        && type_declaration.is_global(t.get_compiler())
                    {
                        let module_name = Self::current_module_name(t);
                        let mut new_name = concat(&base_name, format!("$${module_name}"));
                        if end_index < name.length() as i32 {
                            new_name = concat(&new_name, name.substring_from(end_index as usize));
                        }

                        type_node.set_string(t, new_name);
                        type_node.set_original_name(t, Some(name.clone()));
                    }
                }
            }
        }

        let mut child = type_node.get_first_child(t);
        while let Some(c) = child {
            self.fix_type_node(t, c);
            child = c.get_next(t);
        }
    }

    // port: ProcessCommonJSModules.RewriteModule#splitMultipleDeclarations
    fn split_multiple_declarations(ast: &mut Ast, var: NodeId) -> Vec<NodeId> {
        check_state!(NodeUtil::is_name_declaration(ast, Some(var)));
        let mut vars = Vec::new();
        let info = var.get_jsdoc_info(ast);
        while var.get_second_child(ast).is_some() {
            let first = var.remove_first_child(ast).unwrap();
            let new_var = ast.new_node_with_child(var.get_token(ast), first);

            if let Some(info) = &info {
                new_var.set_jsdoc_info(ast, Some(Arc::new(info.clone_info())));
            }

            new_var.srcref(ast, var);
            new_var.insert_before(ast, var);
            vars.push(new_var);
        }
        vars.push(var);
        vars
    }
}
