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
//   src/com/google/javascript/jscomp/RescopeGlobalSymbols.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    combined_compiler_pass::CombinedCompilerPass,
    compiler_options::OptimizeLocalAccess,
    compiler_pass::CompilerPass,
    node_traversal::{
        AbstractPostOrderCallbackInterface, AbstractShallowStatementCallback, Callback,
        NodeTraversal,
    },
    node_util::NodeUtil,
    rescope_global_symbols_rewrite_callback::{
        RescopeGlobalSymbolsRewriteCallback, SymbolInformation,
    },
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{ir::IR, js_string::JsString, node::NodeId};

/// Finds all references to global symbols and rewrites them to be property accesses to a special
/// object with the same name as the global symbol.
pub struct RescopeGlobalSymbols {
    global_symbol_namespace: JsString,
    add_extern: bool,
    assume_cross_chunk_names: bool,
    optimize_local_access: OptimizeLocalAccess,
}

impl RescopeGlobalSymbols {
    /// Constructor for the RescopeGlobalSymbols compiler pass.
    // port: RescopeGlobalSymbols#RescopeGlobalSymbols(AbstractCompiler, String, boolean, OptimizeLocalAccess)
    pub fn new(
        global_symbol_namespace: impl Into<JsString>,
        assume_cross_chunk_names: bool,
        optimize_local_access: OptimizeLocalAccess,
    ) -> Self {
        Self::new_with_add_extern(
            global_symbol_namespace,
            true,
            assume_cross_chunk_names,
            optimize_local_access,
        )
    }

    /// Constructor for the RescopeGlobalSymbols compiler pass for use in testing.
    // port: RescopeGlobalSymbols#RescopeGlobalSymbols(AbstractCompiler, String, boolean, boolean, OptimizeLocalAccess)
    pub fn new_with_add_extern(
        global_symbol_namespace: impl Into<JsString>,
        add_extern: bool,
        assume_cross_chunk_names: bool,
        optimize_local_access: OptimizeLocalAccess,
    ) -> Self {
        Self {
            global_symbol_namespace: global_symbol_namespace.into(),
            add_extern,
            assume_cross_chunk_names,
            optimize_local_access,
        }
    }

    // port: RescopeGlobalSymbols#addExternForGlobalSymbolNamespace
    fn add_extern_for_global_symbol_namespace(&self, compiler: &mut AbstractCompiler) {
        let name = IR::name(compiler, self.global_symbol_namespace.clone());
        let var_node = IR::var(compiler, name);
        let input = compiler.get_synthesized_externs_input().clone();
        input
            .get_ast_root(compiler)
            .add_child_to_back(compiler, var_node);
        compiler.report_change_to_enclosing_scope(var_node);
    }
}

impl CompilerPass for RescopeGlobalSymbols {
    // port: RescopeGlobalSymbols#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Make the name of the globalSymbolNamespace an extern.
        if self.add_extern {
            self.add_extern_for_global_symbol_namespace(compiler);
        }

        // Rewrite all references to global symbols to properties of a single symbol:

        // Turn global named function statements into var assignments.
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractShallowStatementCallback::new(
                RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback::new(),
            ),
        );

        // Find global names that are used in more than one chunk. Those that
        // are have to be rewritten.
        let mut find_cross_chunk_names = FindCrossChunkNamesCallback::new(
            self.optimize_local_access != OptimizeLocalAccess::DISABLED,
        );

        // And find names that may refer to functions that reference this.
        let mut find_names_referencing_this = FindNamesReferencingThis::new();

        CombinedCompilerPass::traverse(
            compiler,
            root,
            vec![
                &mut find_cross_chunk_names,
                &mut find_names_referencing_this,
            ],
        );

        let symbol_info = SymbolInformation::new(
            &find_cross_chunk_names.cross_chunk_names,
            &find_cross_chunk_names.cross_chunk_names_with_write_from_other_chunk,
            &find_cross_chunk_names.global_names_with_read_in_defining_chunk,
            &find_cross_chunk_names.global_names_with_inner_scope_write_in_defining_chunk,
            &find_names_referencing_this.maybe_references_this,
        );

        // Rewrite all references to be property accesses of the single symbol.
        let mut rewrite_scope = RescopeGlobalSymbolsRewriteCallback::new(
            compiler,
            self.global_symbol_namespace.clone(),
            self.assume_cross_chunk_names,
            self.optimize_local_access,
            externs,
            symbol_info,
        );
        NodeTraversal::traverse(compiler, root, &mut rewrite_scope);

        // Remove the var from statements in global scope if the declared names have been
        // rewritten in the previous pass.
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractShallowStatementCallback::new(RemoveGlobalVarCallback::new()),
        );

        rewrite_scope.add_declarations(compiler);
    }
}

/// Rewrites global function and class declarations to var statements + assignment. Ignores
/// non-global function and class declarations.
struct RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback;

impl RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback {
    // port: RescopeGlobalSymbols.RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback#RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback
    fn new() -> Self {
        Self
    }
}

impl AbstractPostOrderCallbackInterface
    for RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback
{
    // port: RescopeGlobalSymbols.RewriteGlobalClassFunctionDeclarationsToVarAssignmentsCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // Ignore block scopes within the global scope, as class and function declarations are
        // block-scoped.
        // Note that we should never find block-scoped function declarations if outputting ES5
        // code. Es6RewriteBlockScopedFunctionDeclaration will have rewritten them.
        if !t.in_global_scope() {
            return;
        }
        // Ignore everything that's not a function or class declaration.
        if !NodeUtil::is_function_declaration(t, n) && !NodeUtil::is_class_declaration(t, n) {
            return;
        }
        let parent = parent.unwrap();
        let name_node = NodeUtil::get_name_node(t, n).unwrap();
        let name = name_node.get_string(t);
        // Remove the class or function name. Anonymous classes have an EMPTY node, while
        // anonymous functions have a NAME node with an empty string.
        if n.is_class(t) {
            let empty = IR::empty(t).srcref(t, name_node);
            name_node.replace_with(t, empty);
        } else {
            name_node.set_string(t, "");
            t.get_compiler().report_change_to_enclosing_scope(name_node);
        }
        let prev = n.get_previous(t);
        n.detach(t);
        let var = NodeUtil::new_var_node(t, name, Some(n));
        match prev {
            None => parent.add_child_to_front(t, var),
            Some(prev) => var.insert_after(t, prev),
        }
        t.get_compiler().report_change_to_enclosing_scope(parent);
    }
}

/// Find all global names that are used in more than one chunk. The following compiler
/// transformations can ignore the globals that are not.
struct FindCrossChunkNamesCallback {
    track_local_access_sets: bool,
    cross_chunk_names: IndexSet<JsString>,

    /// Global cross-chunk identifiers that are written to from a chunk other than the defining
    /// one. Only populated if trackLocalAccessSets is true.
    cross_chunk_names_with_write_from_other_chunk: IndexSet<JsString>,

    /// Global identifiers that are read in the chunk where they are defined. Only populated if
    /// trackLocalAccessSets is true.
    global_names_with_read_in_defining_chunk: IndexSet<JsString>,

    /// Global identifiers that are written to from a nested scope (i.e. not the global scope) in
    /// their defining chunk. Only populated if trackLocalAccessSets is true.
    global_names_with_inner_scope_write_in_defining_chunk: IndexSet<JsString>,
}

impl FindCrossChunkNamesCallback {
    // port: RescopeGlobalSymbols.FindCrossChunkNamesCallback#FindCrossChunkNamesCallback
    fn new(track_local_access_sets: bool) -> Self {
        Self {
            track_local_access_sets,
            cross_chunk_names: IndexSet::<_>::default(),
            cross_chunk_names_with_write_from_other_chunk: IndexSet::<_>::default(),
            global_names_with_read_in_defining_chunk: IndexSet::<_>::default(),
            global_names_with_inner_scope_write_in_defining_chunk: IndexSet::<_>::default(),
        }
    }
}

impl Callback for FindCrossChunkNamesCallback {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RescopeGlobalSymbols.FindCrossChunkNamesCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) {
            let name = n.get_string(t);
            if name.is_empty()
                || (!self.track_local_access_sets && self.cross_chunk_names.contains(&name))
            {
                return;
            }
            let s = t.get_scope();
            let v = s.get_var(t.get_compiler(), name.clone());
            let Some(v) = v else {
                return;
            };
            if !v.is_global(t.get_compiler()) {
                return;
            }

            let input = v.get_input(t.get_compiler());

            if self.track_local_access_sets
                && !t.in_global_scope()
                && (input.is_none() || input.as_ref().unwrap().get_chunk() == t.get_chunk())
                && NodeUtil::is_l_value(t, n)
                && (!NodeUtil::is_name_declaration(t, parent) || n.has_children(t))
            {
                self.global_names_with_inner_scope_write_in_defining_chunk
                    .insert(name.clone());
            }

            let Some(input) = input else {
                // We know nothing. Assume name is used across chunks.
                self.cross_chunk_names.insert(name);
                return;
            };
            // Compare the chunk where the variable is declared to the current
            // chunk. If they are different, the variable is used across chunks.
            let chunk = input.get_chunk();
            if chunk != t.get_chunk() {
                self.cross_chunk_names.insert(name.clone());

                // If tracking local access, track names with non-local writes separately. This
                // includes all L-value names except for declarations without children.
                if self.track_local_access_sets
                    && NodeUtil::is_l_value(t, n)
                    && (!NodeUtil::is_name_declaration(t, parent) || n.has_children(t))
                {
                    self.cross_chunk_names_with_write_from_other_chunk
                        .insert(name);
                }
            } else if self.track_local_access_sets && Some(n) != v.get_name_node(t.get_compiler()) {
                self.global_names_with_read_in_defining_chunk.insert(name);
            }
        }
    }
}

/// Builds the maybeReferencesThis set of names that may reference a function that references
/// this.
struct FindNamesReferencingThis {
    maybe_references_this: IndexSet<JsString>,
}

impl FindNamesReferencingThis {
    fn new() -> Self {
        Self {
            maybe_references_this: IndexSet::<_>::default(),
        }
    }
}

impl Callback for FindNamesReferencingThis {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RescopeGlobalSymbols.FindNamesReferencingThis#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) {
            let name = n.get_string(t);
            if name.is_empty() {
                return;
            }
            let parent = parent.unwrap();
            let mut value: Option<NodeId> = None;
            if parent.is_assign(t) && Some(n) == parent.get_first_child(t) {
                value = parent.get_last_child(t);
            } else if NodeUtil::is_name_declaration(t, Some(parent)) {
                value = n.get_first_child(t);
            } else if parent.is_function(t) {
                value = Some(parent);
            }
            if value.is_none() && !NodeUtil::is_lhs_by_destructuring(t, n) {
                // If n is assigned in a destructuring pattern, don't bother finding its value and
                // just assume it may reference this.
                return;
            }
            // We already added this symbol. Done after checks above because those
            // are comparatively cheap.
            if self.maybe_references_this.contains(&name) {
                return;
            }
            let s = t.get_scope();
            let v = s.get_var(t.get_compiler(), name.clone());
            let Some(v) = v else {
                return;
            };
            if !v.is_global(t.get_compiler()) {
                return;
            }
            // If anything but a function is assigned we assume that possibly
            // a function referencing this is being assigned. Otherwise we
            // check whether the function assigned is a) an arrow function, which has a
            // lexically-scoped this, or b) a non-arrow function that does not reference this.
            if value.is_none()
                || !value.unwrap().is_function(t)
                || NodeUtil::references_own_receiver(t, value.unwrap())
            {
                self.maybe_references_this.insert(name);
            }
        }
    }
}

/// Removes every occurrence of var/let/const that declares a global variable.
struct RemoveGlobalVarCallback;

impl RemoveGlobalVarCallback {
    // port: RescopeGlobalSymbols.RemoveGlobalVarCallback#RemoveGlobalVarCallback
    fn new() -> Self {
        Self
    }

    // port: RescopeGlobalSymbols.RemoveGlobalVarCallback#joinOnComma
    fn join_on_comma(compiler: &mut AbstractCompiler, commas: &[NodeId], source: NodeId) -> NodeId {
        let mut comma = commas[0];
        for &next in &commas[1..] {
            let next_comma = IR::comma(compiler, comma, next);
            next_comma.srcref_if_missing(compiler, source);
            comma = next_comma;
        }
        comma
    }
}

impl AbstractPostOrderCallbackInterface for RemoveGlobalVarCallback {
    // port: RescopeGlobalSymbols.RemoveGlobalVarCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if !NodeUtil::is_name_declaration(t, Some(n)) {
            return;
        }
        let parent = parent.unwrap();

        let mut commas: Vec<NodeId> = Vec::new();
        let mut interesting_children: Vec<NodeId> = Vec::new();
        // Filter out declarations without assignments.
        // As opposed to regular var nodes, there are always assignments
        // because the previous traversal in RescopeGlobalSymbolsRewriteCallback creates
        // them.
        let mut all_name_or_destructuring = true;
        let mut c = n.get_first_child(t);
        while let Some(cur) = c {
            if !cur.is_name(t) && !cur.is_destructuring_lhs(t) {
                all_name_or_destructuring = false;
            }
            if cur.is_assign(t) || NodeUtil::is_any_for(t, parent) {
                interesting_children.push(cur);
            }
            c = cur.get_next(t);
        }
        // If every child of a var declares a name, it must stay in place.
        // This is the case if none of the declared variables cross chunk
        // boundaries.
        if all_name_or_destructuring {
            return;
        }
        for c in interesting_children {
            if NodeUtil::is_any_for(t, parent) && parent.get_first_child(t) == Some(n) {
                let clone = c.clone_tree(t);
                commas.push(clone);
            } else {
                // Var statement outside of for-loop.
                let clone = c.clone_tree(t);
                let expr = IR::expr_result(t, clone).srcref(t, c);
                NodeUtil::mark_new_scopes_changed(t.get_compiler(), expr);
                expr.insert_before(t, n);
            }
        }
        if !commas.is_empty() {
            let comma = Self::join_on_comma(t.get_compiler(), &commas, n);
            comma.insert_before(t, n);
        }
        // Remove the var/const/let node.
        n.detach(t);
        NodeUtil::mark_functions_deleted(t.get_compiler(), n);
        t.get_compiler().report_change_to_enclosing_scope(parent);
    }
}
