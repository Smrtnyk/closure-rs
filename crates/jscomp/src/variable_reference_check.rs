/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/VariableReferenceCheck.java.

//! Port of `VariableReferenceCheck.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::abstract_scope::ImplicitVar;
use crate::basic_block::BasicBlock;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_group::DiagnosticGroup;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{AbstractShallowCallback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::reference_collector::{Behavior, ReferenceCollector};
use crate::reference_map::ReferenceMap;
use crate::scope::ScopeId;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use crate::var::VarId;
use crate::var_check::{VAR_MULTIPLY_DECLARED_ERROR, VarCheck};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::qualified_name::QualifiedName;
use closure_rhino::token::Token;
use std::rc::Rc;
use std::sync::LazyLock;

// port: VariableReferenceCheck#EARLY_REFERENCE
pub static EARLY_REFERENCE: DiagnosticType = DiagnosticType::warning(
    "JSC_REFERENCE_BEFORE_DECLARE",
    "Variable referenced before declaration: {0}",
);

// port: VariableReferenceCheck#EARLY_EXPORTS_REFERENCE
pub static EARLY_EXPORTS_REFERENCE: DiagnosticType = DiagnosticType::error(
    "JSC_EXPORTS_REFERENCE_BEFORE_ASSIGN",
    "Illegal reference to `exports` before assignment `exports = ...`",
);

// port: VariableReferenceCheck#REDECLARED_VARIABLE
pub static REDECLARED_VARIABLE: DiagnosticType =
    DiagnosticType::warning("JSC_REDECLARED_VARIABLE", "Redeclared variable: {0}");

// port: VariableReferenceCheck#EARLY_REFERENCE_ERROR
pub static EARLY_REFERENCE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_REFERENCE_BEFORE_DECLARE_ERROR",
    "Illegal variable reference before declaration: {0}",
);

// port: VariableReferenceCheck#REASSIGNED_CONSTANT
pub static REASSIGNED_CONSTANT: DiagnosticType =
    DiagnosticType::error("JSC_REASSIGNED_CONSTANT", "Constant reassigned: {0}");

// port: VariableReferenceCheck#REDECLARED_VARIABLE_ERROR
pub static REDECLARED_VARIABLE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_REDECLARED_VARIABLE_ERROR",
    "Illegal redeclared variable: {0}",
);

// port: VariableReferenceCheck#DECLARATION_NOT_DIRECTLY_IN_BLOCK
pub static DECLARATION_NOT_DIRECTLY_IN_BLOCK: DiagnosticType = DiagnosticType::error(
    "JSC_DECLARATION_NOT_DIRECTLY_IN_BLOCK",
    "Block-scoped declaration not directly within block: {0}",
);

// port: VariableReferenceCheck#UNUSED_LOCAL_ASSIGNMENT
pub static UNUSED_LOCAL_ASSIGNMENT: DiagnosticType = DiagnosticType::disabled(
    "JSC_UNUSED_LOCAL_ASSIGNMENT",
    "Value assigned to local variable {0} is never read",
);

// port: VariableReferenceCheck#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: VariableReferenceCheck#GOOG_REQUIRE_TYPE
static GOOG_REQUIRE_TYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: VariableReferenceCheck#GOOG_FORWARD_DECLARE
static GOOG_FORWARD_DECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));

// These types do not permit a block-scoped declaration inside them without an explicit block.
// e.g. if (b) let x;
// This list omits Token.LABEL intentionally. It's handled differently in IRFactory.
// port: VariableReferenceCheck#BLOCKLESS_DECLARATION_FORBIDDEN_STATEMENTS
const BLOCKLESS_DECLARATION_FORBIDDEN_STATEMENTS: [Token; 6] = [
    Token::IF,
    Token::FOR,
    Token::FOR_IN,
    Token::FOR_OF,
    Token::FOR_AWAIT_OF,
    Token::WHILE,
];

// port: VariableReferenceCheck#GOOG_SCOPE
static GOOG_SCOPE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.scope"));

/// Checks variables to see if they are referenced before their declaration, or if they are
/// redeclared in a way that is suspicious (i.e. not dictated by control structures). This is a
/// more aggressive version of `VarCheck`, but it lacks the cross-chunk checks.
pub struct VariableReferenceCheck {
    check_unused_locals: bool,

    // NOTE(nicksantos): It's a lot faster to use a shared Set that
    // we clear after each method call, because the Set never gets too big.
    // (A LinkedHashSet of BasicBlock, which has identity equality.)
    blocks_with_declarations: Vec<Rc<BasicBlock>>,
}

impl VariableReferenceCheck {
    // port: VariableReferenceCheck#VariableReferenceCheck
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let check_unused_locals = compiler
            .get_options()
            .enables(&DiagnosticGroup::for_type(&UNUSED_LOCAL_ASSIGNMENT));
        Self {
            check_unused_locals,
            blocks_with_declarations: Vec::new(),
        }
    }

    /// `blocksWithDeclarations.add(block)`: LinkedHashSet insertion by identity.
    fn add_block_with_declaration(&mut self, block: &Rc<BasicBlock>) {
        if !self
            .blocks_with_declarations
            .iter()
            .any(|b| Rc::ptr_eq(b, block))
        {
            self.blocks_with_declarations.push(block.clone());
        }
    }

    /// Returns the reference to the hoisted function, if the variable is one (its index in
    /// `references`: Java compares the Reference objects by identity).
    // port: VariableReferenceCheck#lookForHoistedFunction
    fn look_for_hoisted_function(&mut self, ast: &Ast, references: &[Reference]) -> Option<usize> {
        for (i, reference) in references.iter().enumerate() {
            if reference.is_hoisted_function(ast) {
                self.add_block_with_declaration(reference.get_basic_block().unwrap());
                return Some(i);
            }
        }
        None
    }

    // port: VariableReferenceCheck#checkBlocklessDeclaration
    fn check_blockless_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        reference: &Reference,
        reference_node: NodeId,
    ) {
        let grandparent = reference.get_grandparent(compiler).unwrap();
        if !reference.is_var_declaration(compiler)
            && grandparent.is_added_block(compiler)
            && BLOCKLESS_DECLARATION_FORBIDDEN_STATEMENTS.contains(
                &grandparent
                    .get_parent(compiler)
                    .unwrap()
                    .get_token(compiler),
            )
        {
            let name = v.get_name(compiler).to_string_lossy();
            let error = JSError::make(
                compiler,
                reference_node,
                &DECLARATION_NOT_DIRECTLY_IN_BLOCK,
                &[&name],
            );
            compiler.report(error);
        }
    }

    /// Returns if a redeclaration error has been found.
    // port: VariableReferenceCheck#checkRedeclaration
    fn check_redeclaration(
        &self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        reference: &Reference,
        reference_node: NodeId,
        hoisted_fn: Option<&Reference>,
        basic_block: &BasicBlock,
    ) -> bool {
        let v_parent_node = v.get_parent_node(compiler);
        let let_const_shadows_var = v_parent_node.unwrap().is_var(compiler)
            && (reference.is_let_declaration(compiler) || reference.is_const_declaration(compiler));
        let is_var_node_same_as_reference_node = v.get_node(compiler) == Some(reference.get_node());
        // We disallow redeclaration of caught exceptions
        let shadow_catch_var =
            v_parent_node.unwrap().is_catch(compiler) && !is_var_node_same_as_reference_node;

        if self.is_redeclaration(basic_block)
            && !VarCheck::has_duplicate_declaration_suppression(
                compiler,
                reference_node,
                v.get_name_node(compiler).unwrap(),
            )
        {
            let diagnostic_type: &'static DiagnosticType;
            let mut warning_node = reference_node;
            let shadow_param = v.is_param(compiler)
                // TODOO - lharker: this won't handle destructuring block scoped declarations. Is
                // this an actual bug?
                && NodeUtil::is_block_scoped_declaration(
                    compiler,
                    reference_node.get_parent(compiler).unwrap(),
                )
                && Some(v.get_scope(compiler))
                    == reference.get_scope().unwrap().get_parent(compiler);

            let is_function_decl = v_parent_node.is_some_and(|parent_node| {
                parent_node.is_function(compiler)
                    && parent_node.get_first_child(compiler) == Some(reference_node)
            });

            if v.is_let(compiler)
                || v.is_const(compiler)
                || v.is_class(compiler)
                || let_const_shadows_var
                || shadow_catch_var
                || shadow_param
                || v.is_import(compiler)
                || is_function_decl
            {
                // These cases are all hard errors that violate ES6 semantics
                diagnostic_type = &REDECLARED_VARIABLE_ERROR;
            } else if reference
                .get_node()
                .get_parent(compiler)
                .unwrap()
                .is_catch(compiler)
            {
                return false;
            } else {
                // These diagnostics are for valid, but suspicious, code, and are suppressible.
                // For vars defined in the global scope, give the same error as VarCheck
                diagnostic_type = if v.get_scope(compiler).is_global(compiler) {
                    &VAR_MULTIPLY_DECLARED_ERROR
                } else {
                    &REDECLARED_VARIABLE
                };
                // Since we skip hoisted functions, we would have the wrong warning node in cases
                // where the redeclaration is a function declaration. Check for that case.
                if is_var_node_same_as_reference_node
                    && let Some(hoisted_fn) = hoisted_fn
                    && v.get_name(compiler) == hoisted_fn.get_node().get_string(compiler)
                {
                    warning_node = hoisted_fn.get_node();
                }
            }
            let name = v.get_name(compiler).to_string_lossy();
            let location = Self::location_of(compiler, v.get_node(compiler));
            let error = JSError::make(compiler, warning_node, diagnostic_type, &[&name, &location]);
            compiler.report(error);
            return true;
        }

        if (let_const_shadows_var || shadow_catch_var)
            && Some(v.get_scope(compiler)) == reference.get_scope()
        {
            let name = v.get_name(compiler).to_string_lossy();
            let error = JSError::make(
                compiler,
                reference_node,
                &REDECLARED_VARIABLE_ERROR,
                &[&name],
            );
            compiler.report(error);
            return true;
        }

        false
    }

    /// Returns whether the given block executes after any known declarations of the variable
    /// being visited.
    // port: VariableReferenceCheck#isRedeclaration
    fn is_redeclaration(&self, new_declaration: &BasicBlock) -> bool {
        for previous_declaration in &self.blocks_with_declarations {
            if previous_declaration.provably_executes_before(new_declaration) {
                return true;
            }
        }
        false
    }

    // port: VariableReferenceCheck#locationOf
    fn location_of(ast: &Ast, n: Option<NodeId>) -> String {
        match n {
            None => "<unknown>".to_string(),
            Some(n) => n.get_location(ast),
        }
    }

    /// Returns if an early reference has been found.
    // port: VariableReferenceCheck#checkEarlyReference
    fn check_early_reference(
        compiler: &mut AbstractCompiler,
        v: VarId,
        reference: &Reference,
        reference_node: NodeId,
    ) -> bool {
        // Don't check the order of references in externs files.
        if reference_node.is_from_externs(compiler) || v.is_implicit_goog_namespace(compiler) {
            return false;
        }
        // Special case to deal with var goog = goog || {}. Note that
        // let x = x || {} is illegal, just like var y = x || {}; let x = y;
        if v.is_var(compiler) {
            let mut curr = reference.get_parent(compiler);
            while curr.is_or(compiler)
                && curr.get_parent(compiler).unwrap().get_first_child(compiler) == Some(curr)
            {
                curr = curr.get_parent(compiler).unwrap();
            }
            if curr.is_name(compiler) && curr.get_string(compiler) == v.get_name(compiler) {
                return false;
            }
        }

        // RHS of public fields are not early references
        let reference_scope_root = reference.get_scope().unwrap().get_root_node(compiler);
        if reference_scope_root.is_member_field_def(compiler)
            && !reference_scope_root.is_static_member(compiler)
        {
            return false;
        }

        // Only generate warnings for early references in the same function scope/global scope in
        // order to deal with possible forward declarations and recursion
        // e.g. don't warn on:
        //   function f() { return x; } f(); let x = 5;
        // We don't track where `f` is called, just where it's defined, and don't want to warn for
        //     function f() { return x; } let x = 5; f();
        // TODO(moz): See if we can remove the bypass for "goog"
        if reference
            .get_scope()
            .unwrap()
            .has_same_container_scope(compiler, v.get_scope(compiler))
            && v.get_name(compiler) != "goog"
        {
            let diagnostic_type: &'static DiagnosticType = if v.is_goog_module_exports(compiler) {
                &EARLY_EXPORTS_REFERENCE
            } else if v.is_let(compiler)
                || v.is_const(compiler)
                || v.is_class(compiler)
                || v.is_param(compiler)
            {
                &EARLY_REFERENCE_ERROR
            } else {
                &EARLY_REFERENCE
            };
            let name = v.get_name(compiler).to_string_lossy();
            let error = JSError::make(compiler, reference.get_node(), diagnostic_type, &[&name]);
            compiler.report(error);
            return true;
        }

        false
    }

    // Only check for unused local if not in a goog.scope function.
    // TODO(tbreisacher): Consider moving UNUSED_LOCAL_ASSIGNMENT into its own check pass, so
    // that we can run it after goog.scope processing, and get rid of the inGoogScope check.
    // port: VariableReferenceCheck#checkForUnusedLocalVar
    fn check_for_unused_local_var(
        compiler: &mut AbstractCompiler,
        v: VarId,
        unused_assignment: &Reference,
    ) {
        if !v.is_local(compiler) {
            return;
        }
        let js_doc = NodeUtil::get_best_jsdoc_info(compiler, unused_assignment.get_node());
        if js_doc.is_some_and(|js_doc| js_doc.has_typedef_type()) {
            return;
        }

        let mut in_goog_scope = false;
        let s = v.get_scope(compiler);
        if s.is_function_block_scope(compiler) {
            let function = s.get_root_node(compiler).get_parent(compiler).unwrap();
            let callee = function.get_previous(compiler);
            in_goog_scope = callee.is_some_and(|callee| GOOG_SCOPE.matches(compiler, callee));
        }

        if in_goog_scope {
            // No warning.
            return;
        }

        if s.is_module_scope(compiler) {
            let statement =
                NodeUtil::get_enclosing_statement(compiler, v.get_node(compiler).unwrap());
            if NodeUtil::is_name_declaration(compiler, statement) {
                let statement = statement.unwrap();
                let lhs = statement.get_first_child(compiler).unwrap();
                let rhs = lhs.get_first_child(compiler);
                if let Some(rhs) = rhs
                    && (NodeUtil::is_call_to_qualified_name(compiler, rhs, &GOOG_FORWARD_DECLARE)
                        || NodeUtil::is_call_to_qualified_name(compiler, rhs, &GOOG_REQUIRE_TYPE)
                        || NodeUtil::is_call_to_qualified_name(compiler, rhs, &GOOG_REQUIRE)
                        || rhs.is_qualified_name(compiler))
                {
                    // No warning. module imports will be caught by the unused-require check, and
                    // if the right side is a qualified name then this is likely an alias used in
                    // type annotations.
                    return;
                }
            }
        }

        let name = v.get_name(compiler).to_string_lossy();
        let error = JSError::make(
            compiler,
            unused_assignment.get_node(),
            &UNUSED_LOCAL_ASSIGNMENT,
            &[&name],
        );
        compiler.report(error);
    }
}

impl CompilerPass for VariableReferenceCheck {
    // port: VariableReferenceCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut scope_creator = SyntacticScopeCreator::new();
        let behavior = ReferenceCheckingBehavior::new(self);
        let mut collector = ReferenceCollector::new(compiler, behavior, &mut scope_creator);
        CompilerPass::process(&mut collector, compiler, externs, root);
    }
}

/// Behavior that checks variables for redeclaration or early references just after they go out
/// of scope.
struct ReferenceCheckingBehavior<'a> {
    outer: &'a mut VariableReferenceCheck,
    vars_in_function_body: IndexSet<JsString>,
}

impl<'a> ReferenceCheckingBehavior<'a> {
    // port: VariableReferenceCheck.ReferenceCheckingBehavior#ReferenceCheckingBehavior
    fn new(outer: &'a mut VariableReferenceCheck) -> Self {
        Self {
            outer,
            vars_in_function_body: IndexSet::<_>::default(),
        }
    }

    // port: VariableReferenceCheck.ReferenceCheckingBehavior#checkDefaultParam
    fn check_default_param(
        compiler: &mut AbstractCompiler,
        param: VarId,
        scope: ScopeId,
        vars_in_function_body: &IndexSet<JsString>,
    ) {
        let default_value = param
            .get_parent_node(compiler)
            .unwrap()
            .get_second_child(compiler)
            .unwrap();
        NodeTraversal::traverse(
            compiler,
            default_value,
            /*
             * Do a shallow check since cases like: {@code
             *   function f(y = () => x, x = 5) { return y(); }
             * } is legal. We are going to miss cases like: {@code
             *   function f(y = (() => x)(), x = 5) { return y(); }
             * } but this should be rare.
             */
            &mut AbstractShallowCallback::new(
                // port: VariableReferenceCheck.ReferenceCheckingBehavior#checkDefaultParam (anonymous AbstractShallowCallback#visit)
                |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| {
                    if !NodeUtil::is_reference_name(t, n) {
                        return;
                    }
                    let ref_name = n.get_string(t);
                    if vars_in_function_body.contains(&ref_name)
                        && !scope.has_slot(t.get_compiler(), ref_name.clone())
                    {
                        let compiler = t.get_compiler();
                        let error = JSError::make(
                            compiler,
                            n,
                            &EARLY_REFERENCE_ERROR,
                            &[&ref_name.to_string()],
                        );
                        compiler.report(error);
                    }
                },
            ),
        );
    }

    // port: VariableReferenceCheck.ReferenceCheckingBehavior#checkShadowParam
    fn check_shadow_param(
        compiler: &mut AbstractCompiler,
        v: VarId,
        function_scope: ScopeId,
        references: &[Reference],
    ) {
        let name = v.get_name(compiler);
        let maybe_param = function_scope.get_var(compiler, name.clone());
        if let Some(maybe_param) = maybe_param
            && maybe_param.is_param(compiler)
            && maybe_param.get_scope(compiler) == function_scope
        {
            for r in references {
                if (r.is_var_declaration(compiler) || r.is_hoisted_function(compiler))
                    && Some(r.get_node()) != v.get_name_node(compiler)
                {
                    let error = JSError::make(
                        compiler,
                        r.get_node(),
                        &REDECLARED_VARIABLE,
                        &[&name.to_string()],
                    );
                    compiler.report(error);
                }
            }
        }
    }

    // port: VariableReferenceCheck.ReferenceCheckingBehavior#checkGoogModuleExports
    fn check_goog_module_exports(
        compiler: &mut AbstractCompiler,
        scope: ScopeId,
        exports_var: VarId,
        reference_map: &dyn ReferenceMap,
    ) {
        // Java's Map<Var, ReferenceCollection>.get uses Var's inherited ScopedName equality (same
        // name and scope root), so the fresh implicit var finds the entry of the Var the
        // ReferenceCollector got from this scope (Rust-only: VarId equality is arena identity).
        let key = scope
            .get_own_slot(compiler, exports_var.get_name(compiler))
            .unwrap_or(exports_var);
        let references = reference_map.get_references(key);
        let Some(references) = references else {
            return;
        };
        if references.is_never_assigned(compiler) {
            return;
        }

        for reference in &references.references {
            if reference.is_lvalue(compiler) {
                break;
            }
            VariableReferenceCheck::check_early_reference(
                compiler,
                exports_var,
                reference,
                reference.get_node(),
            );
        }
    }

    /// If the variable is declared more than once in a basic block, generate a warning. Also
    /// check if a variable is used in a given scope before it is declared, which suggest a likely
    /// error. Relies on the fact that references is in parse-tree order.
    // port: VariableReferenceCheck.ReferenceCheckingBehavior#checkVar
    fn check_var(&mut self, compiler: &mut AbstractCompiler, v: VarId, references: &[Reference]) {
        self.outer.blocks_with_declarations.clear();
        let mut has_seen_declaration = false;
        let mut has_errors = false;
        let mut is_read = false;
        let mut unused_assignment: Option<&Reference> = None;

        let hoisted_fn = self.outer.look_for_hoisted_function(compiler, references);
        if hoisted_fn.is_some() {
            has_seen_declaration = true;
        }

        for (index, reference) in references.iter().enumerate() {
            if Some(index) == hoisted_fn {
                continue;
            }

            let reference_node = reference.get_node();
            let basic_block = reference.get_basic_block().unwrap();
            let is_declaration = reference.is_declaration(compiler);
            let is_assignment = is_declaration || reference.is_lvalue(compiler);

            if is_declaration {
                // Checks for declarations
                has_seen_declaration = true;
                has_errors = self.outer.check_redeclaration(
                    compiler,
                    v,
                    reference,
                    reference_node,
                    hoisted_fn.map(|i| &references[i]),
                    basic_block,
                );
                // Add the current basic block after checking redeclarations
                self.outer.add_block_with_declaration(basic_block);
                self.outer
                    .check_blockless_declaration(compiler, v, reference, reference_node);

                if reference
                    .get_grandparent(compiler)
                    .unwrap()
                    .is_export(compiler)
                {
                    is_read = true;
                }
            } else {
                // Checks for references
                if !has_seen_declaration {
                    has_errors = VariableReferenceCheck::check_early_reference(
                        compiler,
                        v,
                        reference,
                        reference_node,
                    );
                }

                if !has_errors && v.is_const(compiler) && reference.is_lvalue(compiler) {
                    let name = v.get_name(compiler).to_string_lossy();
                    let error =
                        JSError::make(compiler, reference_node, &REASSIGNED_CONSTANT, &[&name]);
                    compiler.report(error);
                }

                // Check for temporal dead zone of let / const declarations in for-in and for-of
                // loops
                // TODO(b/111441110): Fix this check. it causes spurious warnings on `b = a` in
                //   for (const [a, b = a] of []) {}
                if (v.is_let(compiler) || v.is_const(compiler))
                    && Some(v.get_scope(compiler)) == reference.get_scope()
                    && NodeUtil::is_enhanced_for(
                        compiler,
                        reference.get_scope().unwrap().get_root_node(compiler),
                    )
                {
                    let name = v.get_name(compiler).to_string_lossy();
                    let error =
                        JSError::make(compiler, reference_node, &EARLY_REFERENCE_ERROR, &[&name]);
                    compiler.report(error);
                }
            }

            if is_assignment {
                let decl = &references[0];
                let decl_node = decl.get_node();
                let gp = decl_node.get_grandparent(compiler).unwrap();
                let lhs_of_for_in_loop =
                    gp.is_for_in(compiler) && gp.get_first_first_child(compiler) == Some(decl_node);

                if decl.get_scope().unwrap().is_local(compiler)
                    && (decl.is_var_declaration(compiler)
                        || decl.is_let_declaration(compiler)
                        || decl.is_const_declaration(compiler))
                    && !decl.get_node().is_from_externs(compiler)
                    && !lhs_of_for_in_loop
                {
                    unused_assignment = Some(reference);
                }
                let reference_parent = reference.get_parent(compiler);
                if (reference_parent.is_dec(compiler)
                    || reference_parent.is_inc(compiler)
                    || NodeUtil::is_compound_assignment_op(compiler, reference_parent))
                    && NodeUtil::is_expression_result_used(compiler, reference.get_node())
                {
                    is_read = true;
                }
            } else {
                is_read = true;
            }
        }

        if self.outer.check_unused_locals
            && let Some(unused_assignment) = unused_assignment
            && !is_read
            && !has_errors
        {
            VariableReferenceCheck::check_for_unused_local_var(compiler, v, unused_assignment);
        }
    }
}

impl Behavior for ReferenceCheckingBehavior<'_> {
    // port: VariableReferenceCheck.ReferenceCheckingBehavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        // Check all vars after finishing a scope
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        if scope.is_function_block_scope(compiler) {
            self.vars_in_function_body.clear();
            for v in scope.get_var_iterable(compiler) {
                self.vars_in_function_body.insert(v.get_name(compiler));
            }
        }
        for v in scope.get_var_iterable(compiler) {
            let reference_collection = reference_map.get_references(v);
            // TODO(moz): Figure out why this could be null
            if let Some(reference_collection) = reference_collection {
                if scope.get_root_node(compiler).is_function(compiler)
                    && v.is_default_param(compiler)
                {
                    Self::check_default_param(compiler, v, scope, &self.vars_in_function_body);
                }
                if scope.get_root_node(compiler).is_function(compiler) {
                    Self::check_shadow_param(compiler, v, scope, &reference_collection.references);
                }
                self.check_var(compiler, v, &reference_collection.references);
            }
        }
        if scope.has_own_implicit_slot(compiler, Some(ImplicitVar::EXPORTS)) {
            let exports_var = scope.make_implicit_var(compiler, ImplicitVar::EXPORTS);
            Self::check_goog_module_exports(compiler, scope, exports_var, reference_map);
        }
    }
}
