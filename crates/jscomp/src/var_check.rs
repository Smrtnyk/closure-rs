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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/VarCheck.java.

//! Port of `VarCheck.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::abstract_scope::ImplicitVar;
use crate::check_level::CheckLevel;
use crate::closure_coding_convention::matches_qualified_name_pattern;
use crate::compiler_input::CompilerInput;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::scope::ScopeId;
use crate::source_file::bundled_script::BundledScript;
use crate::syntactic_scope_creator::{RedeclarationHandler, SyntacticScopeCreator};
use crate::type_validator::TypeValidator;
use closure_rhino::check_state;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::SnapshotUse;
use closure_rhino::node::{Ast, NodeId, Prop};
use closure_rhino::static_source_file::SourceKind;
use closure_rhino::token::Token;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

// port: VarCheck#UNDEFINED_VAR_ERROR
pub static UNDEFINED_VAR_ERROR: DiagnosticType =
    DiagnosticType::error("JSC_UNDEFINED_VARIABLE", "variable {0} is undeclared");

// port: VarCheck#VIOLATED_CHUNK_DEP_ERROR
pub static VIOLATED_CHUNK_DEP_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_VIOLATED_CHUNK_DEPENDENCY",
    "chunk {0} cannot reference {2}, defined in chunk {1}, since {1} loads after {0}",
);

// port: VarCheck#MISSING_CHUNK_DEP_ERROR
pub static MISSING_CHUNK_DEP_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_CHUNK_DEPENDENCY",
    "missing chunk dependency; chunk {0} should depend on chunk {1} because it references {2}",
);

// port: VarCheck#STRICT_CHUNK_DEP_ERROR
pub static STRICT_CHUNK_DEP_ERROR: DiagnosticType = DiagnosticType::disabled(
    "JSC_STRICT_CHUNK_DEPENDENCY",
    "cannot reference {2} because of a missing chunk dependency\ndefined in chunk {1}, referenced from chunk {0}",
);

// port: VarCheck#NAME_REFERENCE_IN_EXTERNS_ERROR
pub static NAME_REFERENCE_IN_EXTERNS_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_NAME_REFERENCE_IN_EXTERNS",
    "accessing name {0} in externs has no effect. Perhaps you forgot to add a var keyword?",
);

// port: VarCheck#UNDEFINED_EXTERN_VAR_ERROR
pub static UNDEFINED_EXTERN_VAR_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_UNDEFINED_EXTERN_VAR_ERROR",
    "name {0} is not defined in the externs.",
);

// port: VarCheck#VAR_MULTIPLY_DECLARED_ERROR
pub static VAR_MULTIPLY_DECLARED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_VAR_MULTIPLY_DECLARED_ERROR",
    "Variable {0} declared more than once. First occurrence: {1}",
);

// port: VarCheck#BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR
pub static BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR",
    "Block-scoped variable {0} declared more than once. First occurrence: {1}",
);

// port: VarCheck#VAR_ARGUMENTS_SHADOWED_ERROR
pub static VAR_ARGUMENTS_SHADOWED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_VAR_ARGUMENTS_SHADOWED_ERROR",
    "Shadowing \"arguments\" is not allowed",
);

// The arguments variable is special, in that it's declared in every local
// scope, but not explicitly declared.
// port: VarCheck#ARGUMENTS
const ARGUMENTS: &str = "arguments";
// The IRFactory parse step adds references to this special name for @closureUnaware support, but
// it's not actually defined anywhere - just a compiler internal implementation detail.
// port: VarCheck#JSCOMP_CLOSURE_UNAWARE_CODE_SHADOW_HOST_NAME
const JSCOMP_CLOSURE_UNAWARE_CODE_SHADOW_HOST_NAME: &str = "$jscomp_wrap_closure_unaware_code";

/// List of symbols that must always be externed even if they are not referenced anywhere (yet).
/// These are used by runtime libraries that might not be present when the first VarCheck runs.
// port: VarCheck#REQUIRED_SYMBOLS
pub static REQUIRED_SYMBOLS: [&str; 33] = [
    // go/keep-sorted start
    "AggregateError",
    "Array",
    "Error",
    "Float32Array",
    "Function",
    "Infinity",
    "Iterator",
    "JSCOMPILER_PRESERVE", // added by CheckSideEffects
    "JSCompiler_renameProperty",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "RangeError",
    "ReferenceError",
    "Reflect",
    "RegExp",
    "Set",
    "String",
    "SuppressedError",
    "Symbol",
    "TypeError",
    "WeakMap",
    "global",
    "globalThis",
    "isNaN",
    "parseFloat",
    "parseInt",
    "self",
    "undefined",
    "window",
    // go/keep-sorted end
];

/// Checks that all variables are declared, that file-private variables are accessed only in the
/// file that declares them, and that any var references that cross chunk boundaries respect
/// declared chunk dependencies.
pub struct VarCheck {
    // Vars that were referenced in the externs without being declared in externs, even if they
    // were defined in code. These will be declared at the end of this pass.
    undefined_names_from_externs: IndexSet<JsString>,

    // Whether this is the post-processing validity check.
    validity_check: bool,

    // Whether extern checks emit error.
    strict_extern_check: bool,

    dup_handler: Option<RedeclarationCheckHandler>,
}

impl VarCheck {
    // port: VarCheck#VarCheck(AbstractCompiler)
    pub fn new(compiler: &AbstractCompiler) -> Self {
        Self::new_with_validity_check(compiler, false)
    }

    // port: VarCheck#VarCheck(AbstractCompiler, boolean)
    pub fn new_with_validity_check(compiler: &AbstractCompiler, validity_check: bool) -> Self {
        let strict_extern_check = compiler.get_error_level(&JSError::make_with_source_location(
            "",
            0,
            0,
            &UNDEFINED_EXTERN_VAR_ERROR,
            &[],
        )) == CheckLevel::ERROR;
        Self {
            undefined_names_from_externs: IndexSet::<_>::default(),
            validity_check,
            strict_extern_check,
            dup_handler: None,
        }
    }

    /// Creates the scope creator used by this pass. If not in validity check mode, use a
    /// `RedeclarationCheckHandler` to check var redeclarations.
    // port: VarCheck#createScopeCreator
    fn create_scope_creator(&mut self) -> SyntacticScopeCreator<'static> {
        if self.validity_check {
            SyntacticScopeCreator::new()
        } else {
            let dup_handler = RedeclarationCheckHandler::new();
            self.dup_handler = Some(dup_handler.clone());
            SyntacticScopeCreator::new_with_redeclaration_handler(Box::new(dup_handler))
        }
    }

    /// Validates that a NAME node does not refer to an undefined name.
    // port: VarCheck#checkName
    fn check_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // Rust-only: the name is read in place (performance divergence).
        // Only a function can have an empty name.
        if n.get_string_ref(t).is_empty() {
            // Name is optional for function expressions
            // x = function() {...}
            // Arrow functions are also expressions and cannot have a name
            // x = () => {...}
            // Member functions have an empty NAME node string, because the actual name is stored
            // on the MEMBER_FUNCTION_DEF object that contains the FUNCTION.
            // class C { foo() {...} }
            // x = { foo() {...} }
            let parent = parent.unwrap();
            check_state!(
                NodeUtil::is_function_expression(t, parent)
                    || NodeUtil::is_method_declaration(t, parent)
            );
            return;
        }

        // In `#x in obj`, IRFactory represents `#x` as a NAME node and validates at parse time
        // that it is inside a class and declared by an enclosing class. SyntacticScopeCreator only
        // tracks variables, not class members, so skip `Scope` lookup and extern synthesis here.
        if n.is_private_identifier(t) {
            let parent = parent.unwrap();
            check_state!(
                parent.is_in(t) && n.is_first_child_of(t, Some(parent)),
                "%s",
                n.to_string(t)
            );
            return;
        }

        let scope = t.get_scope();
        let var = scope.get_var_of_node(t.get_compiler(), n);

        // Check that the var has been declared.
        let Some(var) = var else {
            let parent = parent.unwrap();
            if (NodeUtil::is_function_expression(t, parent)
                || NodeUtil::is_class_expression(t, parent))
                && n.is_first_child_of(t, Some(parent))
            {
                // e.g. [ function foo() {} ], it's okay if "foo" isn't defined in the
                // current scope.
                return;
            }

            if NodeUtil::is_nonlocal_module_export_name(t, n) {
                // e.g. "export {a as b}" or "import {b as a} from './foo.js'
                // where b is defined in a module's export entries but not in any module scope.
                return;
            }

            self.handle_undeclared_variable_ref(t, n);
            let compiler = t.get_compiler();
            let global_scope = scope.get_global_scope(compiler);
            let input = compiler.get_synthesized_externs_input().clone();
            global_scope.declare(compiler, n.get_string(compiler), n, Some(input));

            return;
        };

        let compiler = t.get_compiler();
        if var.is_implicit_goog_namespace(compiler)
            && var.get_implicit_goog_namespace_strength(compiler) == SourceKind::WEAK
            && Self::strength_of(compiler, n) == SourceKind::STRONG
        {
            // This use will be retained but its definition will be deleted.
            self.handle_undeclared_variable_ref(t, n);
            return;
        }

        let curr_input: Option<CompilerInput> = t.get_input().cloned();
        let var_input: Option<CompilerInput> = var.get_input(t.get_compiler());
        let (Some(curr_input), Some(var_input)) = (curr_input, var_input) else {
            // The variable was defined in the same file. This is fine.
            return;
        };
        if curr_input == var_input {
            // The variable was defined in the same file. This is fine.
            return;
        }

        // Check chunk dependencies.
        let curr_chunk = curr_input.get_chunk();
        let var_chunk = var_input.get_chunk();
        if !self.validity_check
            && var_chunk != curr_chunk
            && let (Some(var_chunk), Some(curr_chunk)) = (var_chunk, curr_chunk)
        {
            if var_chunk.is_weak() {
                self.handle_undeclared_variable_ref(t, n);
            }

            let chunk_graph = t.get_compiler().get_chunk_graph().unwrap();
            let curr_depends_on_var = chunk_graph.depends_on(&curr_chunk, &var_chunk);
            // Read here, where Java reads it, because the report below needs the traversal.
            let var_depends_on_curr = chunk_graph.depends_on(&var_chunk, &curr_chunk);
            if curr_depends_on_var {
                // The chunk dependency was properly declared.
            } else {
                let var_name = n.get_string_ref(t).to_string();
                let args = [
                    curr_chunk.get_name(),
                    var_chunk.get_name(),
                    var_name.clone(),
                ];
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                if scope.is_global(t.get_compiler()) {
                    if var_depends_on_curr {
                        // The variable reference violates a declared chunk dependency.
                        t.report(n, &VIOLATED_CHUNK_DEP_ERROR, &args);
                    } else {
                        // The variable reference is between two chunks that have no dependency
                        // relationship. This should probably be considered an error, but just
                        // issue a warning for now.
                        t.report(n, &MISSING_CHUNK_DEP_ERROR, &args);
                    }
                } else {
                    t.report(n, &STRICT_CHUNK_DEP_ERROR, &args);
                }
            }
        }
    }

    // port: VarCheck#strengthOf
    fn strength_of(ast: &Ast, n: NodeId) -> SourceKind {
        let Some(source) = n.get_static_source_file(ast) else {
            return SourceKind::EXTERN;
        };

        source.get_kind()
    }

    // port: VarCheck#handleUndeclaredVariableRef
    fn handle_undeclared_variable_ref(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(n.is_name(t));

        let var_name = n.get_string(t);

        if n.get_parent(t).unwrap().is_type_of(t) {
            // `typeof` is used for existence checks.
        } else if var_name == JSCOMP_CLOSURE_UNAWARE_CODE_SHADOW_HOST_NAME {
            // Compiler internal, we just create an extern for it.
        } else if self.strict_extern_check && t.get_input().unwrap().is_extern() {
            // The extern checks are stricter, don't report a second error.
        } else {
            t.report(n, &UNDEFINED_VAR_ERROR, &[&var_name.to_string()]);
        }

        if self.validity_check {
            // When the code is initially traversed, any undeclared variables are treated as
            // externs. During this sanity check, we ensure that all variables have either been
            // declared or marked as an extern. A failure at this point means that we have created
            // some variable/generated some code with an undefined reference.
            panic!("Unexpected variable {var_name}");
        } else if !self.undefined_names_from_externs.contains(&var_name) {
            // Skip this case if the name is already going to be added as an "undefined name from
            // extern". That declaration must take priority, to avoid the
            // RemoveUnnecessarySyntheticExterns pass accidentally treating the synthetic extern as
            // unnecessary.
            Self::create_synthesized_extern_var(
                t.get_compiler(),
                var_name,
                /* is_from_undefined_code_ref= */ true,
            );
        }
    }

    /// Create a new variable in a synthetic script. This will prevent subsequent compiler passes
    /// from crashing.
    // port: VarCheck#createSynthesizedExternVar
    fn create_synthesized_extern_var(
        compiler: &mut AbstractCompiler,
        var_name: JsString,
        is_from_undefined_code_ref: bool,
    ) {
        let name_node = IR::name(compiler, var_name.clone());

        // Mark the variable as constant if it matches the coding convention
        // for constant vars.
        // NOTE(nicksantos): honestly, I'm not sure how much this matters.
        // AFAIK, all people who use the CONST coding convention also
        // compile with undeclaredVars as errors. We have some test
        // cases for this configuration though, and it makes them happier.
        if compiler.get_coding_convention().is_constant(&var_name) {
            name_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        }

        let synthetic_extern_var = IR::var(compiler, name_node);
        synthetic_extern_var
            .set_is_synthesized_unfulfilled_name_declaration(compiler, is_from_undefined_code_ref);
        let root = Self::get_synthesized_externs_root(compiler);
        root.add_child_to_back(compiler, synthetic_extern_var);
        compiler.report_change_to_enclosing_scope(synthetic_extern_var);
    }

    /// Returns true if duplication warnings are suppressed on either n or origVar.
    // port: VarCheck#hasDuplicateDeclarationSuppression
    pub fn has_duplicate_declaration_suppression(
        compiler: &AbstractCompiler,
        n: NodeId,
        orig_var: NodeId,
    ) -> bool {
        // For VarCheck and VariableReferenceCheck, variables in externs do not generate duplicate
        // warnings.
        if Self::is_extern_namespace(compiler, n) {
            return true;
        }
        TypeValidator::has_duplicate_declaration_suppression(compiler, orig_var)
    }

    /// Returns true if n is the name of a variable that declares a namespace in an externs file.
    // port: VarCheck#isExternNamespace
    pub fn is_extern_namespace(ast: &Ast, n: NodeId) -> bool {
        n.get_parent(ast).unwrap().is_var(ast)
            && n.is_from_externs(ast)
            && NodeUtil::is_namespace_decl(ast, n)
    }

    // port: VarCheck#reportVarMultiplyDeclared
    pub fn report_var_multiply_declared(
        compiler: &mut AbstractCompiler,
        current: NodeId,
        name: &str,
        original: Option<NodeId>,
    ) {
        let location = Self::location_of(compiler, original);
        let error = JSError::make(
            compiler,
            current,
            &VAR_MULTIPLY_DECLARED_ERROR,
            &[name, &location],
        );
        compiler.report(error);
    }

    // port: VarCheck#reportBlockScopedMultipleDeclaration
    fn report_block_scoped_multiple_declaration(
        compiler: &mut AbstractCompiler,
        current: NodeId,
        name: &str,
        original: Option<NodeId>,
    ) {
        let location = Self::location_of(compiler, original);
        let error = JSError::make(
            compiler,
            current,
            &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
            &[name, &location],
        );
        compiler.report(error);
    }

    // port: VarCheck#locationOf
    fn location_of(ast: &Ast, n: Option<NodeId>) -> String {
        match n {
            None => "<unknown>".to_string(),
            Some(n) => n.get_location(ast),
        }
    }

    /// Lazily create a "new" externs root for undeclared variables.
    // port: VarCheck#getSynthesizedExternsRoot
    fn get_synthesized_externs_root(compiler: &mut AbstractCompiler) -> NodeId {
        let input = compiler.get_synthesized_externs_input().clone();
        input.get_ast_root(compiler)
    }
}

impl CompilerPass for VarCheck {
    // port: VarCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if self.validity_check && try_parallel_validity_check(compiler, externs, root) {
            return;
        }
        let mut scope_creator = self.create_scope_creator();

        // Don't run externs-checking in sanity check mode. Normalization will
        // remove duplicate VAR declarations, which will make
        // externs look like they have assigns.
        if !self.validity_check {
            let mut name_ref_in_externs_check = NameRefInExternsCheck {
                undefined_names_from_externs: &mut self.undefined_names_from_externs,
            };
            NodeTraversal::builder()
                .set_compiler(compiler)
                .set_callback(&mut name_ref_in_externs_check)
                .set_scope_creator(&mut scope_creator)
                .traverse(externs);
        }

        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .set_scope_creator(&mut scope_creator)
            .traverse_roots(externs, root);

        for var_name in self.undefined_names_from_externs.clone() {
            Self::create_synthesized_extern_var(
                compiler, var_name, /* is_from_undefined_code_ref= */ false,
            );
        }

        if let Some(dup_handler) = &self.dup_handler {
            dup_handler.remove_duplicates(compiler);
        }
    }
}

const IMPLICIT_ARGUMENTS: u8 = 1 << 0;
const IMPLICIT_THIS_SUPER: u8 = 1 << 1;
const IMPLICIT_EXPORTS: u8 = 1 << 2;

enum ValidityUnit {
    Subtree(NodeId, Option<NodeId>),
    StmtSlice(NodeId, usize, usize),
}

/// Rust-only (performance divergence): verifies `VarCheck` validity-check invariants read-only
/// on `&Ast` (skipping verified bundled extern prefixes via `BundledScript::extern_referenced_names`
/// and running across scoped worker threads), returning `false` on any violation or edge case so
/// the caller falls back to the exact sequential `NodeTraversal` for byte-identical diagnostics
/// and panics.
fn try_parallel_validity_check(
    compiler: &mut AbstractCompiler,
    externs: NodeId,
    root: NodeId,
) -> bool {
    let Some(global_root) = externs
        .get_parent(compiler)
        .filter(|&p| root.get_parent(compiler) == Some(p))
    else {
        return false;
    };
    let mut scope_creator = SyntacticScopeCreator::new();
    let global_scope = scope_creator.create_scope(compiler, global_root, None);
    let (global_vars, weak_implicit_goog) = global_scope.collect_global_var_names(compiler);

    let mut units: Vec<ValidityUnit> = Vec::new();
    let mut stmts: Vec<NodeId> = Vec::new();

    // Collect externs work units, skipping verified bundled extern prefixes.
    let mut extern_child = externs.get_first_child(compiler);
    while let Some(child) = extern_child {
        if child.is_script(compiler)
            && let Some((facts, suffix)) = compiler
                .parsed_script_record::<BundledScript>(child, SnapshotUse::SyntaxAndNonParamNames)
            && let Some(ref_names) = facts.extern_referenced_names
        {
            let prefix_ok = ref_names.iter().all(|&name| {
                let js = JsString::from(name);
                global_vars.contains(&js) && !weak_implicit_goog.contains(&js)
            });
            if prefix_ok {
                let mut st = suffix;
                while let Some(s) = st {
                    units.push(ValidityUnit::Subtree(s, Some(child)));
                    st = s.get_next(compiler);
                }
                extern_child = child.get_next(compiler);
                continue;
            }
        }
        units.push(ValidityUnit::Subtree(child, Some(externs)));
        extern_child = child.get_next(compiler);
    }

    // Collect code root work units, chunking large non-module scripts by top-level statement so
    // single-bundle compilations also scale across cores.
    let mut code_child = root.get_first_child(compiler);
    while let Some(child) = code_child {
        if child.is_script(compiler)
            && let Some(first) = child.get_first_child(compiler)
            && !first.is_module_body(compiler)
        {
            let start = stmts.len();
            let mut st = Some(first);
            while let Some(s) = st {
                stmts.push(s);
                st = s.get_next(compiler);
            }
            let count = stmts.len() - start;
            const STMT_CHUNK: usize = 64;
            if count > STMT_CHUNK {
                let mut offset = start;
                while offset < stmts.len() {
                    let end = (offset + STMT_CHUNK).min(stmts.len());
                    units.push(ValidityUnit::StmtSlice(child, offset, end));
                    offset = end;
                }
            } else {
                units.push(ValidityUnit::StmtSlice(child, start, stmts.len()));
            }
        } else {
            units.push(ValidityUnit::Subtree(child, Some(root)));
        }
        code_child = child.get_next(compiler);
    }

    if units.is_empty() {
        return true;
    }

    let ast = &compiler.ast;
    let threads = crate::parallel_parse::parallel_thread_count(units.len(), 2, 8);
    let ok = AtomicBool::new(true);
    let global_vars_ref = &global_vars;
    let weak_ref = &weak_implicit_goog;
    let stmts_ref = &stmts;

    if threads < 2 {
        let mut checker = ValidityChecker::new(global_vars_ref, weak_ref);
        for unit in &units {
            if !checker.check_unit(ast, unit, stmts_ref) {
                return false;
            }
        }
        return true;
    }

    crate::parallel_parse::run_scoped_for_each(
        "closure-varcheck",
        threads,
        &units,
        |_idx, unit| {
            if !ok.load(Ordering::Relaxed) {
                return;
            }
            let mut checker = ValidityChecker::new(global_vars_ref, weak_ref);
            if !checker.check_unit(ast, unit, stmts_ref) {
                ok.store(false, Ordering::Relaxed);
            }
        },
    );
    ok.load(Ordering::Relaxed)
}

struct ValidityChecker<'a> {
    global_vars: &'a IndexSet<JsString>,
    weak_implicit_goog: &'a IndexSet<JsString>,
    local_counts: IndexMap<&'a JsString, u32>,
    undo_log: Vec<&'a JsString>,
    implicit_mask: u8,
}

impl<'a> ValidityChecker<'a> {
    fn new(
        global_vars: &'a IndexSet<JsString>,
        weak_implicit_goog: &'a IndexSet<JsString>,
    ) -> Self {
        Self {
            global_vars,
            weak_implicit_goog,
            local_counts: IndexMap::default(),
            undo_log: Vec::new(),
            implicit_mask: 0,
        }
    }

    fn check_unit(&mut self, ast: &'a Ast, unit: &ValidityUnit, stmts: &[NodeId]) -> bool {
        match *unit {
            ValidityUnit::Subtree(node, parent) => self.check_node(ast, node, parent),
            ValidityUnit::StmtSlice(script, start, end) => {
                for &stmt in &stmts[start..end] {
                    if !self.check_node(ast, stmt, Some(script)) {
                        return false;
                    }
                }
                true
            }
        }
    }

    #[inline]
    fn push_local(&mut self, name: &'a JsString) {
        *self.local_counts.entry(name).or_default() += 1;
        self.undo_log.push(name);
    }

    #[inline]
    fn pop_to(&mut self, mark: usize) {
        while self.undo_log.len() > mark {
            let name = self.undo_log.pop().unwrap();
            if let Some(count) = self.local_counts.get_mut(name) {
                *count -= 1;
            }
        }
    }

    fn declare_lhs(&mut self, ast: &'a Ast, n: NodeId) -> bool {
        if n.has_one_child(ast)
            && let Some(first) = n.get_first_child(ast)
            && first.is_name(ast)
        {
            let Some(s) = first.string_payload(ast) else {
                return false;
            };
            self.push_local(s);
            return true;
        }
        let mut valid = true;
        NodeUtil::visit_lhs_nodes_in_node_readonly(ast, n, &mut |lhs| {
            if !valid {
                return;
            }
            if !(lhs.is_name(ast) || lhs.is_import_star(ast)) {
                valid = false;
                return;
            }
            let Some(s) = lhs.string_payload(ast) else {
                valid = false;
                return;
            };
            self.push_local(s);
        });
        valid
    }

    /// Mirrors `SyntacticScopeCreator.ScopeScanner#scanVars`.
    fn scan_vars(
        &mut self,
        ast: &'a Ast,
        scope_root: NodeId,
        n: NodeId,
        hoist: bool,
        block: bool,
    ) -> bool {
        match n.get_token(ast) {
            Token::VAR => {
                if hoist && !self.declare_lhs(ast, n) {
                    return false;
                }
                return true;
            }
            Token::LET | Token::CONST => {
                if block && !self.declare_lhs(ast, n) {
                    return false;
                }
                return true;
            }
            Token::IMPORT => {
                if !hoist {
                    return false;
                }
                return self.declare_lhs(ast, n);
            }
            Token::EXPORT => {
                let Some(decl) = n.get_first_child(ast) else {
                    return false;
                };
                return self.scan_vars(ast, scope_root, decl, hoist, block);
            }
            Token::FUNCTION => {
                if NodeUtil::is_function_expression(ast, n) || !block {
                    return true;
                }
                let Some(fn_name) = n.get_first_child(ast) else {
                    return false;
                };
                let Some(s) = fn_name.string_payload(ast) else {
                    return false;
                };
                if !s.is_empty() {
                    self.push_local(s);
                }
                return true;
            }
            Token::CLASS => {
                if NodeUtil::is_class_expression(ast, n) || !block {
                    return true;
                }
                let Some(class_name) = n.get_first_child(ast) else {
                    return false;
                };
                let Some(s) = class_name.string_payload(ast) else {
                    return false;
                };
                if !s.is_empty() {
                    self.push_local(s);
                }
                return true;
            }
            Token::CATCH => {
                if !n.has_two_children(ast) {
                    return false;
                }
                if block && !self.declare_lhs(ast, n) {
                    return false;
                }
                let Some(catch_block) = n.get_second_child(ast) else {
                    return false;
                };
                return self.scan_vars(ast, scope_root, catch_block, hoist, block);
            }
            Token::MODULE_BODY if n != scope_root => {
                return false;
            }
            _ => {}
        }

        let is_block_start = block && n == scope_root;
        let entering_new_block = !is_block_start && NodeUtil::creates_block_scope(ast, n);
        if entering_new_block && !hoist {
            return true;
        }
        if NodeUtil::is_shallow_statement_tree(ast, Some(n)) {
            let child_block = if entering_new_block { false } else { block };
            let mut child = n.get_first_child(ast);
            while let Some(c) = child {
                if !self.scan_vars(ast, scope_root, c, hoist, child_block) {
                    return false;
                }
                child = c.get_next(ast);
            }
        }
        true
    }

    #[inline]
    fn is_declared_locally(&self, name: &JsString) -> bool {
        if self.local_counts.get(name).copied().unwrap_or(0) > 0 {
            return true;
        }
        match ImplicitVar::of(name) {
            Some(ImplicitVar::ARGUMENTS) => self.implicit_mask & IMPLICIT_ARGUMENTS != 0,
            Some(ImplicitVar::THIS | ImplicitVar::SUPER) => {
                self.implicit_mask & IMPLICIT_THIS_SUPER != 0
            }
            Some(ImplicitVar::EXPORTS) => self.implicit_mask & IMPLICIT_EXPORTS != 0,
            None => false,
        }
    }

    fn check_node(&mut self, ast: &'a Ast, n: NodeId, parent: Option<NodeId>) -> bool {
        match n.get_token(ast) {
            Token::NAME => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if !self.check_node(ast, c, Some(n)) {
                        return false;
                    }
                    child = c.get_next(ast);
                }
                let Some(s) = n.string_payload(ast) else {
                    return false;
                };
                if s.is_empty() {
                    return parent.is_some_and(|p| {
                        NodeUtil::is_function_expression(ast, p)
                            || NodeUtil::is_method_declaration(ast, p)
                    });
                }
                if n.is_private_identifier(ast) {
                    return parent
                        .is_some_and(|p| p.is_in(ast) && n.is_first_child_of(ast, Some(p)));
                }
                if self.is_declared_locally(s) {
                    return true;
                }
                if self.global_vars.contains(s) {
                    if !self.weak_implicit_goog.is_empty()
                        && self.weak_implicit_goog.contains(s)
                        && VarCheck::strength_of(ast, n) == SourceKind::STRONG
                    {
                        return false;
                    }
                    return true;
                }
                if parent.is_some_and(|p| {
                    (NodeUtil::is_function_expression(ast, p)
                        || NodeUtil::is_class_expression(ast, p))
                        && n.is_first_child_of(ast, Some(p))
                }) {
                    return true;
                }
                NodeUtil::is_nonlocal_module_export_name(ast, n)
            }
            Token::FUNCTION => {
                let Some(fn_name) = n.get_first_child(ast) else {
                    return false;
                };
                let Some(args) = fn_name.get_next(ast) else {
                    return false;
                };
                let Some(body) = args.get_next(ast) else {
                    return false;
                };
                if !args.is_param_list(ast) {
                    return false;
                }
                let is_decl = parent.is_some() && NodeUtil::is_function_declaration(ast, n);
                if is_decl && !self.check_node(ast, fn_name, Some(n)) {
                    return false;
                }
                let mark = self.undo_log.len();
                let prev_implicit = self.implicit_mask;
                if !n.is_arrow_function(ast) {
                    self.implicit_mask |= IMPLICIT_ARGUMENTS | IMPLICIT_THIS_SUPER;
                }
                let mut valid = self.declare_lhs(ast, args);
                if valid && NodeUtil::is_function_expression(ast, n) {
                    if let Some(s) = fn_name.string_payload(ast) {
                        if !s.is_empty() {
                            self.push_local(s);
                        }
                    } else {
                        valid = false;
                    }
                }
                if valid && !is_decl {
                    valid = self.check_node(ast, fn_name, Some(n));
                }
                if valid {
                    valid = self.check_node(ast, args, Some(n));
                }
                if valid {
                    valid = self.check_node(ast, body, Some(n));
                }
                self.implicit_mask = prev_implicit;
                self.pop_to(mark);
                valid
            }
            Token::CLASS => {
                let Some(class_name) = n.get_first_child(ast) else {
                    return false;
                };
                let Some(extends_clause) = class_name.get_next(ast) else {
                    return false;
                };
                let Some(body) = extends_clause.get_next(ast) else {
                    return false;
                };
                let is_expr = NodeUtil::is_class_expression(ast, n);
                if !self.check_node(ast, extends_clause, Some(n)) {
                    return false;
                }
                if !is_expr && !self.check_node(ast, class_name, Some(n)) {
                    return false;
                }
                let mark = self.undo_log.len();
                let mut valid = true;
                if is_expr && !class_name.is_empty(ast) {
                    if let Some(s) = class_name.string_payload(ast) {
                        self.push_local(s);
                    } else {
                        valid = false;
                    }
                }
                if valid && is_expr {
                    valid = self.check_node(ast, class_name, Some(n));
                }
                if valid {
                    valid = self.check_node(ast, body, Some(n));
                }
                self.pop_to(mark);
                valid
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if c.is_computed_prop(ast) || c.is_computed_field_def(ast) {
                        let Some(key) = c.get_first_child(ast) else {
                            return false;
                        };
                        if !self.check_node(ast, key, Some(c)) {
                            return false;
                        }
                    }
                    child = c.get_next(ast);
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    match c.get_token(ast) {
                        Token::COMPUTED_PROP => {
                            let Some(val) = c.get_last_child(ast) else {
                                return false;
                            };
                            if !self.check_node(ast, val, Some(c)) {
                                return false;
                            }
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(ast) {
                                let prev = self.implicit_mask;
                                self.implicit_mask |= IMPLICIT_THIS_SUPER;
                                let ok =
                                    self.check_node(ast, c.get_last_child(ast).unwrap(), Some(c));
                                self.implicit_mask = prev;
                                if !ok {
                                    return false;
                                }
                            }
                        }
                        Token::MEMBER_FIELD_DEF => {
                            if let Some(init) = c.get_first_child(ast) {
                                let prev = self.implicit_mask;
                                self.implicit_mask |= IMPLICIT_THIS_SUPER;
                                let ok = self.check_node(ast, init, Some(c));
                                self.implicit_mask = prev;
                                if !ok {
                                    return false;
                                }
                            }
                        }
                        _ => {
                            if !self.check_node(ast, c, Some(n)) {
                                return false;
                            }
                        }
                    }
                    child = c.get_next(ast);
                }
                true
            }
            Token::MODULE_BODY => {
                let mark = self.undo_log.len();
                let prev_implicit = self.implicit_mask;
                if parent.is_some_and(|p| p.get_boolean_prop(ast, NodeId::GOOG_MODULE)) {
                    self.implicit_mask |= IMPLICIT_EXPORTS;
                }
                let mut valid = self.scan_vars(ast, n, n, true, true);
                if valid {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        if !self.check_node(ast, c, Some(n)) {
                            valid = false;
                            break;
                        }
                        child = c.get_next(ast);
                    }
                }
                self.implicit_mask = prev_implicit;
                self.pop_to(mark);
                valid
            }
            Token::BLOCK
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY
                if NodeUtil::creates_block_scope(ast, n) =>
            {
                let mark = self.undo_log.len();
                let prev_implicit = self.implicit_mask;
                let is_static_block = n.is_block(ast) && NodeUtil::is_class_static_block(ast, n);
                let hoist =
                    is_static_block || (n.is_block(ast) && NodeUtil::is_function_block(ast, n));
                if is_static_block {
                    self.implicit_mask |= IMPLICIT_THIS_SUPER;
                }
                let mut valid = self.scan_vars(ast, n, n, hoist, true);
                if valid {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        if !self.check_node(ast, c, Some(n)) {
                            valid = false;
                            break;
                        }
                        child = c.get_next(ast);
                    }
                }
                self.implicit_mask = prev_implicit;
                self.pop_to(mark);
                valid
            }
            _ => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if !self.check_node(ast, c, Some(n)) {
                        return false;
                    }
                    child = c.get_next(ast);
                }
                true
            }
        }
    }
}

impl Callback for VarCheck {
    // port: VarCheck#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    /// Rust-only (performance divergence): Java visits every NAME of the bundled externs. In an
    /// unchanged `normalize_clean` bundled prefix every function is an empty stub with distinct
    /// plain parameters (its scopes declare each name once, so the redeclaration handler is not
    /// called), its parameters resolve in their own scope and in their own input, and the
    /// remaining names are `extern_referenced_names`, resolved in the global scope. When the
    /// global scope declares all of them, `checkName` finds every var and reports nothing: an
    /// extern input belongs to no chunk, and a NAME in an extern file is not a strong use of a weak
    /// implicit goog namespace. Statements appended to the script are visited as usual.
    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if *crate::variable_reference_check::SKIP_BUNDLED_EXTERNS
            && t.in_global_scope()
            && let Some((facts, first)) =
                t.bundled_script_prefix(script, SnapshotUse::SyntaxAndNonParamNames)
            && facts.normalize_clean
            && let Some(names) = facts.extern_referenced_names
        {
            // Java makes the global scope at the first NAME of the first script it visits, and
            // reports nothing between entering the script and that NAME.
            t.get_scope();
            if t.global_declares_all(names) {
                t.traverse_script_suffix(script, parent, first, self);
                return false;
            }
        }
        self.should_traverse(t, script, parent)
    }

    // port: VarCheck#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) {
            self.check_name(t, n, parent);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for VarCheck {
    // port: VarCheck#enterScope
    fn enter_scope(&mut self, _t: &mut NodeTraversal<'_>) {}

    // port: VarCheck#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if !self.validity_check && t.in_global_scope() {
            let scope = t.get_scope();
            // Add symbols that are known to be needed to the standard injected code (polyfills,
            // etc).
            for required_symbol in REQUIRED_SYMBOLS {
                let var = scope.get_var(t.get_compiler(), required_symbol);
                if var.is_none() {
                    self.undefined_names_from_externs
                        .insert(JsString::from(required_symbol));
                }
            }
        }
    }
}

/// A check for name references in the externs inputs. These used to prevent a variable from
/// getting renamed, but no longer have any effect.
struct NameRefInExternsCheck<'a> {
    undefined_names_from_externs: &'a mut IndexSet<JsString>,
}

// port: VarCheck#googForwardDeclare
/// Rust-only: Java's static node lives in an arena of its own (as CheckClosureImports' node
/// constants do), not in the compilation's, so that a compilation's arena only holds nodes of its
/// code; it is matched with `Node#matchesQualifiedName(Node)` across the two arenas.
static GOOG_FORWARD_DECLARE: LazyLock<(Ast, NodeId)> = LazyLock::new(|| {
    let mut ast = Ast::new();
    let goog = IR::name(&mut ast, "goog");
    let node = IR::getprop(&mut ast, goog, "forwardDeclare");
    (ast, node)
});

impl Callback for NameRefInExternsCheck<'_> {
    // port: VarCheck.NameRefInExternsCheck#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Type summaries are generated from code rather than hand-written,
        // so warning about name references there would usually not be helpful.
        !n.is_script(t) || !NodeUtil::is_from_type_summary(t, n)
    }

    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if NodeUtil::is_from_type_summary(t, script) {
            return false;
        }
        let Some((facts, first)) = t.bundled_script_prefix(script, SnapshotUse::SyntaxAndNames)
        else {
            return true;
        };
        let Some(roots) = facts.global_property_roots else {
            return true;
        };
        if !roots.is_empty() && !t.global_declares_all(roots) {
            return true;
        }
        t.traverse_script_suffix(script, parent, first, self);
        false
    }

    // port: VarCheck.NameRefInExternsCheck#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) {
            let parent = parent.unwrap();
            match parent.get_token(t) {
                Token::VAR
                | Token::LET
                | Token::CONST
                | Token::FUNCTION
                | Token::CLASS
                | Token::PARAM_LIST
                | Token::DEFAULT_VALUE
                | Token::ITER_REST
                | Token::OBJECT_REST
                | Token::ARRAY_PATTERN => {
                    // These are okay.
                    return;
                }
                Token::STRING_KEY => {
                    if parent.get_parent(t).unwrap().is_object_pattern(t) {
                        return;
                    }
                }
                Token::GETPROP => {
                    if Some(n) == parent.get_first_child(t) {
                        let scope = t.get_scope();
                        let name = n.get_string(t);
                        let var = scope.get_var(t.get_compiler(), name);
                        if var.is_some() {
                            return;
                        }
                        if matches_qualified_name_pattern(
                            t,
                            parent,
                            &GOOG_FORWARD_DECLARE.0,
                            GOOG_FORWARD_DECLARE.1,
                        ) {
                            // Allow using `goog.forwardDeclare` in the externs without an externs
                            // definition of goog.
                            return;
                        }
                        let name = n.get_string(t);
                        t.report(n, &UNDEFINED_EXTERN_VAR_ERROR, &[&name.to_string()]);
                        self.undefined_names_from_externs.insert(name);
                    }
                    return;
                }
                Token::ASSIGN => {
                    // Don't warn for the "window.foo = foo;" nodes added by
                    // DeclaredGlobalExternsOnWindow, nor for alias declarations
                    // of the form "/** @const */ ns.Foo = Bar;"
                    if Some(n) == parent.get_last_child(t)
                        && n.is_qualified_name(t)
                        && parent.get_first_child(t).unwrap().is_qualified_name(t)
                    {
                        return;
                    }
                }
                Token::NAME => {
                    // Don't warn for simple var assignments "/** @const */ var foo = bar;"
                    // They are used to infer the types of namespace aliases.
                    if NodeUtil::is_name_declaration(t, parent.get_parent(t)) {
                        return;
                    }
                }
                Token::OR => {
                    // Don't warn for namespace declarations: "/** @const */ var ns = ns || {};"
                    if NodeUtil::is_namespace_decl(t, parent.get_parent(t).unwrap()) {
                        return;
                    }
                }
                _ => {}
            }
            let name = n.get_string(t);
            t.report(n, &NAME_REFERENCE_IN_EXTERNS_ERROR, &[&name.to_string()]);
            let scope = t.get_scope();
            let var = scope.get_var(t.get_compiler(), name.clone());
            if var.is_none() {
                self.undefined_names_from_externs.insert(name);
            }
        }
    }
}

/// The handler for duplicate declarations.
///
/// Java's VarCheck keeps the handler it gave the scope creator in its `dupHandler` field; the
/// Rust scope creator owns its handler, so the two copies share the node list.
#[derive(Clone)]
struct RedeclarationCheckHandler {
    dup_decl_nodes: Rc<RefCell<Vec<NodeId>>>,
}

impl RedeclarationCheckHandler {
    fn new() -> Self {
        Self {
            dup_decl_nodes: Rc::new(RefCell::new(Vec::new())),
        }
    }

    // port: VarCheck.RedeclarationCheckHandler#removeDuplicates
    fn remove_duplicates(&self, compiler: &mut AbstractCompiler) {
        for n in self.dup_decl_nodes.borrow().iter().copied() {
            let parent = n.get_parent(compiler);
            if let Some(parent) = parent {
                n.detach(compiler);
                compiler.report_change_to_enclosing_scope(parent);
            }
        }
    }
}

impl RedeclarationHandler for RedeclarationCheckHandler {
    // port: VarCheck.RedeclarationCheckHandler#onRedeclaration
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn on_redeclaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        s: ScopeId,
        name: &JsString,
        n: NodeId,
        _input: Option<CompilerInput>,
    ) {
        let parent = NodeUtil::get_declaring_parent(compiler, n);
        let orig_var = s.get_var(compiler, name.clone()).unwrap();
        // origNode will be null for `arguments`, since there's no node that declares it.
        let orig_node = orig_var.get_node(compiler);
        let orig_parent =
            orig_node.map(|orig_node| NodeUtil::get_declaring_parent(compiler, orig_node));
        let name_str = name.to_string();

        match parent.get_token(compiler) {
            Token::CLASS | Token::CONST | Token::LET => {
                VarCheck::report_block_scoped_multiple_declaration(
                    compiler, n, &name_str, orig_node,
                );
                return;
            }
            _ => {}
        }

        if let Some(orig_parent) = orig_parent {
            match orig_parent.get_token(compiler) {
                Token::CLASS | Token::CONST | Token::LET => {
                    VarCheck::report_block_scoped_multiple_declaration(
                        compiler, n, &name_str, orig_node,
                    );
                    return;
                }
                Token::FUNCTION => {
                    // Redeclarations of functions in global scope are fairly common, so allow them
                    // (at least for now).
                    if !s.is_global(compiler) && parent.is_function(compiler) {
                        VarCheck::report_block_scoped_multiple_declaration(
                            compiler, n, &name_str, orig_node,
                        );
                        return;
                    }
                }
                _ => {}
            }
        }

        // Don't allow multiple variables to be declared at the top-level scope
        if s.is_global(compiler) {
            // Java dereferences origParent here (NullPointerException when it is null).
            if orig_parent.unwrap().is_catch(compiler) && parent.is_catch(compiler) {
                // Okay, both are 'catch(x)' variables.
                return;
            }

            let allow_dupe = VarCheck::has_duplicate_declaration_suppression(
                compiler,
                n,
                orig_var.get_name_node(compiler).unwrap(),
            );
            if VarCheck::is_extern_namespace(compiler, n) {
                self.dup_decl_nodes.borrow_mut().push(parent);
                return;
            }
            if !allow_dupe {
                VarCheck::report_var_multiply_declared(compiler, n, &name_str, orig_node);
            }
        } else if *name == ARGUMENTS
            && !(NodeUtil::is_name_declaration(compiler, n.get_parent(compiler))
                && n.is_name(compiler))
        {
            // Disallow shadowing "arguments" as we can't handle with our current
            // scope modeling.
            let error = JSError::make(compiler, n, &VAR_ARGUMENTS_SHADOWED_ERROR, &[]);
            compiler.report(error);
        }
    }
}
