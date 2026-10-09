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
//   src/com/google/javascript/jscomp/Normalize.java.

//! Port of `Normalize.java`.
//!
//! The goal with this pass is to simplify the other passes, by making less complex statements.
//!
//! Starting with statements like: `var a = 0, b = foo();`
//!
//! Which become: `var a = 0; var b = foo();`
//!
//! The key here is only to break down things that help the other passes and can be put back
//! together in a form that is at least as small when all is said and done.
//!
//! This pass currently does the following:
//!
//! 1. Simplifies the AST by splitting var/let/const statements, moving initializers out of for
//!    loops, and converting whiles to fors.
//! 2. Makes all variable names globally unique (extern or otherwise) so that no value is ever
//!    shadowed (note: "arguments" may require special handling).
//! 3. Removes duplicate variable declarations.
//! 4. Marks constants with the IS_CONSTANT_NAME annotation.
//! 5. Rewrite body of arrow function as a block.
//! 6. Take var statements out from for-loop initializer. This: for(var a = 0;a<0;a++) {}
//!    becomes: var a = 0; for(a;a<0;a++) {}
// Preserve the Java switch cases with nested ifs.
#![allow(clippy::collapsible_match)]
use crate::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    ast_factory::AstFactory,
    compiler_input::CompilerInput,
    compiler_pass::CompilerPass,
    make_declared_names_unique::MakeDeclaredNamesUnique,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    rewrite_logical_assignment_operators_helper::RewriteLogicalAssignmentOperatorsHelper,
    scope::ScopeId,
    syntactic_scope_creator::{RedeclarationHandler, SyntacticScopeCreator},
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId, Prop},
    token::Token,
};
use std::rc::Rc;

pub struct Normalize {
    ast_factory: AstFactory,
    assert_on_change: bool,
}

impl Normalize {
    // port: Normalize#Normalize
    fn new(builder: Builder<'_>) -> Self {
        let assert_on_change = builder.assert_on_change;
        let ast_factory = builder.compiler.create_ast_factory();
        Self {
            ast_factory,
            assert_on_change,
        }
    }

    // port: Normalize#createNormalizeForOptimizations
    pub fn create_normalize_for_optimizations(compiler: &mut AbstractCompiler) -> Normalize {
        // The default option values are the right ones for optimizations
        Self::builder(compiler).build()
    }

    // port: Normalize#builder
    pub fn builder(compiler: &mut AbstractCompiler) -> Builder<'_> {
        Builder::new(compiler)
    }

    // port: Normalize#reportCodeChange
    fn report_code_change(
        &self,
        compiler: &mut AbstractCompiler,
        change_description: &str,
        n: NodeId,
    ) {
        if self.assert_on_change {
            panic!("Normalize constraints violated:\n{change_description}");
        }
        compiler.report_change_to_enclosing_scope(n);
    }

    /// Is this a name node of a function expression?
    // port: Normalize#isFunctionExpressionNameNode
    fn is_function_expression_name_node(ast: &Ast, n: Option<NodeId>) -> bool {
        let Some(n) = n else {
            return false;
        };
        if !n.is_name(ast) {
            return false;
        }
        let Some(parent) = n.get_parent(ast) else {
            return false;
        };
        NodeUtil::is_function_expression(ast, parent) && n.is_first_child_of(ast, Some(parent))
    }
}

impl CompilerPass for Normalize {
    // port: Normalize#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let renamer = MakeDeclaredNamesUnique::builder()
            .with_assert_on_change(self.assert_on_change)
            .build();
        let mut normalize_statements =
            NormalizeStatements::new(compiler, self.assert_on_change, Some(renamer));
        NodeTraversal::traverse_roots(compiler, &mut normalize_statements, externs, root);

        let mut propagate_constant_property_over_vars =
            PropagateConstantPropertyOverVars::new(self.assert_on_change);
        let mut scope_creator = SyntacticScopeCreator::new_with_redeclaration_handler(Box::new(
            DuplicateDeclarationHandler::new(self),
        ));
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut propagate_constant_property_over_vars)
            .set_scope_creator(&mut scope_creator)
            .traverse_roots(externs, root);

        if !compiler.get_life_cycle_stage().is_normalized() {
            compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        }
    }
}

/// Configures and builds a `Normalize` object.
pub struct Builder<'a> {
    compiler: &'a mut AbstractCompiler,
    assert_on_change: bool,
}

impl<'a> Builder<'a> {
    // port: Normalize.Builder#Builder
    fn new(compiler: &'a mut AbstractCompiler) -> Self {
        Self {
            compiler,
            assert_on_change: false,
        }
    }

    /// If the Normalize pass finds work to do, it will throw an exception.
    ///
    /// This is intended for use in validating that an AST is already normalized.
    ///
    /// This option is `false` by default.
    // port: Normalize.Builder#assertOnChange
    pub fn assert_on_change(mut self, assert_on_change: bool) -> Self {
        self.assert_on_change = assert_on_change;
        self
    }

    // port: Normalize.Builder#build
    pub fn build(self) -> Normalize {
        Normalize::new(self)
    }
}

/// Propagate constant annotations and IS_CONSTANT_NAME property over the Var graph.
///
/// Also invokes t.getScope() on every scope, for use with the `DuplicateDeclarationHandler`.
struct PropagateConstantPropertyOverVars {
    assert_on_change: bool,
}

impl PropagateConstantPropertyOverVars {
    // port: Normalize.PropagateConstantPropertyOverVars#PropagateConstantPropertyOverVars
    fn new(forbid_changes: bool) -> Self {
        Self {
            assert_on_change: forbid_changes,
        }
    }
}

impl ScopedCallback for PropagateConstantPropertyOverVars {
    // port: Normalize.PropagateConstantPropertyOverVars#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        // Cause the scope to be created, which will cause duplicate
        // to be found.
        t.get_scope();
    }

    // port: Normalize.PropagateConstantPropertyOverVars#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {
        // Nothing to do.
    }
}

impl Callback for PropagateConstantPropertyOverVars {
    // port: Normalize.PropagateConstantPropertyOverVars#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Normalize.PropagateConstantPropertyOverVars#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // Note: Constant properties annotations are not propagated.
        if !n.is_name(t) || n.get_string_ref(t).is_empty() {
            return;
        }

        // Find the JSDocInfo for a top-level variable
        let scope = t.get_scope();
        let name = n.get_string(t);
        let var = scope.get_var(t.get_compiler(), name);
        let compiler: &AbstractCompiler = t.get_compiler();
        let info = var.and_then(|var| var.get_jsdoc_info(compiler));

        let should_be_constant = var.is_some_and(|var| {
            var.is_const(compiler)
                || Normalize::is_function_expression_name_node(compiler, var.get_node(compiler))
        }) || info.as_ref().is_some_and(|info| info.is_constant())
            || NodeUtil::is_constant_by_convention(compiler, compiler.get_coding_convention(), n);
        let is_marked_constant = n.get_boolean_prop(compiler, Prop::IS_CONSTANT_NAME);
        if should_be_constant && !is_marked_constant {
            if self.assert_on_change {
                let name = n.get_string(t);
                panic!(
                    "Should be const but not marked as const.\n  name: {}\n  parent:{}",
                    name,
                    n.get_parent(t).unwrap().to_string_tree(t)
                );
            }
            n.put_boolean_prop(t, Prop::IS_CONSTANT_NAME, true);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

/// Simplify the AST:
/// - VAR declarations split, so they represent exactly one child declaration.
/// - WHILEs are converted to FORs.
/// - FOR loop are initializers are moved out of the FOR structure.
/// - LABEL node of children other than LABEL, BLOCK, WHILE, FOR, or DO are moved into a block.
/// - Add constant annotations based on coding convention.
pub struct NormalizeStatements {
    ast_factory: Rc<AstFactory>,
    assert_on_change: bool,
    make_declared_names_unique: Option<MakeDeclaredNamesUnique>,
    rewrite_logical_assignment_operators_helper: RewriteLogicalAssignmentOperatorsHelper,
}

impl NormalizeStatements {
    /// Prefer using `Normalize` as a compiler pass instead of using this class directly.
    // port: Normalize.NormalizeStatements#NormalizeStatements
    pub fn new(
        compiler: &mut AbstractCompiler,
        assert_on_change: bool,
        make_declared_names_unique: Option<MakeDeclaredNamesUnique>,
    ) -> Self {
        let ast_factory = Rc::new(compiler.create_ast_factory());
        // Java passes compiler.getUniqueIdSupplier(); the helper reaches it through the compiler.
        let rewrite_logical_assignment_operators_helper =
            RewriteLogicalAssignmentOperatorsHelper::new(ast_factory.clone());
        Self {
            ast_factory,
            assert_on_change,
            make_declared_names_unique,
            rewrite_logical_assignment_operators_helper,
        }
    }

    // port: Normalize.NormalizeStatements#reportCodeChange
    fn report_code_change(
        &self,
        compiler: &mut AbstractCompiler,
        change_description: &str,
        n: NodeId,
    ) {
        if self.assert_on_change {
            panic!("Normalize constraints violated:\n{change_description}");
        }
        compiler.report_change_to_enclosing_scope(n);
    }

    /// Mark names and properties that are constants by convention.
    // port: Normalize.NormalizeStatements#annotateConstantsByConvention
    fn annotate_constants_by_convention(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        check_state!(n.is_name(compiler));

        // Need to check that variables have not been renamed, to determine whether
        // coding conventions still apply.
        if compiler.get_life_cycle_stage().is_normalized_obfuscated() {
            return;
        }

        let shared: &AbstractCompiler = compiler;
        if !n.get_boolean_prop(shared, Prop::IS_CONSTANT_NAME)
            && NodeUtil::is_constant_by_convention(shared, shared.get_coding_convention(), n)
        {
            check_state!(
                !self.assert_on_change,
                "Not marked as constant when it should be: %s",
                n.to_string(compiler)
            );
            n.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        }
    }

    /// Annotate function names on function expressions to be unconditionally constant
    // port: Normalize.NormalizeStatements#annotateFunctionExpressionNameAsConstant
    fn annotate_function_expression_name_as_constant(&self, ast: &mut Ast, n: NodeId) {
        check_state!(
            n.is_name(ast),
            "Expected NAME node but got %s",
            n.get_token(ast)
        );
        if Normalize::is_function_expression_name_node(ast, Some(n)) {
            n.put_boolean_prop(ast, Prop::IS_CONSTANT_NAME, true);
        }
    }

    /// Splits ES6 export combined with a variable or function declaration.
    // port: Normalize.NormalizeStatements#splitExportDeclaration
    fn split_export_declaration(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.get_boolean_prop(compiler, Prop::EXPORT_DEFAULT) {
            return;
        }
        let mut destructuring_lhs_nodes = Vec::new();
        let c = n.get_first_child(compiler).unwrap();
        if NodeUtil::is_declaration(compiler, c) {
            c.detach(compiler);

            let export_specs = compiler.new_node(Token::EXPORT_SPECS).srcref(compiler, n);
            n.add_child_to_front(compiler, export_specs);
            if c.is_class(compiler) || c.is_function(compiler) {
                let name = c.get_first_child(compiler).unwrap();
                c.insert_before(compiler, n);
                Self::add_name_node_to_export_specs(compiler, export_specs, name);
            } else {
                NodeUtil::visit_lhs_nodes_in_node(
                    &mut **compiler,
                    c,
                    &mut |ast: &mut Ast, name: NodeId| {
                        Self::add_name_node_to_export_specs(ast, export_specs, name)
                    },
                );
                // Split up var declarations onto separate lines.
                let mut child = c.get_first_child(compiler);
                while let Some(cur) = child {
                    let next = cur.get_next(compiler);
                    cur.detach(compiler);
                    let token = c.get_token(compiler);
                    let new_declaration =
                        compiler.new_node_with_child(token, cur).srcref(compiler, n);
                    new_declaration.insert_before(compiler, n);
                    if cur.is_destructuring_lhs(compiler) {
                        // `export {a, b} = ...` changed to `var {a,b} = ...; export {a, b};`
                        // Hence we must normalize the destructuring declaration.
                        destructuring_lhs_nodes.push(cur);
                    }
                    child = next;
                }
            }

            // normalize the newly added destructuring var declarations
            for destructuring_lhs_node in destructuring_lhs_nodes {
                self.normalize_destructuring_lhs(compiler, destructuring_lhs_node);
            }

            let parent = n.get_parent(compiler).unwrap();
            self.report_code_change(compiler, "combined export and declaration", parent);
        }
    }

    // port: Normalize.NormalizeStatements#addNameNodeToExportSpecs
    fn add_name_node_to_export_specs(ast: &mut Ast, export_specs: NodeId, name: NodeId) {
        let export_spec = ast.new_node(Token::EXPORT_SPEC).srcref(ast, name);
        let clone = name.clone_node(ast);
        export_spec.add_child_to_front(ast, clone);
        let clone = name.clone_node(ast);
        export_spec.add_child_to_front(ast, clone);
        export_specs.add_child_to_back(ast, export_spec);
    }

    /// Rewrite blockless arrow functions to have a block with a single return statement
    ///
    /// For example: `(x) => x` becomes `(x) => { return x; }`.
    ///
    /// This simplifies optimizations as they can now assume all functions have a BLOCK.
    // port: Normalize.NormalizeStatements#normalizeBlocklessFunction
    fn normalize_blockless_function(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        check_state!(n.is_function(compiler), "%s", n.to_string(compiler));
        if !NodeUtil::get_function_body(compiler, n).is_block(compiler) {
            let return_value = NodeUtil::get_function_body(compiler, n);
            let detached = return_value.detach(compiler);
            let return_node = IR::return_node_with_expression(compiler, detached);
            let body = IR::block_with_child(compiler, return_node);
            body.srcref_tree_if_missing(compiler, return_value);
            n.add_child_to_back(compiler, body);
            self.report_code_change(compiler, "blockless arrow function", body);
        }
    }

    /// Do normalizations that introduce new siblings or parents.
    // port: Normalize.NormalizeStatements#doStatementNormalizations
    fn do_statement_normalizations(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_label(compiler) {
            self.normalize_labels(compiler, n);
        }

        // Only inspect the children of SCRIPTs, BLOCKs and LABELs, as all these
        // are the only legal place for VARs and FOR statements.
        if NodeUtil::is_statement_block(compiler, n) || n.is_label(compiler) {
            // makes sure that var [a,b] destructuring from for-loop initializer is also extracted
            self.extract_for_initializer(compiler, n, None, None);
        }

        // Only inspect the children of SCRIPTs, BLOCKs, as all these
        // are the only legal place for VARs.
        if NodeUtil::is_statement_block(compiler, n) {
            self.split_var_declarations(compiler, n);
        }

        if n.is_function(compiler) {
            self.normalize_blockless_function(compiler, n);
            let body = n.get_last_child(compiler).unwrap();
            self.move_named_functions(compiler, body);
        }

        if NodeUtil::is_class_static_block(compiler, n) {
            self.move_named_functions(compiler, n);
        }

        if n.is_export(compiler) {
            // Perform the split in pre-order traversal to accurately normalize destructuring
            // declarations.
            self.split_export_declaration(compiler, n);
        }

        if NodeUtil::is_compound_assignment_op(compiler, n)
            && !NodeUtil::is_logical_assignment_op(compiler, n)
        {
            // Logical assignments should be handled in visit(), not here
            self.normalize_assign_shorthand(compiler, n);
        }
    }

    /// Split a var destructuring declaration into stub declarations of individual LHS name nodes
    /// following by the destructuring pattern assignment.
    // port: Normalize.NormalizeStatements#normalizeDestructuringLhs
    fn normalize_destructuring_lhs(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        if self.is_var_destructuring_declaration(compiler, n) {
            self.rewrite_var_destructuring_declaration(compiler, n);
        }
    }

    /// Is this a var destructuring LHS like `[a,b]` in `var [a,b] = ...`
    // port: Normalize.NormalizeStatements#isVarDestructuringDeclaration
    fn is_var_destructuring_declaration(&self, ast: &Ast, n: NodeId) -> bool {
        n.is_destructuring_lhs(ast) // `[a,b]`
            && (n.get_first_child(ast).unwrap().is_array_pattern(ast)
                || n.get_first_child(ast).unwrap().is_object_pattern(ast)) // `var [a,b] = [5,6];` or `var {a,b} = {a: 5, b: 6};`
            && n.get_parent(ast).unwrap().is_var(ast) // `var [a,b] = [5,6];`
    }

    /// Transforms a var destructuring array declarations into stub declarations of the
    /// individual lhs names within the array. For example:
    ///
    /// ```text
    ///    var [a, b = 3] = ....
    /// to:
    ///    var a; var b; [a, b = 3] = ...
    /// ```
    ///
    /// Same for object destructuring declarations. For example:
    ///
    /// ```text
    ///    var {a, b = 3} = ....
    /// to:
    ///    var a; var b; ({a, b = 3} = ...);
    /// ```
    ///
    /// Before this function is used, `extractForInitializer` has already run during pre-order
    /// traversal, so we need not worry about destructuring declarations in the for loop
    /// initializers (e.g. `for (var [a,b] ...)` is already rewritten before this). Also, export
    /// declarations are already handled in `splitExportDeclaration` during pre-order traversal.
    // port: Normalize.NormalizeStatements#rewriteVarDestructuringDeclaration
    fn rewrite_var_destructuring_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        destructuring_lhs: NodeId,
    ) {
        check_state!(
            !destructuring_lhs
                .get_grandparent(compiler)
                .unwrap()
                .is_export(compiler),
            "Export destructuring declarations should be already handled in `splitExportDeclaration` during pre-order traversal."
        );
        let var = destructuring_lhs.get_parent(compiler).unwrap();

        // create a stub declaration for each name in the destructuring pattern
        NodeUtil::visit_lhs_nodes_in_node(
            &mut **compiler,
            destructuring_lhs,
            &mut |ast: &mut Ast, name: NodeId| {
                // Add a declaration outside the destructuring pattern for the given name.
                check_state!(
                    name.is_name(ast),
                    "lhs in destructuring declaration should be a simple name. (%s)",
                    name.to_string(ast)
                );
                let name_string = name.get_string(ast);
                let new_name = IR::name(ast, name_string).srcref(ast, name);
                if name.get_boolean_prop(ast, Prop::IS_CONSTANT_NAME) {
                    // if old name was a const, new name should be too
                    // e.g. when rewriting `{VALUE} = ...` the `VALUE` is const by coding
                    // convention
                    new_name.put_boolean_prop(ast, Prop::IS_CONSTANT_NAME, true);
                }
                let new_var = IR::var(ast, new_name).srcref(ast, name);
                new_var.insert_before(ast, var);
            },
        );

        // Transform destructuring var declaration to assignment. That is, `var [a, b] = ...` to
        // `[a, b] = ...` and `var {a, b} = ...` to `({a, b} = ...);`
        let destructuring_pattern = destructuring_lhs.remove_first_child(compiler).unwrap();
        check_state!(
            destructuring_pattern.is_destructuring_pattern(compiler),
            "Expected destructuring pattern."
        );

        let rhs = destructuring_lhs.remove_first_child(compiler).unwrap();
        let assign = self
            .ast_factory
            .create_assign(compiler, destructuring_pattern, rhs);
        assign.srcref(compiler, var);
        let expr = self.ast_factory.expr_result(compiler, assign);
        expr.srcref(compiler, var);

        var.replace_with(compiler, expr);
        destructuring_lhs.detach(compiler);
        self.report_code_change(compiler, "Var destructuring declaration rewritten", expr);
    }

    // TODO(johnlenz): Move this to NodeTypeNormalizer once the unit tests are
    // fixed.
    /// Limit the number of special cases where LABELs need to be handled. Only BLOCK and loops
    /// are allowed to be labeled. Loop labels must remain in place as the named continues are
    /// not allowed for labeled blocks.
    // port: Normalize.NormalizeStatements#normalizeLabels
    fn normalize_labels(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        check_argument!(n.is_label(compiler));

        let last = n.get_last_child(compiler).unwrap();
        // TODO(moz): Avoid adding blocks for cases like "label: let x;"
        match last.get_token(compiler) {
            Token::LABEL
            | Token::BLOCK
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::WHILE
            | Token::DO => {}
            _ => {
                let block = IR::block(compiler);
                block.srcref_if_missing(compiler, last);
                last.replace_with(compiler, block);
                block.add_child_to_front(compiler, last);
                self.report_code_change(compiler, "LABEL normalization", n);
            }
        }
    }

    /// Bring the initializers out of FOR loops. These need to be placed before any associated
    /// LABEL nodes. This needs to be done from the top level label first so this is called as a
    /// pre-order callback (from shouldTraverse).
    ///
    /// `n`: The node to inspect.
    /// `before`: The node to insert the initializer before.
    /// `before_parent`: The parent of the node before which the initializer will be inserted.
    // port: Normalize.NormalizeStatements#extractForInitializer
    fn extract_for_initializer(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        before: Option<NodeId>,
        before_parent: Option<NodeId>,
    ) {
        let mut next;
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            next = cur.get_next(compiler);
            let insert_before = before.unwrap_or(cur);
            let insert_before_parent = if before.is_none() {
                Some(n)
            } else {
                before_parent
            };
            match cur.get_token(compiler) {
                Token::LABEL => self.extract_for_initializer(
                    compiler,
                    cur,
                    Some(insert_before),
                    insert_before_parent,
                ),
                Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                    let first = cur.get_first_child(compiler).unwrap();
                    if first.is_var(compiler) {
                        let lhs = first.get_first_child(compiler).unwrap();
                        if lhs.is_destructuring_lhs(compiler) {
                            // Transform:
                            //    for (var [a, b = 3] in c) {}
                            // to:
                            //    var a; var b; for ([a, b = 3] in c) {}
                            NodeUtil::visit_lhs_nodes_in_node(
                                &mut **compiler,
                                lhs,
                                &mut |ast: &mut Ast, name: NodeId| {
                                    // Add a declaration outside the for loop for the given name.
                                    check_state!(
                                        name.is_name(ast),
                                        "lhs in destructuring declaration should be a simple name. (%s)",
                                        name.to_string(ast)
                                    );
                                    let name_string = name.get_string(ast);
                                    let new_name = IR::name(ast, name_string).srcref(ast, name);
                                    let new_var = IR::var(ast, new_name).srcref(ast, name);
                                    new_var.insert_before(ast, insert_before);
                                },
                            );

                            // Transform for (var [a, b]... ) to for ([a, b]...
                            let destructuring_pattern = lhs.remove_first_child(compiler).unwrap();
                            first.replace_with(compiler, destructuring_pattern);
                        } else {
                            // Transform:
                            //    for (var a in b) {}
                            // to:
                            //    var a; for (a in b) {};
                            let new_statement = first;
                            let name = new_statement
                                .get_first_child(compiler)
                                .unwrap()
                                .clone_node(compiler);
                            first.replace_with(compiler, name);
                            new_statement.insert_before(compiler, insert_before);
                        }
                        self.report_code_change(compiler, "FOR-IN var declaration", n);
                    }
                }
                Token::FOR => {
                    if !cur.get_first_child(compiler).unwrap().is_empty(compiler) {
                        let init = cur.get_first_child(compiler).unwrap();

                        if init.is_let(compiler)
                            || init.is_const(compiler)
                            || init.is_class(compiler)
                            || init.is_function(compiler)
                        {
                            c = next;
                            continue;
                        }

                        let empty = IR::empty(compiler);
                        empty.srcref_if_missing(compiler, cur);
                        init.replace_with(compiler, empty);

                        // Only VAR statements, and expressions are allowed,
                        // but are handled differently.
                        let new_statement = if init.is_var(compiler) {
                            init
                        } else {
                            NodeUtil::new_expr(compiler, init)
                        };

                        new_statement.insert_before(compiler, insert_before);
                        self.report_code_change(compiler, "FOR initializer", n);
                    }
                }
                _ => {}
            }
            c = next;
        }
    }

    /// Split a var (or let or const) node such as:
    ///
    /// var a, b;
    ///
    /// into individual statements:
    ///
    /// var a; var b;
    ///
    /// `n`: The whose children we should inspect.
    // port: Normalize.NormalizeStatements#splitVarDeclarations
    fn split_var_declarations(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut next;
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            next = cur.get_next(compiler);
            if NodeUtil::is_name_declaration(compiler, Some(cur)) {
                if self.assert_on_change && !cur.has_children(compiler) {
                    panic!("Empty VAR node.");
                }

                while cur.get_first_child(compiler) != cur.get_last_child(compiler) {
                    let name = cur.get_first_child(compiler).unwrap();
                    name.detach(compiler);
                    let token = cur.get_token(compiler);
                    let new_var = compiler
                        .new_node_with_child(token, name)
                        .srcref(compiler, n);
                    new_var.insert_before(compiler, cur);
                    self.report_code_change(compiler, "VAR with multiple children", n);
                }
            }
            c = next;
        }
    }

    /// Move all the functions that are valid at the execution of the first statement of the
    /// function or static block to the beginning of the function definition or class static
    /// block.
    // port: Normalize.NormalizeStatements#moveNamedFunctions
    fn move_named_functions(&self, compiler: &mut AbstractCompiler, body: NodeId) {
        check_state!(
            body.get_parent(compiler).unwrap().is_function(compiler)
                || NodeUtil::is_class_static_block(compiler, body)
        );
        let mut insert_after = None;
        let mut current = body.get_first_child(compiler);
        // Skip any declarations at the beginning of the function body, they
        // are already in the right place.
        while let Some(cur) = current
            && NodeUtil::is_function_declaration(compiler, cur)
        {
            insert_after = Some(cur);
            current = cur.get_next(compiler);
        }

        // Find any remaining declarations and move them.
        while let Some(cur) = current {
            // Save off the next node as the current node maybe removed.
            let next = cur.get_next(compiler);
            if NodeUtil::is_function_declaration(compiler, cur) {
                // Remove the declaration from the body.
                cur.detach(compiler);

                // Read the function at the top of the function body (after any
                // previous declarations).
                insert_after = Some(Self::add_to_front(compiler, body, cur, insert_after));
                self.report_code_change(
                    compiler,
                    "Move function declaration not at top of function or class static block",
                    body,
                );
            }
            current = next;
        }
    }

    // port: Normalize.NormalizeStatements#normalizeAssignShorthand
    fn normalize_assign_shorthand(&self, compiler: &mut AbstractCompiler, shorthand: NodeId) {
        if !shorthand
            .get_first_child(compiler)
            .unwrap()
            .is_name(compiler)
        {
            return;
        }

        check_argument!(
            shorthand.has_two_children(compiler),
            "Expected two children for shorthand assignment."
        );
        let lhs = shorthand.get_first_child(compiler).unwrap();
        let rhs = shorthand.get_second_child(compiler).unwrap();
        lhs.detach(compiler);
        rhs.detach(compiler);

        let op = NodeUtil::get_op_from_assignment_op(compiler, shorthand);
        let lhs_clone = lhs.clone_node(compiler).srcref(compiler, lhs);
        let op_node = compiler
            .new_node_with_children2(op, lhs_clone, rhs)
            .srcref(compiler, shorthand)
            .copy_type_from(compiler, shorthand);

        // Change the original ASSIGN_X to an ASSIGN.
        shorthand.set_token(compiler, Token::ASSIGN);
        shorthand.add_child_to_back(compiler, lhs);
        shorthand.add_child_to_back(compiler, op_node);

        self.report_code_change(compiler, "assign shorthand", shorthand);
    }

    /// `after`: The child node to insert the newChild after, or null if newChild should be added
    /// to the front of parent's child list.
    /// Returns the inserted child node.
    // port: Normalize.NormalizeStatements#addToFront
    fn add_to_front(
        ast: &mut Ast,
        parent: NodeId,
        new_child: NodeId,
        after: Option<NodeId>,
    ) -> NodeId {
        match after {
            None => parent.add_child_to_front(ast, new_child),
            Some(after) => new_child.insert_after(ast, after),
        }
        new_child
    }
}

impl ScopedCallback for NormalizeStatements {
    // port: Normalize.NormalizeStatements#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(make_declared_names_unique) = &mut self.make_declared_names_unique {
            make_declared_names_unique.enter_scope(t);
        }
    }

    // port: Normalize.NormalizeStatements#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(make_declared_names_unique) = &mut self.make_declared_names_unique {
            make_declared_names_unique.exit_scope(t);
        }
    }
}

impl Callback for NormalizeStatements {
    // port: Normalize.NormalizeStatements#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if let Some(make_declared_names_unique) = &mut self.make_declared_names_unique {
            let _unused = make_declared_names_unique.should_traverse(t, n, parent);
        }

        let old_parent = n.get_parent(t);
        self.do_statement_normalizations(t.get_compiler(), n);
        check_state!(
            n.get_parent(t) == old_parent,
            "n's parent changed during normalization. It needs to stay the same so that we properly visit any added nodes."
        );
        true
    }

    // port: Normalize.NormalizeStatements#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if let Some(make_declared_names_unique) = &mut self.make_declared_names_unique {
            make_declared_names_unique.visit(t, n, parent);
        }

        match n.get_token(t) {
            Token::WHILE => {
                let expr = n.get_first_child(t).unwrap();
                n.set_token(t, Token::FOR);
                let empty = IR::empty(t);
                empty.srcref_if_missing(t, n);
                empty.insert_before(t, expr);
                let empty_clone = empty.clone_node(t);
                empty_clone.insert_after(t, expr);
                self.report_code_change(t.get_compiler(), "WHILE node", n);
            }
            Token::ARRAYLIT
            | Token::CALL
            | Token::PARAM_LIST
            | Token::NEW
            | Token::OBJECTLIT
            | Token::OPTCHAIN_CALL => n.set_trailing_comma(t, false),
            Token::NAME => {
                self.annotate_constants_by_convention(t.get_compiler(), n);
                self.annotate_function_expression_name_as_constant(t, n);
            }
            Token::DESTRUCTURING_LHS => self.normalize_destructuring_lhs(t.get_compiler(), n),
            Token::ASSIGN_OR | Token::ASSIGN_AND | Token::ASSIGN_COALESCE => self
                .rewrite_logical_assignment_operators_helper
                .visit_logical_assignment_operator(t, n),
            _ => {}
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

/// ScopeCreator duplicate declaration handler.
struct DuplicateDeclarationHandler<'a> {
    // The enclosing Normalize (Java inner class): its astFactory and reportCodeChange.
    normalize: &'a Normalize,
    // Java's LinkedHashSet<Var> compares vars as ScopedNames: by name and scope root.
    has_ok_duplicate_declaration: IndexSet<(JsString, NodeId)>,
}

impl<'a> DuplicateDeclarationHandler<'a> {
    // port: Normalize.DuplicateDeclarationHandler#DuplicateDeclarationHandler
    fn new(normalize: &'a Normalize) -> Self {
        Self {
            normalize,
            has_ok_duplicate_declaration: IndexSet::<_>::default(),
        }
    }

    /// Remove the parent VAR. There is only one case that need to be handled: "var a = b;" which
    /// is replaced with "a = b"
    ///
    /// Cases we don't need to handle are VARs with multiple children, which have already been
    /// split into separate declarations, so there is no need to handle that here; "for (var
    /// a;;);"/"for (var a of b)"/"for (var a in b)", which have been moved out of the loop; and
    /// "LABEL: var x;" which has been put in a BLOCK
    ///
    /// The result of this is that in each case the parent node is replaced which is generally
    /// dangerous in a traversal but is fine here with the scope creator, as the next node of
    /// interest is the parent's next sibling.
    // port: Normalize.DuplicateDeclarationHandler#replaceVarWithAssignment
    fn replace_var_with_assignment(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: NodeId,
        grandparent: NodeId,
    ) {
        if n.has_children(compiler) {
            // The  *  is being initialize, preserve the new value.
            n.detach(compiler);
            // Convert "var name = value" to "name = value"
            let value = n.get_first_child(compiler).unwrap();
            value.detach(compiler);
            let replacement = self.normalize.ast_factory.create_assign(compiler, n, value);
            let info = parent.get_jsdoc_info(compiler);
            replacement.set_jsdoc_info(compiler, info);
            replacement.srcref_if_missing(compiler, parent);
            let statement = NodeUtil::new_expr(compiler, replacement);
            parent.replace_with(compiler, statement);
            self.normalize
                .report_code_change(compiler, "Duplicate VAR declaration", statement);
        } else {
            // It is an empty reference. Remove it.
            check_state!(
                NodeUtil::is_statement_block(compiler, grandparent),
                "%s",
                grandparent.to_string(compiler)
            );
            parent.detach(compiler);
            self.normalize
                .report_code_change(compiler, "Duplicate VAR declaration", grandparent);
        }
    }
}

impl RedeclarationHandler for DuplicateDeclarationHandler<'_> {
    /// Remove duplicate VAR declarations discovered during scope creation.
    // port: Normalize.DuplicateDeclarationHandler#onRedeclaration
    fn on_redeclaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        s: ScopeId,
        name: &JsString,
        n: NodeId,
        input: Option<CompilerInput>,
    ) {
        check_state!(n.is_name(compiler));
        let parent = n.get_parent(compiler).unwrap();
        let v = s.get_var(compiler, name.clone()).unwrap();

        if s.is_global(compiler) {
            // We allow variables to be duplicate declared if one
            // declaration appears in source and the other in externs.
            // This deals with issues where a browser built-in is declared
            // in one browser but not in another.
            if v.is_extern(compiler) && !input.as_ref().unwrap().is_extern() {
                let key = (v.get_name(compiler), v.get_scope_root(compiler));
                if self.has_ok_duplicate_declaration.insert(key) {
                    return;
                }
            }
        }

        if parent.is_function(compiler) {
            if v.get_parent_node(compiler).unwrap().is_var(compiler) {
                s.undeclare(compiler, v);
                let v_input = v.get_input(compiler);
                s.declare(compiler, name.clone(), n, v_input);
                let name_node = v.get_name_node(compiler).unwrap();
                let parent_node = v.get_parent_node(compiler).unwrap();
                let grandparent = parent_node.get_parent(compiler).unwrap();
                self.replace_var_with_assignment(compiler, name_node, parent_node, grandparent);
            }
        } else if parent.is_var(compiler) {
            check_state!(parent.has_one_child(compiler));
            let grandparent = parent.get_parent(compiler).unwrap();
            self.replace_var_with_assignment(compiler, n, parent, grandparent);
        }
    }
}
