/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/FunctionToBlockMutator.java.

//! Port of `com.google.javascript.jscomp.FunctionToBlockMutator`.
//!
//! A class to transform the body of a function into a generic block suitable for inlining.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_coding_convention::ClosureCodingConvention,
    coding_convention::CodingConvention,
    function_argument_injector::{FunctionArgumentInjector, ParamArgPair, THIS_MARKER},
    make_declared_names_unique::{InlineRenamer, MakeDeclaredNamesUnique},
    node_traversal::NodeTraversal,
    node_util::{MatchShallowStatement, NodeUtil},
    rename_labels::RenameLabels,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::Arc;

pub struct FunctionToBlockMutator {
    safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    function_argument_injector: FunctionArgumentInjector,
}

impl FunctionToBlockMutator {
    // port: FunctionToBlockMutator#FunctionToBlockMutator
    pub fn new(
        compiler: &AbstractCompiler,
        safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    ) -> Self {
        Self {
            safe_name_id_supplier,
            function_argument_injector: FunctionArgumentInjector::new(compiler.get_ast_analyzer()),
        }
    }

    /// Returns a clone of the function body mutated to be suitable for injection as a statement
    /// into another code block.
    ///
    /// `fn_name` is the name to use when preparing human readable names, `fn_node` the function
    /// to prepare, `call_node` the call node that will be replaced, `result_name` the name
    /// function results should be assigned to, `needs_default_result` whether the result value
    /// must be set, and `is_call_in_loop` whether the function body must be prepared to be
    /// injected into the body of a loop.
    // port: FunctionToBlockMutator#mutate
    #[allow(clippy::too_many_arguments)] // Java's signature.
    pub fn mutate(
        &self,
        compiler: &mut AbstractCompiler,
        fn_name: &JsString,
        fn_node: NodeId,
        call_node: NodeId,
        result_name: Option<&JsString>,
        needs_default_result: bool,
        is_call_in_loop: bool,
    ) -> NodeId {
        self.mutate_internal(
            compiler,
            fn_name,
            fn_node,
            call_node,
            result_name,
            needs_default_result,
            is_call_in_loop,
            /* renameLocals= */ true,
        )
    }

    /// Used where the inlining occurs into an isolated scope such as a module. Renaming is
    /// avoided since the associated JSDoc annotations are not updated.
    // port: FunctionToBlockMutator#mutateWithoutRenaming
    #[allow(clippy::too_many_arguments)] // Java's signature.
    pub fn mutate_without_renaming(
        &self,
        compiler: &mut AbstractCompiler,
        fn_name: &JsString,
        fn_node: NodeId,
        call_node: NodeId,
        result_name: Option<&JsString>,
        needs_default_result: bool,
        is_call_in_loop: bool,
    ) -> NodeId {
        self.mutate_internal(
            compiler,
            fn_name,
            fn_node,
            call_node,
            result_name,
            needs_default_result,
            is_call_in_loop,
            /* renameLocals= */ false,
        )
    }

    // port: FunctionToBlockMutator#mutateInternal
    #[allow(clippy::too_many_arguments)]
    fn mutate_internal(
        &self,
        compiler: &mut AbstractCompiler,
        fn_name: &JsString,
        fn_node: NodeId,
        call_node: NodeId,
        result_name: Option<&JsString>,
        needs_default_result: bool,
        is_call_in_loop: bool,
        rename_locals: bool,
    ) -> NodeId {
        let new_fn_node = fn_node.clone_tree(compiler);
        // Wrap the clone in a script, in a root, so that makeLocalNamesUnique sees a coherent
        // tree.
        let script = IR::script_with_children(compiler, &[new_fn_node]);
        IR::root(compiler, &[script]);

        if rename_locals {
            // Now that parameter names have been replaced, make sure all the local
            // names are unique, to allow functions to be inlined multiple times
            // without causing conflicts.
            self.make_local_names_unique(compiler, new_fn_node, is_call_in_loop);

            // Function declarations must be rewritten as function expressions as
            // they will be within a block and normalization prevents function
            // declarations within block as browser implementations vary.
            let body = new_fn_node.get_last_child(compiler).unwrap();
            Self::rewrite_function_declarations(compiler, body);
        }

        // TODO(johnlenz): Mark NAME nodes constant for parameters that are not modified.
        let modified_parameters = self
            .function_argument_injector
            .find_modified_parameters(compiler, new_fn_node);
        let args = self
            .function_argument_injector
            .get_function_call_parameter_map(
                compiler,
                new_fn_node,
                call_node,
                &*self.safe_name_id_supplier,
            );
        let has_args = !args.is_empty();
        let mut all_names_to_alias: IndexSet<JsString> = IndexSet::<_>::default();
        if has_args {
            let temps = self
                .function_argument_injector
                .gather_call_arguments_needing_temps(
                    compiler,
                    new_fn_node,
                    &args,
                    &modified_parameters,
                    None,
                );
            all_names_to_alias = modified_parameters
                .iter()
                .chain(temps.iter())
                .cloned()
                .collect();
        }

        let new_block = NodeUtil::get_function_body(compiler, new_fn_node);
        // Make the newBlock insertable .
        new_block.detach(compiler);

        if has_args {
            let inline_result = self.alias_and_inline_arguments(
                compiler,
                new_block,
                &args,
                Some(&all_names_to_alias),
            );
            check_state!(new_block == inline_result);
        }

        //
        // For calls inlined into loops, VAR declarations are not reinitialized to
        // undefined as they would have been if the function were called, so ensure
        // that they are properly initialized.
        //
        if is_call_in_loop {
            Self::fix_uninitialized_var_declarations(compiler, new_block, new_block);
        }

        let label_name = self.get_label_name_for_function(fn_name);
        Self::replace_returns(
            compiler,
            new_block,
            result_name,
            &label_name,
            needs_default_result,
        )
    }

    // port: FunctionToBlockMutator#rewriteFunctionDeclarations
    fn rewrite_function_declarations(ast: &mut Ast, n: NodeId) -> Option<NodeId> {
        if n.is_function(ast) {
            if NodeUtil::is_function_declaration(ast, n) {
                // Rewrite: function f() {} ==> var f = function() {}
                let fn_name_node = n.get_first_child(ast).unwrap();

                let fn_name = fn_name_node.get_string(ast);
                let name = IR::name(ast, fn_name).srcref(ast, fn_name_node);
                let var = IR::var(ast, name).srcref(ast, n);

                fn_name_node.set_string(ast, "");
                // Add the VAR, remove the FUNCTION
                n.replace_with(ast, var);
                // readd the function as a function expression
                name.add_child_to_front(ast, n);

                return Some(var);
            }
            return None;
        }

        // Keep track of any rewritten functions and hoist them to the top
        // of the block they are defined in. This isn't fully compliant hoisting
        // but it does address a large set of use cases.
        let mut functions_to_hoist: Vec<NodeId> = Vec::new();
        let mut c = n.get_first_child(ast);
        while let Some(current) = c {
            let next = current.get_next(ast); // We may rewrite "c"
            let fn_var = Self::rewrite_function_declarations(ast, current);
            if let Some(fn_var) = fn_var {
                functions_to_hoist.insert(0, fn_var);
            }
            c = next;
        }

        for fn_var in functions_to_hoist {
            if n.get_first_child(ast) != Some(fn_var) {
                let detached = fn_var.detach(ast);
                n.add_child_to_front(ast, detached);
            }
        }

        None
    }

    /// For all VAR declarations (and LET declarations outside loops) that are uninitialized, set
    /// their initial values to "undefined" and hoist them to the containing block.
    ///
    /// When a function is called within a loop, local variables are recreated on each call in
    /// normal execution. When inlined into the loop, uninitialized variables would otherwise
    /// retain their values across loop iterations.
    // port: FunctionToBlockMutator#fixUninitializedVarDeclarations(Node,Node)
    fn fix_uninitialized_var_declarations(ast: &mut Ast, n: NodeId, containing_block: NodeId) {
        Self::fix_uninitialized_var_declarations_in_loop(
            ast,
            n,
            containing_block,
            /* inLoop= */ false,
        );
    }

    // port: FunctionToBlockMutator#fixUninitializedVarDeclarations(Node,Node,boolean)
    fn fix_uninitialized_var_declarations_in_loop(
        ast: &mut Ast,
        n: NodeId,
        containing_block: NodeId,
        in_loop: bool,
    ) {
        // Nested functions introduce there own var hoisting scope
        // thier `var` should not be hoisted out to the containing block.
        if n.is_function(ast) {
            return;
        }

        // Inner loop structures: var declarations inside loops are still function-scoped and
        // need to be initialized at the containing block level to avoid carrying values across
        // outer loop iterations. When hoisting, any assignment at the declaration site in the
        // loop must remain in the loop. Let declarations inside loops are block-scoped to that
        // loop and do not need hoisting.
        if NodeUtil::is_loop_structure(ast, n) {
            let body = NodeUtil::get_loop_code_block(ast, n);
            if let Some(body) = body {
                Self::fix_uninitialized_var_declarations_in_loop(
                    ast,
                    body,
                    containing_block,
                    /* inLoop= */ true,
                );
            }
            if n.is_vanilla_for(ast) {
                let init = n.get_first_child(ast);
                if let Some(init) = init
                    && !init.is_empty(ast)
                {
                    Self::fix_uninitialized_var_declarations_in_loop(
                        ast,
                        init,
                        containing_block,
                        /* inLoop= */ true,
                    );
                }
            }
            return;
        }

        if in_loop && n.is_var(ast) && n.has_one_child(ast) {
            let name = n.get_first_child(ast).unwrap();
            let src_location = name;
            let parent = n.get_parent(ast);
            // Hoist per-function declaration initialized to undefined
            let name_clone = name.clone_node(ast).set_jsdoc_info(ast, None);
            let hoisted_var = IR::var(ast, name_clone).srcref(ast, n);
            let undefined = NodeUtil::new_undefined_node(ast, Some(src_location));
            hoisted_var
                .get_first_child(ast)
                .unwrap()
                .add_child_to_back(ast, undefined);
            containing_block.add_child_to_front(ast, hoisted_var);

            if name.has_children(ast) {
                // If the var in the loop had an initializer, keep the assignment at the
                // declaration site in the loop.
                let value = name.remove_first_child(ast).unwrap();
                let detached_name = name.detach(ast);
                let assign = IR::assign(ast, detached_name, value).srcref(ast, n);
                if let Some(parent) = parent
                    && parent.is_vanilla_for(ast)
                    && parent.get_first_child(ast) == Some(n)
                {
                    n.replace_with(ast, assign);
                } else {
                    let expr = NodeUtil::new_expr(ast, assign);
                    n.replace_with(ast, expr);
                }
            } else {
                // If uninitialized in the loop, remove the var declaration from the loop.
                if let Some(parent) = parent
                    && parent.is_vanilla_for(ast)
                    && parent.get_first_child(ast) == Some(n)
                {
                    let empty = IR::empty(ast);
                    n.replace_with(ast, empty);
                } else {
                    n.detach(ast);
                }
            }
            return;
        }

        if !in_loop && (n.is_var(ast) || n.is_let(ast)) && n.has_one_child(ast) {
            let name = n.get_first_child(ast).unwrap();
            // It isn't initialized.
            if !name.has_children(ast) {
                let src_location = name;
                let undefined_node = NodeUtil::new_undefined_node(ast, Some(src_location));
                name.add_child_to_back(ast, undefined_node);
                let parent = n.get_parent(ast);
                if let Some(parent) = parent
                    && parent.is_vanilla_for(ast)
                    && parent.get_first_child(ast) == Some(n)
                {
                    let empty = IR::empty(ast);
                    n.replace_with(ast, empty);
                } else {
                    n.detach(ast);
                }
                containing_block.add_child_to_front(ast, n);
            }
            return;
        }

        let mut c = n.get_first_child(ast);
        while let Some(current) = c {
            Self::fix_uninitialized_var_declarations_in_loop(
                ast,
                current,
                containing_block,
                in_loop,
            );
            c = current.get_next(ast);
        }
    }

    /// Fix-up all local names to be unique for this subtree.
    // port: FunctionToBlockMutator#makeLocalNamesUnique
    fn make_local_names_unique(
        &self,
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
        is_call_in_loop: bool,
    ) {
        let id_supplier = compiler.get_unique_name_id_supplier();
        // Make variable names unique to this instance.
        let mut callback = MakeDeclaredNamesUnique::builder()
            .with_renamer(InlineRenamer::new(
                coding_convention_handle(compiler),
                id_supplier.clone(),
                "inline_",
                is_call_in_loop,
                true,
                None,
            ))
            .with_mark_changes(false)
            .build();
        NodeTraversal::traverse_scope_roots(compiler, &[fn_node], &mut callback, true);
        // Make label names unique to this instance.
        let label_name_supplier = LabelNameSupplier::new(id_supplier);
        RenameLabels::with_supplier(Arc::new(move || label_name_supplier.get()), false, false)
            .process(compiler, None, fn_node);
    }

    /// Create a unique label name.
    // port: FunctionToBlockMutator#getLabelNameForFunction
    fn get_label_name_for_function(&self, fn_name: &JsString) -> String {
        let name = if fn_name.is_empty() {
            JsString::from("anon")
        } else {
            fn_name.clone()
        };
        format!(
            "JSCompiler_inline_label_{}_{}",
            name,
            (self.safe_name_id_supplier)()
        )
    }

    /// Create a unique "this" name.
    // port: FunctionToBlockMutator#getUniqueThisName
    fn get_unique_this_name(&self) -> String {
        format!("JSCompiler_inline_this_{}", (self.safe_name_id_supplier)())
    }

    /// Inlines the arguments within the node tree using the given argument map, replaces
    /// "unsafe" names with local aliases.
    ///
    /// The aliases for unsafe require new VAR declarations, so this function can not be used in
    /// for direct CALL node replacement as VAR nodes can not be created there.
    ///
    /// Returns the node or its replacement.
    // port: FunctionToBlockMutator#aliasAndInlineArguments
    fn alias_and_inline_arguments(
        &self,
        compiler: &mut AbstractCompiler,
        fn_template_root: NodeId,
        param_to_arg_map: &IndexMap<JsString, ParamArgPair>,
        names_to_alias: Option<&IndexSet<JsString>>,
    ) -> NodeId {
        if names_to_alias.is_none_or(|names| names.is_empty()) {
            // There are no names to alias. Just inline the arguments directly.
            let mut replacements: IndexMap<JsString, NodeId> = IndexMap::<_, _>::default();
            for (key, value) in param_to_arg_map {
                replacements.insert(key.clone(), value.arg());
            }
            let result = self.function_argument_injector.inject(
                compiler,
                fn_template_root,
                None,
                &mut replacements,
            );
            check_state!(result == fn_template_root);
            result
        } else {
            let names_to_alias = names_to_alias.unwrap();
            // Create local alias of names that can not be safely used directly.

            // A map from function parameter to the value it should be replaced with
            // post-inlining. This is a subset of paramToArg: we exclude parameters for which we
            // create an explicit alias.
            let mut param_replacements: IndexMap<JsString, NodeId> = IndexMap::<_, _>::default();

            // Declare the aliases in the same order as the arguments are defined.
            let mut new_aliases_to_add: Vec<NodeId> = Vec::new();
            // NOTE: paramToArgMap is a linked map so we get the parameters in the order that
            // they were declared.
            for (name, arg) in param_to_arg_map {
                if !names_to_alias.contains(name) {
                    // Replace references to the parameter with the argument directly.
                    param_replacements.insert(name.clone(), arg.arg());
                    continue;
                }
                if *name == *THIS_MARKER {
                    let references_this =
                        NodeUtil::references_enclosing_receiver(compiler, fn_template_root);
                    // Update "this". We only need to create an alias if "this" is referenced
                    // and the value of "this" is not Token.THIS, or the value of "this" has
                    // side effects.
                    let original_value = arg.arg();
                    let mut replacement = original_value;
                    if !original_value.is_this(compiler)
                        && (references_this
                            || compiler
                                .get_ast_analyzer()
                                .may_have_side_effects(compiler, original_value))
                    {
                        let new_name = self.get_unique_this_name();
                        let new_value = original_value.clone_tree(compiler);
                        let new_node =
                            NodeUtil::new_var_node(compiler, new_name.as_str(), Some(new_value))
                                .srcref_tree_if_missing(compiler, new_value);
                        new_aliases_to_add.insert(0, new_node);
                        replacement =
                            IR::name(compiler, new_name.as_str()).srcref_tree(compiler, new_value);
                    }
                    param_replacements.insert(JsString::from(THIS_MARKER), replacement);
                } else {
                    // Add an alias for a given parameter/argument. For example, if inlining this
                    // function:
                    //
                    //  function f(x) { use(x, x); } f(computeValue());
                    //
                    // This code defines
                    //    var x = computeValue();
                    // (as use(computeValue(), computeValue()) would compute the value twice).
                    let new_name = arg.param_node().clone_node(compiler);
                    let new_value = arg.arg().clone_tree(compiler);
                    new_name.srcref(compiler, new_value); // source information should point to the argument.
                    let new_node = IR::var_with_value(compiler, new_name, new_value)
                        .srcref_tree_if_missing(compiler, new_value);
                    new_aliases_to_add.insert(0, new_node);
                }
            }

            // Inline the arguments.
            let result = self.function_argument_injector.inject(
                compiler,
                fn_template_root,
                None,
                &mut param_replacements,
            );
            check_state!(result == fn_template_root);

            // Now that the names have been replaced, add the new aliases for
            // the old names.
            for n in new_aliases_to_add {
                fn_template_root.add_child_to_front(compiler, n);
            }

            result
        }
    }

    /// Convert returns to assignments and breaks, as needed. For example, with a labelName of
    /// 'foo': { return a; } becomes: foo: { a; break foo; } or foo: { resultName = a; break foo; }
    ///
    /// Returns the node containing the transformed block, this may be different than the passed
    /// in node 'block'.
    // port: FunctionToBlockMutator#replaceReturns
    fn replace_returns(
        ast: &mut Ast,
        block: NodeId,
        result_name: Option<&JsString>,
        label_name: &str,
        result_must_be_set: bool,
    ) -> NodeId {
        let mut root = block;

        let mut has_return_at_exit = false;
        let mut return_count =
            NodeUtil::get_node_type_reference_count(ast, block, Token::RETURN, &|ast, n| {
                MatchShallowStatement.apply(ast, n)
            });
        if return_count > 0 {
            has_return_at_exit = Self::has_return_at_exit(ast, block);
            // TODO(johnlenz): Simpler not to special case this,
            // and let it be optimized later.
            if has_return_at_exit {
                Self::convert_last_return_to_statement(ast, block, result_name);
                return_count -= 1;
            }

            if return_count > 0 {
                // A label and breaks are needed.

                // Add the breaks
                Self::replace_return_with_break(ast, block, None, result_name, label_name);

                // Add label
                let name = IR::label_name(ast, label_name).srcref(ast, block);
                let label = IR::label(ast, name, block).srcref(ast, block);

                let new_root = IR::block(ast).srcref(ast, block);
                new_root.add_child_to_back(ast, label);

                // The label is now the root.
                root = new_root;
            }
        }

        // If there wasn't an return at the end of the function block, and we need
        // a result, add one to the block.
        if result_must_be_set
            && !has_return_at_exit
            && let Some(result_name) = result_name
        {
            Self::add_dummy_assignment(ast, block, result_name);
        }

        root
    }

    /// Example: a = (void) 0;
    // port: FunctionToBlockMutator#addDummyAssignment
    fn add_dummy_assignment(ast: &mut Ast, node: NodeId, result_name: &JsString) {
        check_argument!(node.is_block(ast));

        // A result is needed create a dummy value.
        let src_location = node;
        let ret_val = NodeUtil::new_undefined_node(ast, Some(src_location));
        let result_node = Self::create_assign_statement_node(ast, result_name, ret_val);
        result_node.srcref_tree_if_missing(ast, node);

        node.add_child_to_back(ast, result_node);
    }

    /// Replace the 'return' statement with its child expression. "return foo()" becomes "foo()"
    /// or "resultName = foo()" "return" is removed or becomes "resultName = void 0".
    // port: FunctionToBlockMutator#convertLastReturnToStatement
    fn convert_last_return_to_statement(
        ast: &mut Ast,
        block: NodeId,
        result_name: Option<&JsString>,
    ) {
        let ret = block.get_last_child(ast).unwrap();
        check_argument!(ret.is_return(ast));
        let result_node = Self::get_replacement_return_statement(ast, ret, result_name);

        match result_node {
            None => {
                ret.detach(ast);
            }
            Some(result_node) => {
                result_node.srcref_tree_if_missing(ast, ret);
                ret.replace_with(ast, result_node);
            }
        }
    }

    /// Create a valid statement Node containing an assignment to name of the given expression.
    // port: FunctionToBlockMutator#createAssignStatementNode
    fn create_assign_statement_node(ast: &mut Ast, name: &JsString, expression: NodeId) -> NodeId {
        // Create 'name = result-expression;' statement.
        // EXPR (ASSIGN (NAME, EXPRESSION))
        let name_node = IR::name(ast, name.clone());
        let assign = IR::assign(ast, name_node, expression);
        NodeUtil::new_expr(ast, assign)
    }

    /// Replace the 'return' statement with its child expression. If the result is needed
    /// (resultName != null): "return foo()" becomes "resultName = foo()" "return" becomes
    /// "resultName = void 0". Otherwise: "return foo()" becomes "foo()" "return", null is
    /// returned.
    // port: FunctionToBlockMutator#getReplacementReturnStatement
    fn get_replacement_return_statement(
        ast: &mut Ast,
        node: NodeId,
        result_name: Option<&JsString>,
    ) -> Option<NodeId> {
        let mut result_node = None;

        let mut ret_val = None;
        if node.has_children(ast) {
            // Clone the child as the child hasn't been removed
            // from the node yet.
            ret_val = Some(node.get_first_child(ast).unwrap().clone_tree(ast));
        }

        match result_name {
            None => {
                if let Some(ret_val) = ret_val {
                    result_node = Some(NodeUtil::new_expr(ast, ret_val)); // maybe null.
                }
            }
            Some(result_name) => {
                let ret_val = match ret_val {
                    Some(ret_val) => ret_val,
                    None => {
                        // A result is needed create a dummy value.
                        let src_location = node;
                        NodeUtil::new_undefined_node(ast, Some(src_location))
                    }
                };
                // Create a "resultName = retVal;" statement.
                result_node = Some(Self::create_assign_statement_node(
                    ast,
                    result_name,
                    ret_val,
                ));
            }
        }

        result_node
    }

    /// Returns whether the given block end with an return statement.
    // port: FunctionToBlockMutator#hasReturnAtExit
    fn has_return_at_exit(ast: &Ast, block: NodeId) -> bool {
        // Only inline functions that return something (empty returns
        // will be handled by ConstFolding+EmptyFunctionRemoval)
        block.get_last_child(ast).unwrap().is_return(ast)
    }

    /// Replace the 'return' statement with its child expression. "return foo()" becomes
    /// "{foo(); break;}" or "{resultName = foo(); break;}" "return" becomes {break;} or
    /// "{resultName = void 0;break;}".
    // port: FunctionToBlockMutator#replaceReturnWithBreak
    fn replace_return_with_break(
        ast: &mut Ast,
        mut current: NodeId,
        parent: Option<NodeId>,
        result_name: Option<&JsString>,
        label_name: &str,
    ) -> NodeId {
        if current.is_function(ast) || current.is_expr_result(ast) {
            // Don't recurse into functions definitions, and expressions can't
            // contain RETURN nodes.
            return current;
        }

        if current.is_return(ast) {
            check_state!(NodeUtil::is_statement_block(ast, parent.unwrap()));

            let result_node = Self::get_replacement_return_statement(ast, current, result_name);
            let label = IR::label_name(ast, label_name);
            let break_node = IR::break_node_with_label(ast, label);

            // Replace the node in parent, and reset current to the first new child.
            break_node.srcref_tree_if_missing(ast, current);
            current.replace_with(ast, break_node);
            if let Some(result_node) = result_node {
                result_node.srcref_tree_if_missing(ast, current);
                result_node.insert_before(ast, break_node);
            }
            current = break_node;
        } else {
            let mut c = current.get_first_child(ast);
            while let Some(child) = c {
                // c may be replaced.
                let child = Self::replace_return_with_break(
                    ast,
                    child,
                    Some(current),
                    result_name,
                    label_name,
                );
                c = child.get_next(ast);
            }
        }

        current
    }
}

pub struct LabelNameSupplier {
    id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
}

impl LabelNameSupplier {
    // port: FunctionToBlockMutator.LabelNameSupplier#LabelNameSupplier
    pub fn new(id_supplier: Arc<dyn Fn() -> String + Send + Sync>) -> Self {
        Self { id_supplier }
    }

    // port: FunctionToBlockMutator.LabelNameSupplier#get
    pub fn get(&self) -> String {
        format!("JSCompiler_inline_label_{}", (self.id_supplier)())
    }
}

/// Rust-only: `compiler.getCodingConvention()` as the shared handle InlineRenamer holds. Java's
/// compiler falls back to its own `new ClosureCodingConvention()`, which keeps no state, so a new
/// one stands for it.
fn coding_convention_handle(compiler: &AbstractCompiler) -> Arc<dyn CodingConvention> {
    match compiler.get_options().get_coding_convention() {
        Some(convention) => convention.clone(),
        None => Arc::new(ClosureCodingConvention::new()),
    }
}
