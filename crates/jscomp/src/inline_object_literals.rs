/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/InlineObjectLiterals.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
    reference::Reference,
    reference_collection::ReferenceCollection,
    reference_collector::{Behavior, ReferenceCollector},
    reference_map::ReferenceMap,
    scope::ScopeId,
    syntactic_scope_creator::SyntacticScopeCreator,
    var::VarId,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state, ir::IR, js_string::JsString, node::NodeId, token::Token, token_stream::TokenStream,
};
use std::sync::Arc;

/// Using the infrastructure provided by ReferenceCollector, identify variables that are only ever
/// assigned to object literals and that are never used in entirety, and expand the objects into
/// individual variables.
///
/// Based on the InlineVariables pass
pub struct InlineObjectLiterals {
    safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
}

impl InlineObjectLiterals {
    pub const VAR_PREFIX: &'static str = "JSCompiler_object_inline_";
    pub const STRING_KEY_IDENTIFIER: &'static str = "string_key";

    // port: InlineObjectLiterals#InlineObjectLiterals
    pub fn new(safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>) -> Self {
        Self {
            safe_name_id_supplier,
        }
    }
}

impl CompilerPass for InlineObjectLiterals {
    // port: InlineObjectLiterals#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut behavior = InliningBehavior {
            safe_name_id_supplier: self.safe_name_id_supplier.clone(),
            stale_vars: IndexSet::<_>::default(),
        };
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut callback = ReferenceCollector::new(compiler, &mut behavior, &mut scope_creator);
        CompilerPass::process(&mut callback, compiler, externs, root);
    }
}

/// Builds up information about nodes in each scope. When exiting the scope, inspects all variables
/// in that scope, and inlines any that we can.
// port: InlineObjectLiterals.InliningBehavior
struct InliningBehavior {
    // Java: the outer class's safeNameIdSupplier, read by this inner class.
    safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    /// A list of variables that should not be inlined, because their reference information is out
    /// of sync with the state of the AST.
    // Java: Set<Var> (LinkedHashSet). Var's equals/hashCode are ScopedName's (name and scope
    // root), so the set is keyed by exactly those two values.
    stale_vars: IndexSet<(JsString, NodeId)>,
}

impl Behavior for InliningBehavior {
    // port: InlineObjectLiterals.InliningBehavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        for v in scope.get_var_iterable(compiler) {
            if self.is_var_inline_forbidden(compiler, v) {
                continue;
            }

            let reference_info = reference_map.get_references(v).unwrap();

            if self.is_inlinable_object(compiler, &reference_info.references) {
                // Skiplist the object itself, as well as any other values
                // that it refers to, since they will have been moved around.
                Self::add_stale_var(&mut self.stale_vars, compiler, Some(v));

                let init = reference_info.get_initializing_reference(compiler);

                // Split up the object into individual variables if the object
                // is never referenced directly in full.
                self.split_object(compiler, v, init, reference_info);
            }
        }
    }
}

impl InliningBehavior {
    // Java: staleVars.add(var) (ScopedName equality; a null Var is never looked up again).
    fn add_stale_var(
        stale_vars: &mut IndexSet<(JsString, NodeId)>,
        compiler: &AbstractCompiler,
        var: Option<VarId>,
    ) {
        if let Some(var) = var {
            stale_vars.insert((var.get_name(compiler), var.get_scope_root(compiler)));
        }
    }

    /// If there are any variable references in the given node tree, skiplist them to prevent the
    /// pass from trying to inline the variable. Any code modifications will have potentially made
    /// the ReferenceCollection invalid.
    // port: InlineObjectLiterals.InliningBehavior#recordStaleVarReferencesInTree
    fn record_stale_var_references_in_tree(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        scope: ScopeId,
    ) {
        // The visitor only reads the tree; Scope#getVar needs the compiler (lazy implicit vars),
        // so the NAME strings are collected in visit order and looked up in that order.
        let mut names: Vec<JsString> = Vec::new();
        NodeUtil::visit_pre_order_with_predicate(
            compiler,
            root,
            &mut |ast: &mut closure_rhino::node::Ast, node: NodeId| {
                if node.is_name(ast) {
                    names.push(node.get_string(ast));
                }
            },
            &NodeUtil::MATCH_NOT_FUNCTION,
        );
        for name in names {
            let var = scope.get_var(compiler, name);
            Self::add_stale_var(&mut self.stale_vars, compiler, var);
        }
    }

    /// Whether the given variable is forbidden from being inlined.
    // port: InlineObjectLiterals.InliningBehavior#isVarInlineForbidden
    fn is_var_inline_forbidden(&self, compiler: &AbstractCompiler, var: VarId) -> bool {
        // A variable may not be inlined if:
        // 1) The variable is defined in the externs
        // 2) The variable is exported,
        // 3) Don't inline the special RENAME_PROPERTY_FUNCTION_NAME
        // 4) A reference to the variable has been inlined. We're downstream
        //    of the mechanism that creates variable references, so we don't
        //    have a good way to update the reference. Just punt on it.

        // Additionally, exclude global variables for now.

        var.is_global(compiler)
            || var.is_extern(compiler)
            || compiler
                .get_coding_convention()
                .is_exported(&var.get_name(compiler), /* local= */ true)
            || compiler
                .get_coding_convention()
                .is_property_rename_function(compiler, var.get_name_node(compiler).unwrap())
            || self
                .stale_vars
                .contains(&(var.get_name(compiler), var.get_scope_root(compiler)))
    }

    /// Counts the number of direct (full) references to an object. Specifically, we check for
    /// references of the following type:
    ///
    /// ```text
    ///   x;
    ///   x.fn();
    /// ```
    // port: InlineObjectLiterals.InliningBehavior#isInlinableObject
    fn is_inlinable_object(&self, compiler: &AbstractCompiler, refs: &[Reference]) -> bool {
        let ast = compiler;
        let mut ret = false;
        let mut valid_properties: IndexSet<JsString> = IndexSet::<_>::default();
        for r in refs {
            let name = r.get_node();
            let parent = r.get_parent(ast);
            let grandparent = r.get_grandparent(ast);

            // Ignore most indirect references, like x.y (but not x.y(),
            // since the function referenced by y might reference 'this').
            //
            if NodeUtil::is_normal_or_opt_chain_get_prop(ast, parent) {
                check_state!(parent.get_first_child(ast) == Some(name));
                // A call target may be using the object as a 'this' value.
                if NodeUtil::is_invocation_target(ast, parent) {
                    return false;
                }

                // Deleting a property has different semantics from deleting
                // a variable, so deleted properties should not be inlined.
                let grandparent = grandparent.unwrap();
                if grandparent.is_del_prop(ast) {
                    return false;
                }

                // NOTE(nicksantos): This pass's object-splitting algorithm has
                // a blind spot. It assumes that if a property isn't defined on an
                // object, then the value is undefined. This is not true, because
                // Object.prototype can have arbitrary properties on it.
                //
                // We short-circuit this problem by bailing out if we see a reference
                // to a property that isn't defined on the object literal. This
                // isn't a perfect algorithm, but it should catch most cases.
                let prop_name = parent.get_string(ast);
                if !valid_properties.contains(&prop_name) {
                    if NodeUtil::is_name_decl_or_simple_assign_lhs(ast, parent, grandparent) {
                        valid_properties.insert(prop_name);
                    } else {
                        return false;
                    }
                }
                continue;
            }

            // Only rewrite VAR declarations or simple assignment statements
            if !Self::is_var_or_assign_expr_lhs(compiler, name) {
                return false;
            }

            // Don't try to handle rewriting VAR/CONST/LET declarations inside for loops.
            // Currently, normalization moves var declarations out of for loop initializers anyway.
            // let/const are more difficult. Declaring each property outside the for loop puts them
            // in an incorrect scope. Declaring them in the loop would initialize them multiple
            // times.
            if NodeUtil::is_name_declaration(ast, Some(parent))
                && NodeUtil::is_any_for(ast, grandparent.unwrap())
            {
                return false;
            }

            let Some(val) = r.get_assigned_value(ast) else {
                // A var with no assignment.
                continue;
            };

            // We're looking for object literal assignments only.
            if !val.is_object_lit(ast) {
                return false;
            }

            // Make sure that the value is not self-referential. IOW,
            // disallow things like x = {b: x.a}.
            //
            // TODO(dimvar): Only exclude unorderable self-referential
            // assignments. i.e. x = {a: x.b, b: x.a} is not orderable,
            // but x = {a: 1, b: x.a} is.
            //
            // Also, ES5 getters/setters aren't handled by this pass.
            let mut child = val.get_first_child(ast);
            while let Some(c) = child {
                match c.get_token(ast) {
                    Token::GETTER_DEF
                    | Token::SETTER_DEF
                    | Token::COMPUTED_PROP
                    | Token::OBJECT_SPREAD => {
                        // ES5 get/set not supported.
                        // Don't inline computed property names
                        // Spread can overwrite any preceding prop if there are matching keys.
                        // TODO(b/126567617): Allow inlining props declared after the SPREAD.
                        return false;
                    }
                    Token::MEMBER_FUNCTION_DEF | Token::STRING_KEY => {}
                    _ => panic!("Unexpected child of OBJECTLIT: {}", c.to_string_tree(ast)),
                }

                valid_properties.insert(c.get_string(ast));

                let child_val = c.get_first_child(ast);
                // Check if childVal is the parent of any of the passed in
                // references, as that is how self-referential assignments
                // will happen.
                for t in refs {
                    let mut ref_node = t.get_parent(ast);
                    while !NodeUtil::is_statement_block(ast, ref_node) {
                        if Some(ref_node) == child_val {
                            // There's a self-referential assignment
                            return false;
                        }
                        ref_node = ref_node.get_parent(ast).unwrap();
                    }
                }
                child = c.get_next(ast);
            }

            // We have found an acceptable object literal assignment. As
            // long as there are no other assignments that mess things up,
            // we can inline.
            ret = true;
        }
        ret
    }

    // port: InlineObjectLiterals.InliningBehavior#isVarOrAssignExprLhs
    fn is_var_or_assign_expr_lhs(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let ast = compiler;
        let parent = n.get_parent(ast).unwrap();
        NodeUtil::is_name_declaration(ast, Some(parent))
            || (parent.is_assign(ast)
                && parent.get_first_child(ast) == Some(n)
                && parent.get_parent(ast).unwrap().is_expr_result(ast))
    }

    /// Computes a list of ever-referenced keys in the object being inlined, and returns a mapping
    /// of key name -> generated variable name.
    // port: InlineObjectLiterals.InliningBehavior#computeVarList
    fn compute_var_list(
        &self,
        compiler: &AbstractCompiler,
        reference_info: &ReferenceCollection,
    ) -> IndexMap<JsString, JsString> {
        let ast = compiler;
        let mut varmap: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();

        for r in &reference_info.references {
            if r.is_lvalue(ast) || r.is_initializing_declaration(ast) {
                if let Some(val) = r.get_assigned_value(ast) {
                    check_state!(val.is_object_lit(ast), "%s", val.to_string(ast));
                    let mut child = val.get_first_child(ast);
                    while let Some(c) = child {
                        let varname = c.get_string(ast);
                        if varmap.contains_key(&varname) {
                            child = c.get_next(ast);
                            continue;
                        }
                        let mut var = varname.clone();
                        if !TokenStream::is_js_identifier(&varname) {
                            var = JsString::from(InlineObjectLiterals::STRING_KEY_IDENTIFIER);
                        }
                        var = JsString::from(InlineObjectLiterals::VAR_PREFIX)
                            .concat(&var)
                            .concat(&JsString::from("_"))
                            .concat(&JsString::from((self.safe_name_id_supplier)()));
                        varmap.insert(varname, var);
                        child = c.get_next(ast);
                    }
                }
            } else if NodeUtil::is_name_declaration(ast, Some(r.get_parent(ast))) {
                // This is the var. There is no value.
            } else {
                let getprop = r.get_parent(ast);
                check_state!(
                    NodeUtil::is_normal_or_opt_chain_get_prop(ast, getprop),
                    "%s",
                    getprop.to_string(ast)
                );

                // The key being looked up in the original map.
                let varname = getprop.get_string(ast);
                if varmap.contains_key(&varname) {
                    continue;
                }

                let var = JsString::from(InlineObjectLiterals::VAR_PREFIX)
                    .concat(&varname)
                    .concat(&JsString::from("_"))
                    .concat(&JsString::from((self.safe_name_id_supplier)()));
                varmap.insert(varname, var);
            }
        }

        varmap
    }

    /// Populates a map of key names -> initial assigned values. The object literal these are being
    /// pulled from is invalidated as a result.
    // port: InlineObjectLiterals.InliningBehavior#fillInitialValues
    fn fill_initial_values(
        compiler: &mut AbstractCompiler,
        init: &Reference,
        initvals: &mut IndexMap<JsString, Option<NodeId>>,
    ) {
        let object = init.get_assigned_value(compiler).unwrap();
        check_state!(
            object.is_object_lit(compiler),
            "%s",
            object.to_string(compiler)
        );
        let mut key = object.get_first_child(compiler);
        while let Some(k) = key {
            let name = k.get_string(compiler);
            let value = k.remove_first_child(compiler);
            initvals.insert(name, value);
            key = k.get_next(compiler);
        }
    }

    /// Replaces an assignment like x = {...} with t1=a,t2=b,t3=c,true. Note that the resulting
    /// expression will always evaluate to true, as would the x = {...} expression.
    // port: InlineObjectLiterals.InliningBehavior#replaceAssignmentExpression
    fn replace_assignment_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        r: &Reference,
        varmap: &IndexMap<JsString, JsString>,
    ) {
        // Compute all of the assignments necessary
        let mut nodes: Vec<NodeId> = Vec::new();
        let val = r.get_assigned_value(compiler).unwrap();
        let scope = v.get_scope(compiler);
        self.record_stale_var_references_in_tree(compiler, val, scope);
        check_state!(val.is_object_lit(compiler), "%s", val.to_string(compiler));
        let mut all: IndexSet<JsString> = varmap.keys().cloned().collect();
        let mut key = val.get_first_child(compiler);
        while let Some(k) = key {
            let var = k.get_string(compiler);
            let value = k.remove_first_child(compiler).unwrap();
            // TODO(user): Copy type information.
            let name = IR::name(compiler, varmap[&var].clone());
            nodes.push(IR::assign(compiler, name, value));
            all.shift_remove(&var);
            key = k.get_next(compiler);
        }

        // TODO(user): Better source information.
        for var in &all {
            let name = IR::name(compiler, varmap[var].clone());
            let undefined = NodeUtil::new_undefined_node(compiler, None);
            nodes.push(IR::assign(compiler, name, undefined));
        }

        let replacement;
        if nodes.is_empty() {
            replacement = IR::true_node(compiler);
        } else {
            // All assignments evaluate to true, so make sure that the
            // expr statement evaluates to true in case it matters.
            nodes.push(IR::true_node(compiler));

            // Join these using COMMA.  A COMMA node must have 2 children, so we
            // create a tree. In the tree the first child be the COMMA to match
            // the parser, otherwise tree equality tests fail.
            nodes.reverse();
            replacement = compiler.new_node(Token::COMMA);
            let mut cur = replacement;
            let mut i = 0;
            while i < nodes.len() - 2 {
                cur.add_child_to_front(compiler, nodes[i]);
                let t = compiler.new_node(Token::COMMA);
                cur.add_child_to_front(compiler, t);
                cur = t;
                i += 1;
            }
            cur.add_child_to_front(compiler, nodes[i]);
            cur.add_child_to_front(compiler, nodes[i + 1]);
        }

        let replace = r.get_parent(compiler);
        replacement.srcref_tree_if_missing(compiler, replace);

        if NodeUtil::is_name_declaration(compiler, Some(replace)) {
            let expr = NodeUtil::new_expr(compiler, replacement);
            replace.replace_with(compiler, expr);
        } else {
            replace.replace_with(compiler, replacement);
        }
    }

    /// Splits up the object literal into individual variables, and updates all uses.
    // port: InlineObjectLiterals.InliningBehavior#splitObject
    fn split_object(
        &mut self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        init: Option<&Reference>,
        reference_info: &ReferenceCollection,
    ) {
        // First figure out the FULL set of possible keys, so that they
        // can all be properly set as necessary.
        let varmap = self.compute_var_list(compiler, reference_info);

        let mut initvals: IndexMap<JsString, Option<NodeId>> = IndexMap::<_, _>::default();
        // Figure out the top-level of the var assign node. If it's a plain
        // ASSIGN, then there's an EXPR_STATEMENT above it, if it's a
        // VAR then it should be directly replaced.
        let mut vnode;
        let defined = reference_info.is_well_defined(compiler)
            && NodeUtil::is_name_declaration(compiler, Some(init.unwrap().get_parent(compiler)));
        if defined {
            let init = init.unwrap();
            vnode = init.get_parent(compiler);
            Self::fill_initial_values(compiler, init, &mut initvals);
        } else if v.get_parent_node(compiler).unwrap().is_let(compiler)
            || v.get_parent_node(compiler).unwrap().is_const(compiler)
        {
            // Find the beginning of the current scope.
            let scope_root = v.get_scope(compiler).get_root_node(compiler);
            // Assuming scopeRoot is a BLOCK, then we want to insert at the top of the block,
            // before the first statement.
            vnode = scope_root.get_first_child(compiler).unwrap();
            // Some scope-creating nodes might not be BLOCK nodes
            if !NodeUtil::is_statement(compiler, vnode)
                && NodeUtil::is_statement(compiler, scope_root)
            {
                vnode = scope_root;
            } else if scope_root.is_switch_body(compiler) {
                vnode = scope_root.get_parent(compiler).unwrap();
            }
        } else {
            // Find the beginning of the function body / script.
            vnode = v
                .get_scope(compiler)
                .get_closest_hoist_scope(compiler)
                .unwrap()
                .get_root_node(compiler)
                .get_first_child(compiler)
                .unwrap();
        }

        check_state!(
            NodeUtil::is_statement(compiler, vnode),
            "%s",
            vnode.to_string(compiler)
        );

        for (key, value) in &varmap {
            let val = initvals.get(key).copied().flatten();
            let new_var_node = NodeUtil::new_var_node(compiler, value.clone(), val);
            match val {
                None => {
                    // is this right?
                    new_var_node.srcref_tree_if_missing(compiler, vnode);
                }
                Some(val) => {
                    let scope = v.get_scope(compiler);
                    self.record_stale_var_references_in_tree(compiler, val, scope);
                }
            }
            new_var_node.insert_before(compiler, vnode);
            compiler.report_change_to_enclosing_scope(vnode);
        }

        if defined {
            let vnode_parent = vnode.get_parent(compiler).unwrap();
            compiler.report_change_to_enclosing_scope(vnode_parent);
            vnode.detach(compiler);
        }

        for r in &reference_info.references {
            // The init/decl have already been converted.
            if defined && init.is_some_and(|init| std::ptr::eq(r, init)) {
                continue;
            }
            compiler.report_change_to_enclosing_scope(r.get_node());

            if r.is_lvalue(compiler) {
                // Assignments have to be handled specially, since they
                // expand out into multiple assignments.
                self.replace_assignment_expression(compiler, v, r, &varmap);
            } else if NodeUtil::is_name_declaration(compiler, Some(r.get_parent(compiler))) {
                // The old variable declaration. It didn't have a
                // value. Remove it entirely as it should now be unused.
                r.get_parent(compiler).detach(compiler);
            } else {
                // Make sure that the reference is a GETPROP as we expect it to be.
                let getprop = r.get_parent(compiler);
                check_state!(
                    NodeUtil::is_normal_or_opt_chain_get_prop(compiler, getprop),
                    "%s",
                    getprop.to_string(compiler)
                );

                // The key being looked up in the original map.
                let var = getprop.get_string(compiler);

                // If the variable hasn't already been declared, add an empty
                // declaration near all the other declarations.
                check_state!(varmap.contains_key(&var));

                // Replace the GETPROP node with a NAME.
                let replacement = IR::name(compiler, varmap[&var].clone());
                replacement.srcref_if_missing(compiler, getprop);
                r.get_parent(compiler).replace_with(compiler, replacement);
            }
        }
    }
}
