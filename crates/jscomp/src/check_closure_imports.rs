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
//   src/com/google/javascript/jscomp/CheckClosureImports.java.

//! Port of `CheckClosureImports.java`: checks all goog.requires, goog.module.gets,
//! goog.forwardDeclares, and goog.requireTypes in all files.
#![allow(clippy::collapsible_if, clippy::collapsible_else_if)] // Keep Java's control flow.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_check_module::INCORRECT_SHORTNAME_CAPITALIZATION,
    closure_primitive_errors::{
        INVALID_CLOSURE_CALL_SCOPE_ERROR, INVALID_GET_CALL_SCOPE, MISSING_MODULE_OR_PROVIDE,
        MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE, MODULE_USES_GOOG_MODULE_GET,
    },
    closure_rewrite_module::INVALID_GET_ALIAS,
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    diagnostic_type::DiagnosticType,
    es6_rewrite_modules::SHOULD_IMPORT_ES6_MODULE,
    guava_ascii::{is_upper_case, to_lower_case, to_upper_case},
    js_error::JSError,
    modules::module_metadata_map::{ModuleMetadata, ModuleMetadataMap},
    node_traversal::{AbstractModuleCallback, ModuleCallback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::sync::{Arc, LazyLock};

// port: CheckClosureImports.ClosureImport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum ClosureImport {
    REQUIRE,
    FORWARD_DECLARE,
    REQUIRE_TYPE,
    REQUIRE_DYNAMIC,
}

impl ClosureImport {
    /// Whether or not a destructuring alias is allowed, e.g. `const {Class} =
    /// goog.require('namespace');`.
    // port: CheckClosureImports.ClosureImport#allowDestructuring
    fn allow_destructuring(self) -> bool {
        match self {
            ClosureImport::REQUIRE => true,
            ClosureImport::FORWARD_DECLARE => false,
            ClosureImport::REQUIRE_TYPE => true,
            ClosureImport::REQUIRE_DYNAMIC => true,
        }
    }

    /// True if the alias of the call must be constant, false if it can be `let`.
    ///
    /// Note: since not every goog.module is using ES6, `const` is only checked in ES6 modules.
    /// `let` will always be disallowed if this is false. But `var` will be allowed in
    /// goog.module files even if this returns true (since it assumes the file is non-ES6).
    // port: CheckClosureImports.ClosureImport#aliasMustBeConstant
    fn alias_must_be_constant(self) -> bool {
        match self {
            ClosureImport::REQUIRE => true,
            ClosureImport::FORWARD_DECLARE => false,
            ClosureImport::REQUIRE_TYPE => true,
            ClosureImport::REQUIRE_DYNAMIC => true,
        }
    }

    /// True if the providing symbol must appear before this call in the program.
    ///
    /// A `goog.require` for something can only appear after it is defined (e.g. `goog.provide`,
    /// `goog.module`, or `goog.declareModuleId` call). But the same is not true for
    /// `goog.requireType` and `goog.forwardDeclare`, which can appear before the definition.
    // port: CheckClosureImports.ClosureImport#mustBeOrdered
    fn must_be_ordered(self) -> bool {
        match self {
            ClosureImport::REQUIRE => true,
            ClosureImport::FORWARD_DECLARE => false,
            ClosureImport::REQUIRE_TYPE => false,
            ClosureImport::REQUIRE_DYNAMIC => false,
        }
    }

    /// Human readable name of this call, used in error reporting.
    // port: CheckClosureImports.ClosureImport#callName
    fn call_name(self) -> &'static str {
        match self {
            ClosureImport::REQUIRE => "goog.require",
            ClosureImport::FORWARD_DECLARE => "goog.forwardDeclare",
            ClosureImport::REQUIRE_TYPE => "goog.requireType",
            ClosureImport::REQUIRE_DYNAMIC => "goog.requireDynamic",
        }
    }
}

// port: CheckClosureImports#INVALID_CLOSURE_IMPORT_DESTRUCTURING
pub static INVALID_CLOSURE_IMPORT_DESTRUCTURING: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_CLOSURE_IMPORT_DESTRUCTURING",
    "Destructuring {0} must be a simple object pattern.",
);

// port: CheckClosureImports#ONE_CLOSURE_IMPORT_PER_DECLARATION
pub static ONE_CLOSURE_IMPORT_PER_DECLARATION: DiagnosticType = DiagnosticType::error(
    "JSC_ONE_CLOSURE_IMPORT_PER_DECLARATION",
    "There may only be one {0} per var/let/const declaration.",
);

// port: CheckClosureImports#INVALID_CLOSURE_IMPORT_CALL
pub static INVALID_CLOSURE_IMPORT_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_CLOSURE_IMPORT_CALL",
    "{0} parameter must be a string literal.",
);

// port: CheckClosureImports#LATE_PROVIDE_ERROR
pub static LATE_PROVIDE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_LATE_PROVIDE_ERROR",
    "Required namespace \"{0}\" not provided yet.",
);

// port: CheckClosureImports#LET_CLOSURE_IMPORT
pub static LET_CLOSURE_IMPORT: DiagnosticType = DiagnosticType::disabled(
    "JSC_LET_CLOSURE_IMPORT",
    "Module imports must be constant. Please use ''const'' instead of ''let''.",
);

// port: CheckClosureImports#NO_CLOSURE_IMPORT_DESTRUCTURING
pub static NO_CLOSURE_IMPORT_DESTRUCTURING: DiagnosticType = DiagnosticType::error(
    "JSC_NO_CLOSURE_IMPORT_DESTRUCTURING",
    "Cannot destructure the return value of {0}",
);

// port: CheckClosureImports#LHS_OF_CLOSURE_IMPORT_MUST_BE_CONST_IN_ES_MODULE
pub static LHS_OF_CLOSURE_IMPORT_MUST_BE_CONST_IN_ES_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_LHS_OF_CLOUSRE_IMPORT_MUST_BE_CONST_IN_ES_MODULE",
    "The left side of a {0} must use ''const'' (not ''let'' or ''var'') in an ES module.",
);

// port: CheckClosureImports#CROSS_CHUNK_REQUIRE_ERROR
pub static CROSS_CHUNK_REQUIRE_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_XMODULE_REQUIRE_ERROR",
    "namespace \"{0}\" is required in chunk {2} but provided in chunk {1}. Is chunk {2} missing a dependency on chunk {1}?",
);

/// Rust-only: Java's static `Node` constants live in their own arena; they are matched with
/// `Node#matchesQualifiedName(Node)` across the two arenas.
fn static_qname(names: &[&str]) -> (Ast, NodeId) {
    let mut ast = Ast::new();
    let mut node = IR::name(&mut ast, names[0]);
    for name in &names[1..] {
        node = IR::getprop(&mut ast, node, *name);
    }
    (ast, node)
}

// port: CheckClosureImports#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<(Ast, NodeId)> = LazyLock::new(|| static_qname(&["goog", "require"]));
// port: CheckClosureImports#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<(Ast, NodeId)> =
    LazyLock::new(|| static_qname(&["goog", "module", "get"]));
// port: CheckClosureImports#GOOG_FORWARD_DECLARE
static GOOG_FORWARD_DECLARE: LazyLock<(Ast, NodeId)> =
    LazyLock::new(|| static_qname(&["goog", "forwardDeclare"]));
// port: CheckClosureImports#GOOG_REQUIRE_TYPE
static GOOG_REQUIRE_TYPE: LazyLock<(Ast, NodeId)> =
    LazyLock::new(|| static_qname(&["goog", "requireType"]));
// port: CheckClosureImports#GOOG_REQUIRE_DYNAMIC
static GOOG_REQUIRE_DYNAMIC: LazyLock<(Ast, NodeId)> =
    LazyLock::new(|| static_qname(&["goog", "requireDynamic"]));

// Node#matchesQualifiedName(Node) against the static patterns, ported from Closure's
// Rhino-derived Node (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "check_closure_imports_rhino.rs"]
mod rhino;
use rhino::matches_static;

/// Checks all goog.requires, goog.module.gets, goog.forwardDeclares, and goog.requireTypes in all
/// files.
///
/// Checks that these dependency calls both contain a valid Closure namespace and are in an
/// acceptable location (e.g. a goog.require cannot be within a function).
pub struct CheckClosureImports {
    // Java's Checker holds the map; the AbstractModuleCallback is built from it in process.
    module_metadata_map: Arc<ModuleMetadataMap>,
    namespaces_seen: IndexSet<JsString>,
}

impl CheckClosureImports {
    // port: CheckClosureImports#CheckClosureImports
    // (Java also captures compiler.getChunkGraph(); it is read where it is used.)
    pub fn new(_compiler: &AbstractCompiler, module_metadata_map: Arc<ModuleMetadataMap>) -> Self {
        Self {
            module_metadata_map,
            namespaces_seen: IndexSet::<_>::default(),
        }
    }

    // port: CheckClosureImports#verifyRequireOrder
    fn verify_require_order(
        &mut self,
        namespace: &JsString,
        call: NodeId,
        t: &mut NodeTraversal<'_>,
        required_module: &Arc<ModuleMetadata>,
    ) {
        if !self.namespaces_seen.contains(namespace) {
            t.report(call, &LATE_PROVIDE_ERROR, &[&namespace.to_string()]);
            return;
        }

        let Some(required_root) = required_module.root_node() else {
            return; // synthetic metadata for tests may have no root node.
        };
        if required_root.is_from_externs(t) {
            return; // synthetic metadata for tests may have no root node.
        }

        let required_input_id = NodeUtil::get_input_id(t, required_root);
        let required_input = required_input_id
            .as_ref()
            .and_then(|id| t.get_compiler().get_input(id));
        let Some(required_input) = required_input else {
            let message = format!(
                "Cannot find CompilerInput for {}",
                required_module.to_string(t)
            );
            panic!("NullPointerException: {message}");
        };

        let required_chunk = required_input.get_chunk();
        let current_chunk = t.get_chunk();
        if current_chunk != required_chunk && {
            let chunk_graph = t.get_compiler().get_chunk_graph().unwrap();
            !chunk_graph.depends_on(
                current_chunk.as_ref().unwrap(),
                required_chunk.as_ref().unwrap(),
            )
        } {
            let required_chunk_name = required_chunk.as_ref().unwrap().get_name();
            let current_chunk_name = current_chunk.as_ref().unwrap().get_name();
            let error = JSError::make(
                t,
                call,
                &CROSS_CHUNK_REQUIRE_ERROR,
                &[
                    &namespace.to_string(),
                    &required_chunk_name,
                    &current_chunk_name,
                ],
            );
            t.get_compiler().report(error);
        }
    }
}

impl CompilerPass for CheckClosureImports {
    // port: CheckClosureImports#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let module_metadata_map = self.module_metadata_map.clone();
        let mut checker = AbstractModuleCallback::new(&module_metadata_map, Checker { pass: self });
        NodeTraversal::traverse(compiler, externs, &mut checker);
        NodeTraversal::traverse(compiler, root, &mut checker);
    }
}

// port: CheckClosureImports.Checker
struct Checker<'p> {
    pass: &'p mut CheckClosureImports,
}

impl ModuleCallback for Checker<'_> {
    // port: CheckClosureImports.Checker#enterModule
    fn enter_module(
        &mut self,
        _compiler: &mut AbstractCompiler,
        current_module: &Arc<ModuleMetadata>,
        _module_scope_root: NodeId,
    ) {
        self.pass
            .namespaces_seen
            .extend(current_module.goog_namespaces().element_set().cloned());
    }

    // port: CheckClosureImports.Checker#visit
    fn visit(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_module: Option<&Arc<ModuleMetadata>>,
        _module_scope_root: Option<NodeId>,
    ) {
        // currentModule is null on ROOT nodes.
        let Some(current_module) = current_module else {
            return;
        };

        let parent = n.get_parent(t).unwrap();

        if n.is_call(t) {
            let kind_of_call = Self::kind_of_call(t, n);
            if let Some(kind_of_call) = kind_of_call {
                self.check_import(t, n, parent, current_module, kind_of_call);
            } else if matches_static(t, n.get_first_child(t).unwrap(), &GOOG_MODULE_GET) {
                self.check_goog_module_get(t, n, current_module);
            }
        } else if n.is_name(t) {
            Self::check_valid_import_code_reference(t, n);
        }
    }
}

impl Checker<'_> {
    /// Given a call node determines what kind of `ClosureImport` it is. Returns null if not a
    /// `ClosureImport`.
    // port: CheckClosureImports.Checker#kindOfCall
    fn kind_of_call(ast: &Ast, call_node: NodeId) -> Option<ClosureImport> {
        check_state!(call_node.is_call(ast));
        let callee = call_node.get_first_child(ast).unwrap();
        if matches_static(ast, callee, &GOOG_REQUIRE) {
            return Some(ClosureImport::REQUIRE);
        } else if matches_static(ast, callee, &GOOG_FORWARD_DECLARE) {
            return Some(ClosureImport::FORWARD_DECLARE);
        } else if matches_static(ast, callee, &GOOG_REQUIRE_TYPE) {
            return Some(ClosureImport::REQUIRE_TYPE);
        } else if matches_static(ast, callee, &GOOG_REQUIRE_DYNAMIC) {
            return Some(ClosureImport::REQUIRE_DYNAMIC);
        }
        None
    }

    // port: CheckClosureImports.Checker#checkValidImportCodeReference
    fn check_valid_import_code_reference(t: &mut NodeTraversal<'_>, name_node: NodeId) {
        let name = name_node.get_string(t);
        let scope = t.get_scope();
        let var = scope.get_var(t.get_compiler(), name);

        let Some(var) = var else {
            return;
        };
        if var.get_name_node(t.get_compiler()) == Some(name_node) {
            return;
        }

        let declaration_name_node = var.get_name_node(t.get_compiler());
        let Some(declaration_name_node) = declaration_name_node else {
            return;
        };
        if !NodeUtil::is_declaration_l_value(t, declaration_name_node) {
            return;
        }

        let decl = if declaration_name_node
            .get_parent(t)
            .unwrap()
            .is_string_key(t)
        {
            declaration_name_node
                .get_grandparent(t)
                .unwrap()
                .get_grandparent(t)
        } else {
            declaration_name_node.get_parent(t)
        };
        if !NodeUtil::is_name_declaration(t, decl)
            || !decl.unwrap().has_one_child(t)
            || !decl.unwrap().get_first_child(t).unwrap().has_children(t)
            || !decl
                .unwrap()
                .get_first_child(t)
                .unwrap()
                .get_last_child(t)
                .unwrap()
                .is_call(t)
        {
            return;
        }

        let kind_of_call = Self::kind_of_call(
            t,
            decl.unwrap()
                .get_first_child(t)
                .unwrap()
                .get_last_child(t)
                .unwrap(),
        );

        if kind_of_call.is_none() {
            #[allow(clippy::needless_return)]
            return;
        }
    }

    // port: CheckClosureImports.Checker#checkGoogModuleGet
    fn check_goog_module_get(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call: NodeId,
        current_module: &Arc<ModuleMetadata>,
    ) {
        if t.in_module_hoist_scope() {
            t.report(call, &MODULE_USES_GOOG_MODULE_GET, &[]);
            return;
        }

        if !current_module.is_es6_module()
            && !current_module.is_goog_module()
            && t.get_compiler().get_options().get_module_resolution_mode()
                != ResolutionMode::WEBPACK
        {
            // Here we are making a heuristic check of the use of goog.module.get.  There are many
            // different ways to deliberately subvert these checks.  What we are trying to avoid
            // is accidential use of a `goog.provide` file as module scoped. So we want to avoid:
            //   `const Foo = goog.module.get('a.b.Foo');
            // and similar patterns, but we won't try to make this a perfect check.

            // Anything not in global scope is ok.
            if t.in_global_scope() {
                // Find the root of expressions like `goog.module.get().export`
                let mut get_sub_expr_root = call;
                while NodeUtil::is_normal_or_opt_chain_get_prop(
                    t,
                    get_sub_expr_root.get_parent(t).unwrap(),
                ) {
                    get_sub_expr_root = get_sub_expr_root.get_parent(t).unwrap();
                }

                let mut invalid = false;
                let parent = get_sub_expr_root.get_parent(t).unwrap();
                if parent.is_name(t) || parent.is_destructuring_lhs(t) {
                    invalid = true;
                } else if parent.is_assign(t) {
                    let target = parent.get_first_child(t).unwrap();
                    if target.is_name(t) || target.is_object_pattern(t) {
                        invalid = true;
                    }
                }

                if invalid {
                    t.report(call, &INVALID_GET_CALL_SCOPE, &[]);
                }
            }
        }

        if !call.has_two_children(t) || !call.get_second_child(t).unwrap().is_string_lit(t) {
            t.report(call, &INVALID_CLOSURE_IMPORT_CALL, &["goog.module.get"]);
            return;
        }

        let namespace = call.get_second_child(t).unwrap().get_string(t);
        let required_module = self
            .pass
            .module_metadata_map
            .get_modules_by_goog_namespace()
            .get(&namespace)
            .cloned();

        if required_module.is_none() {
            t.report(call, &MISSING_MODULE_OR_PROVIDE, &[&namespace.to_string()]);
            return;
        }

        let maybe_assign = call.get_parent(t).unwrap();
        let is_filling_an_alias =
            maybe_assign.is_assign(t) && maybe_assign.get_first_child(t).unwrap().is_name(t);
        let is_module = current_module.is_goog_module() || current_module.is_es6_module();
        if is_filling_an_alias && is_module {
            let alias_name = call
                .get_parent(t)
                .unwrap()
                .get_first_child(t)
                .unwrap()
                .get_string(t);

            // If the assignment isn't into a var in our scope then it's not ok.
            let scope = t.get_scope();
            let alias_var = scope.get_var(t.get_compiler(), alias_name);
            let Some(alias_var) = alias_var else {
                t.report(call, &INVALID_GET_ALIAS, &[]);
                return;
            };

            // Even if it was to a var in our scope it should still only rewrite if the var looked
            // like:
            //   let x = goog.forwardDeclare('a.namespace');
            let alias_var_node = alias_var.get_node(t.get_compiler()).unwrap();
            let alias_var_node_rhs = NodeUtil::get_r_value_of_l_value(t, alias_var_node);

            // NodeUtil.isCallTo(aliasVarNodeRhs, GOOG_FORWARD_DECLARE)
            let is_call_to_forward_declare = |ast: &Ast, n: NodeId| {
                n.is_call(ast)
                    && matches_static(ast, n.get_first_child(ast).unwrap(), &GOOG_FORWARD_DECLARE)
            };
            let Some(alias_var_node_rhs) = alias_var_node_rhs else {
                t.report(call, &INVALID_GET_ALIAS, &[]);
                return;
            };
            if !is_call_to_forward_declare(t, alias_var_node_rhs) {
                t.report(call, &INVALID_GET_ALIAS, &[]);
                return;
            }

            if namespace != alias_var_node_rhs.get_last_child(t).unwrap().get_string(t) {
                t.report(call, &INVALID_GET_ALIAS, &[]);
                #[allow(clippy::needless_return)]
                return;
            }
        }
    }

    // port: CheckClosureImports.Checker#isValidDestructuringImport
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
            }
            if !key.has_children(ast) || !key.get_first_child(ast).unwrap().is_name(ast) {
                return false;
            }
            string_key = key.get_next(ast);
        }
        true
    }

    // port: CheckClosureImports.Checker#checkShortName
    fn check_short_name(t: &mut NodeTraversal<'_>, short_name_node: NodeId, namespace: &JsString) {
        let short_name = short_name_node.get_string(t);
        let last_segment =
            namespace.substring_from((namespace.last_index_of_char(b'.' as u16) + 1) as usize);
        if short_name == last_segment || last_segment.is_empty() {
            return;
        }

        if is_upper_case(short_name.char_at(0)) != is_upper_case(last_segment.char_at(0)) {
            let new_start_char = if is_upper_case(short_name.char_at(0)) {
                to_lower_case(short_name.char_at(0))
            } else {
                to_upper_case(short_name.char_at(0))
            };
            let corrected_name =
                JsString::from_units(vec![new_start_char]).concat(&short_name.substring_from(1));
            t.report(
                short_name_node,
                &INCORRECT_SHORTNAME_CAPITALIZATION,
                &[&short_name.to_string(), &corrected_name.to_string()],
            );
        }
    }

    // port: CheckClosureImports.Checker#checkImport
    fn check_import(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call: NodeId,
        parent: NodeId,
        current_module: &Arc<ModuleMetadata>,
        import_type: ClosureImport,
    ) {
        let at_top_level_scope = t.in_global_hoist_scope() || t.in_module_scope();
        let is_module = current_module.is_module();
        let valid_assignment = is_module || parent.is_expr_result(t);
        let is_aliased = NodeUtil::is_name_declaration(t, parent.get_parent(t));

        // goog.requireDynamic() can be in non-top scope.
        if (!at_top_level_scope || !valid_assignment)
            && import_type != ClosureImport::REQUIRE_DYNAMIC
        {
            t.report(call, &INVALID_CLOSURE_CALL_SCOPE_ERROR, &[]);
            return;
        }

        if !call.has_two_children(t) || !call.get_second_child(t).unwrap().is_string_lit(t) {
            t.report(
                call,
                &INVALID_CLOSURE_IMPORT_CALL,
                &[import_type.call_name()],
            );
            return;
        }

        if is_module && is_aliased {
            let declaration = parent.get_parent(t).unwrap();

            if !declaration.has_one_child(t) {
                t.report(
                    call,
                    &ONE_CLOSURE_IMPORT_PER_DECLARATION,
                    &[import_type.call_name()],
                );
                return;
            }

            if import_type.alias_must_be_constant() {
                if declaration.is_let(t) {
                    t.report(call, &LET_CLOSURE_IMPORT, &[import_type.call_name()]);
                } else if !declaration.is_const(t) && current_module.is_es6_module() {
                    t.report(
                        call,
                        &LHS_OF_CLOSURE_IMPORT_MUST_BE_CONST_IN_ES_MODULE,
                        &[import_type.call_name()],
                    );
                }
            }

            let lhs = declaration.get_first_child(t).unwrap();
            if lhs.is_destructuring_lhs(t) {
                if import_type.allow_destructuring() {
                    if !Self::is_valid_destructuring_import(t, lhs) {
                        t.report(
                            declaration,
                            &INVALID_CLOSURE_IMPORT_DESTRUCTURING,
                            &[import_type.call_name()],
                        );
                    }
                } else {
                    t.report(
                        declaration,
                        &NO_CLOSURE_IMPORT_DESTRUCTURING,
                        &[import_type.call_name()],
                    );
                }
            } else if lhs.is_name(t) {
                let namespace = call.get_last_child(t).unwrap().get_string(t);
                Self::check_short_name(t, lhs, &namespace);
            }
        }

        let namespace = call.get_second_child(t).unwrap().get_string(t);
        let required_module = self
            .pass
            .module_metadata_map
            .get_modules_by_goog_namespace()
            .get(&namespace)
            .cloned();

        let Some(required_module) = required_module else {
            if import_type == ClosureImport::FORWARD_DECLARE {
                // Ok to forwardDeclare any global, sadly. This is some bad legacy behavior and
                // people ought to use externs.
                if is_aliased && is_module {
                    // Special case `const Foo = goog.forwardDeclare('a.Foo');`. For legacy reasons,
                    // we allow js_library to be less strict about missing forwardDeclares vs.
                    // other missing Closure imports.
                    t.report(
                        call,
                        &MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE,
                        &[&namespace.to_string()],
                    );
                }
                return;
            }
            t.report(call, &MISSING_MODULE_OR_PROVIDE, &[&namespace.to_string()]);
            return;
        };

        if import_type.must_be_ordered() {
            self.pass
                .verify_require_order(&namespace, call, t, &required_module);
        }

        if import_type == ClosureImport::REQUIRE
            && current_module.is_es6_module()
            && required_module.is_es6_module()
        {
            t.report(call, &SHOULD_IMPORT_ES6_MODULE, &[]);
        }
    }
}
