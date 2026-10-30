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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 8ac03b6:
//   src/com/google/javascript/jscomp/RescopeGlobalSymbolsRewriteCallback.java.

// Identity keys are immutable even though the associated Java objects are mutable.
#![allow(clippy::mutable_key_type)]
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_options::OptimizeLocalAccess,
    js_chunk::JSChunk,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    node::{NodeId, Prop},
    token::Token,
};

/// Node traversal callback to rewrite global symbols as part of the RescopeGlobalSymbols pass.
pub struct RescopeGlobalSymbolsRewriteCallback {
    global_symbol_namespace: JsString,
    assume_cross_chunk_names: bool,
    optimize_local_access: OptimizeLocalAccess,
    extern_names: IndexSet<JsString>,
    symbol_info: SymbolInformation,

    pre_declarations: Vec<ChunkGlobal>,

    /// Map from chunks to the set of global symbols to be aliased directly for that chunk (e.g.
    /// `var {a} = _`). Only populated if optimizeLocalAccess is an ALL_CHUNKS option. These
    /// symbols are used in that chunk, to avoid declaring unnecessary aliases. They can only be
    /// reassigned during static execution of the defining chunk, and that defining chunk must be a
    /// dependency of the using chunk. If ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS is set, then
    /// symbols that should be wrapped and aliased will be tracked separately in
    /// wrappedCrossChunkNames.
    local_aliases_for_unwrapped_cross_chunk_names: IndexMap<JSChunk, IndexSet<JsString>>,

    /// Map from cross-chunk global symbols to the chunks that reference them. Only populated if
    /// optimizeLocalAccess.shouldWrapReassignableSymbols() is true. This is used to determine which
    /// chunks to define wrapper objects in and when a symbol on the global namespace can be safely
    /// deleted after its last local alias is set.
    chunks_using_cross_chunk_symbols: IndexMap<JsString, IndexSet<JSChunk>>,

    /// Global symbols that are reassignable or referenced in a chunk that does not depend on their
    /// defining chunk, and will be wrapped in an object. Only populated if
    /// optimizeLocalAccess.shouldWrapReassignableSymbols() is true.
    wrapped_cross_chunk_names: IndexSet<JsString>,
}

// Appended to variables names that conflict with globalSymbolNamespace.
const DISAMBIGUATION_SUFFIX: &str = "$";

// port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymbolInformation {
    cross_chunk_names: IndexSet<JsString>,
    cross_chunk_names_with_write_from_other_chunk: IndexSet<JsString>,
    cross_chunk_names_used_in_non_dependent_chunk: IndexSet<JsString>,
    global_names_with_read_in_defining_chunk: IndexSet<JsString>,
    global_names_with_inner_scope_write_in_defining_chunk: IndexSet<JsString>,
    maybe_references_this: IndexSet<JsString>,
}

impl SymbolInformation {
    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#SymbolInformation(Set, Set, Set, Set, Set, Set)
    pub fn new(
        cross_chunk_names: &IndexSet<JsString>,
        cross_chunk_names_with_write_from_other_chunk: &IndexSet<JsString>,
        cross_chunk_names_used_in_non_dependent_chunk: &IndexSet<JsString>,
        global_names_with_read_in_defining_chunk: &IndexSet<JsString>,
        global_names_with_inner_scope_write_in_defining_chunk: &IndexSet<JsString>,
        maybe_references_this: &IndexSet<JsString>,
    ) -> Self {
        Self {
            cross_chunk_names: cross_chunk_names.clone(),
            cross_chunk_names_with_write_from_other_chunk:
                cross_chunk_names_with_write_from_other_chunk.clone(),
            cross_chunk_names_used_in_non_dependent_chunk:
                cross_chunk_names_used_in_non_dependent_chunk.clone(),
            global_names_with_read_in_defining_chunk: global_names_with_read_in_defining_chunk
                .clone(),
            global_names_with_inner_scope_write_in_defining_chunk:
                global_names_with_inner_scope_write_in_defining_chunk.clone(),
            maybe_references_this: maybe_references_this.clone(),
        }
    }

    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#crossChunkNames
    pub fn cross_chunk_names(&self) -> &IndexSet<JsString> {
        &self.cross_chunk_names
    }

    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#crossChunkNamesWithWriteFromOtherChunk
    pub fn cross_chunk_names_with_write_from_other_chunk(&self) -> &IndexSet<JsString> {
        &self.cross_chunk_names_with_write_from_other_chunk
    }

    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#crossChunkNamesUsedInNonDependentChunk
    pub fn cross_chunk_names_used_in_non_dependent_chunk(&self) -> &IndexSet<JsString> {
        &self.cross_chunk_names_used_in_non_dependent_chunk
    }

    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#globalNamesWithReadInDefiningChunk
    pub fn global_names_with_read_in_defining_chunk(&self) -> &IndexSet<JsString> {
        &self.global_names_with_read_in_defining_chunk
    }

    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#globalNamesWithInnerScopeWriteInDefiningChunk
    pub fn global_names_with_inner_scope_write_in_defining_chunk(&self) -> &IndexSet<JsString> {
        &self.global_names_with_inner_scope_write_in_defining_chunk
    }

    // port: RescopeGlobalSymbolsRewriteCallback.SymbolInformation#maybeReferencesThis
    pub fn maybe_references_this(&self) -> &IndexSet<JsString> {
        &self.maybe_references_this
    }
}

impl RescopeGlobalSymbolsRewriteCallback {
    // port: RescopeGlobalSymbolsRewriteCallback#RescopeGlobalSymbolsRewriteCallback
    pub fn new(
        compiler: &mut AbstractCompiler,
        global_symbol_namespace: JsString,
        assume_cross_chunk_names: bool,
        optimize_local_access: OptimizeLocalAccess,
        externs: NodeId,
        symbol_info: SymbolInformation,
    ) -> Self {
        let extern_names = NodeUtil::collect_extern_variable_names(compiler, externs);
        let mut this = Self {
            global_symbol_namespace,
            assume_cross_chunk_names,
            optimize_local_access,
            extern_names,
            symbol_info,
            pre_declarations: Vec::new(),
            local_aliases_for_unwrapped_cross_chunk_names: IndexMap::<_, _>::default(),
            chunks_using_cross_chunk_symbols: IndexMap::<_, _>::default(),
            wrapped_cross_chunk_names: IndexSet::<_>::default(),
        };
        this.wrapped_cross_chunk_names = this.collect_wrapped_cross_chunk_names(compiler);
        this
    }

    // port: RescopeGlobalSymbolsRewriteCallback#isCrossChunkName
    fn is_cross_chunk_name(&self, compiler: &AbstractCompiler, name: &JsString) -> bool {
        self.assume_cross_chunk_names
            || self.symbol_info.cross_chunk_names().contains(name)
            || compiler
                .get_coding_convention()
                .is_exported(name, /* local= */ false)
    }

    // port: RescopeGlobalSymbolsRewriteCallback#isExternVar
    fn is_extern_var(&self, varname: &JsString, t: &mut NodeTraversal<'_>) -> bool {
        if varname.is_empty() {
            return false;
        }
        let scope = t.get_scope();
        let v = scope.get_var(t.get_compiler(), varname.clone());
        let compiler = t.get_compiler();
        match v {
            None => true,
            Some(v) => {
                v.is_extern(compiler)
                    || (v.get_scope(compiler).is_global(compiler)
                        && self.extern_names.contains(varname))
            }
        }
    }

    // port: RescopeGlobalSymbolsRewriteCallback#collectWrappedCrossChunkNames
    fn collect_wrapped_cross_chunk_names(&self, compiler: &AbstractCompiler) -> IndexSet<JsString> {
        if !self
            .optimize_local_access
            .should_wrap_reassignable_symbols()
        {
            return IndexSet::<_>::default();
        }

        let mut candidates: IndexSet<JsString> = IndexSet::<_>::default();
        candidates.extend(
            self.symbol_info
                .cross_chunk_names_with_write_from_other_chunk()
                .iter()
                .cloned(),
        );
        candidates.extend(
            self.symbol_info
                .cross_chunk_names_used_in_non_dependent_chunk()
                .iter()
                .cloned(),
        );
        for name in self
            .symbol_info
            .global_names_with_inner_scope_write_in_defining_chunk()
        {
            if self.is_cross_chunk_name(compiler, name) {
                candidates.insert(name.clone());
            }
        }
        // Java: Set#removeAll (LinkedHashSet keeps the order of the remaining names).
        candidates.retain(|name| !self.extern_names.contains(name));
        candidates
    }

    // port: RescopeGlobalSymbolsRewriteCallback#visitNameDeclaration
    fn visit_name_declaration(&mut self, t: &mut NodeTraversal<'_>, declaration: NodeId) {
        let mut all_lhs_nodes: Vec<NodeId> = Vec::new();
        NodeUtil::visit_lhs_nodes_in_node(t.get_compiler(), declaration, &mut |_, n| {
            all_lhs_nodes.push(n);
        });
        if all_lhs_nodes.is_empty() {
            return;
        }
        let mut has_important_name = false;
        let first_name = all_lhs_nodes[0].get_string(t);
        let scope = t.get_scope();
        let is_global_declaration = scope
            .get_var(t.get_compiler(), first_name)
            .expect("NullPointerException")
            .is_global(t.get_compiler());

        // Check if any names are in the externs or are global and cross chunk.
        for &lhs in &all_lhs_nodes {
            check_state!(
                lhs.is_name(t),
                "Unexpected lhs node %s, expected NAME",
                lhs.to_string(t)
            );
            let lhs_name = lhs.get_string(t);
            if (is_global_declaration && self.is_cross_chunk_name(t.get_compiler(), &lhs_name))
                || self.is_extern_var(&lhs_name, t)
            {
                has_important_name = true;
                break;
            }
        }

        if has_important_name {
            self.rewrite_name_declaration(t, declaration, &all_lhs_nodes, is_global_declaration);
        }
    }

    /// Returns whether a global variable declared in the current chunk remains a local variable in
    /// that chunk (rather than being replaced everywhere in the chunk by a property access like
    /// `_.a` or `a._`, or referring to an extern).
    ///
    /// A global variable stays local in its defining chunk if it is not an extern and either:
    ///
    /// - it is not a cross-chunk global variable (so it is only ever used within this chunk), or
    /// - `optimizeLocalAccess` is enabled and the cross-chunk variable is read in its defining
    ///   chunk, is not written to from any other chunk, and is not wrapped in a holder object.
    // port: RescopeGlobalSymbolsRewriteCallback#globalVarStaysLocalInDefiningChunk
    fn global_var_stays_local_in_defining_chunk(
        &self,
        name: &JsString,
        t: &mut NodeTraversal<'_>,
    ) -> bool {
        let scope = t.get_scope();
        let is_global = scope
            .get_var(t.get_compiler(), name.clone())
            .expect("NullPointerException")
            .is_global(t.get_compiler());
        check_state!(is_global, "Expected global variable: %s", name);
        if self.is_extern_var(name, t) {
            return false;
        }
        if !self.is_cross_chunk_name(t.get_compiler(), name) {
            return true;
        }
        if !self.optimize_local_access.should_optimize() {
            return false;
        }
        !self
            .symbol_info
            .cross_chunk_names_with_write_from_other_chunk()
            .contains(name)
            && !self.wrapped_cross_chunk_names.contains(name)
            && self
                .symbol_info
                .global_names_with_read_in_defining_chunk()
                .contains(name)
    }

    /// Partially rewrites a variable declaration that contains at least one cross-chunk global or
    /// extern variable.
    ///
    /// In the post-order traversal ([`Self::visit_name`]), all rescoped cross-chunk global
    /// references will become property accesses (e.g. `_.a`), which are invalid as the LHS of a
    /// variable declaration and therefore must be converted into `ASSIGN` nodes beforehand.
    ///
    /// - When `optimizeLocalAccess.shouldPreserveLocalVarDeclarations()` is true (outside of
    ///   `for`-loop headers), `NAME` declarations that remain local variables in their defining
    ///   chunk (both chunk-local variables and local-access-optimized cross-chunk variables) are
    ///   kept in place as `NAME` children on the `VAR` declaration so their initializers stay
    ///   attached to `var`. Only non-local `NAME` children with initializers and
    ///   `DESTRUCTURING_LHS` children are converted to `ASSIGN` nodes.
    /// - Otherwise (when `optimizeLocalAccess.shouldPreserveLocalVarDeclarations()` is false, or
    ///   in `for`-loop headers), all local variables in the statement are recorded in
    ///   `preDeclarations` to be hoisted as uninitialized `var` declarations at the top of the
    ///   chunk, and all initialized `NAME` children and `DESTRUCTURING_LHS` children are
    ///   converted to `ASSIGN` nodes.
    // port: RescopeGlobalSymbolsRewriteCallback#rewriteNameDeclaration
    fn rewrite_name_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        declaration: NodeId,
        all_lhs_nodes: &[NodeId],
        is_global_declaration: bool,
    ) {
        let input = t.get_input().cloned().expect("NullPointerException");

        if self
            .optimize_local_access
            .should_preserve_local_var_declarations()
            && !NodeUtil::is_any_for(t, declaration.get_parent(t).unwrap())
        {
            // If all variables in a global `let` or `const` declaration stay local (e.g.
            // `let a = 1;` becoming `var a = _.a = 1;`), RemoveGlobalVarCallback will leave the
            // declaration node in place, so convert its token to `VAR`. Uninitialized declarations
            // with no rescoped variables (e.g. `let a;`) are left untouched.
            if is_global_declaration
                && !declaration.is_var(t)
                && self.contains_rescoped_or_initialized_vars(t, declaration, is_global_declaration)
            {
                declaration.set_token(t, Token::VAR);
            }
            let mut child = declaration.get_first_child(t);
            while let Some(c) = child {
                let next = c.get_next(t);
                if c.is_name(t) {
                    let name = c.get_string(t);
                    if !self.global_var_stays_local_in_defining_chunk(&name, t) {
                        if c.has_children(t) {
                            let target = c.clone_node(t);
                            let value = c.remove_first_child(t).unwrap();
                            let assign = IR::assign(t, target, value);
                            c.replace_with(t, assign);
                            let info = declaration.get_jsdoc_info(t);
                            assign.set_jsdoc_info(t, info);
                        } else if self.is_extern_var(&name, t) {
                            // Detach uninitialized extern redeclarations (e.g.
                            // `var externVar, a = 1;`) so RemoveGlobalVarCallback does not treat
                            // the remaining NAME node as a local variable.
                            c.detach(t);
                        }
                    }
                } else if c.is_destructuring_lhs(t) {
                    // Destructuring declarations are converted to assignments, so any bound
                    // variables that stay local to the defining chunk must be pre-declared at the
                    // top of the chunk.
                    let mut lhs_nodes: Vec<NodeId> = Vec::new();
                    NodeUtil::visit_lhs_nodes_in_node(t.get_compiler(), c, &mut |_, lhs| {
                        lhs_nodes.push(lhs);
                    });
                    for lhs in lhs_nodes {
                        let name = lhs.get_string(t);
                        if self.global_var_stays_local_in_defining_chunk(&name, t) {
                            let root = input.get_ast_root(t.get_compiler());
                            let name_node = IR::name(t, name).srcref(t, lhs);
                            self.pre_declarations
                                .push(ChunkGlobal::new(root, name_node));
                        }
                    }
                    let target = c.remove_first_child(t).unwrap();
                    let value = c.remove_first_child(t).unwrap();
                    let assign = IR::assign(t, target, value);
                    c.replace_with(t, assign);
                    let info = declaration.get_jsdoc_info(t);
                    assign.set_jsdoc_info(t, info);
                }
                child = next;
            }
            t.get_compiler()
                .report_change_to_enclosing_scope(declaration);
            return;
        }

        // Add pre-declarations for all LHS variables that remain local to the defining chunk.
        // If we are optimizing local accesses (in a for-loop header), we only want to pre-declare
        // variables if the declaration will be converted to assignments/property accesses and
        // removed by RemoveGlobalVarCallback, to avoid redeclaration errors (e.g. "var a; let a;").
        if !self.optimize_local_access.should_optimize()
            || self.contains_rescoped_or_initialized_vars(t, declaration, is_global_declaration)
        {
            for &lhs in all_lhs_nodes {
                let name = lhs.get_string(t);
                if self.global_var_stays_local_in_defining_chunk(&name, t) {
                    let root = input.get_ast_root(t.get_compiler());
                    let name_node = IR::name(t, name).srcref(t, lhs);
                    self.pre_declarations
                        .push(ChunkGlobal::new(root, name_node));
                }
            }
        }

        // Convert all names with an rhs and all destructuring patterns to be assignments. e.g.
        //  VAR
        //    NAME foo
        //      NUMBER 3
        // becomes
        //  VAR
        //    ASSIGN
        //      NAME foo
        //      NUMBER 3
        let mut child = declaration.get_first_child(t);
        while let Some(c) = child {
            let next = c.get_next(t);
            if c.is_name(t) && c.has_children(t) {
                let target = c.clone_node(t);
                let value = c.remove_first_child(t).unwrap();
                let assign = IR::assign(t, target, value);
                c.replace_with(t, assign);
                let info = declaration.get_jsdoc_info(t);
                assign.set_jsdoc_info(t, info);
            } else if c.is_destructuring_lhs(t) {
                if c.has_one_child(t) {
                    check_state!(
                        NodeUtil::is_enhanced_for(t, declaration.get_parent(t).unwrap()),
                        "DESTRUCTURING_LHS should have two children: %s",
                        declaration.to_string_tree(t)
                    );
                    // remove the DESTRUCTURING_LHS but leave the actual destructuring pattern
                    let pattern = c.remove_first_child(t).unwrap();
                    c.replace_with(t, pattern);
                } else {
                    let target = c.remove_first_child(t).unwrap();
                    let value = c.remove_first_child(t).unwrap();
                    let assign = IR::assign(t, target, value);
                    c.replace_with(t, assign);
                    let info = declaration.get_jsdoc_info(t);
                    assign.set_jsdoc_info(t, info);
                }
            }
            child = next;
        }
        t.get_compiler()
            .report_change_to_enclosing_scope(declaration);
    }

    /// Determines whether a variable declaration statement contains any variables that will be
    /// rescoped (replaced with property accesses) or initialized (replaced with assignments or
    /// chained namespace initializers). Assumes that optimizeLocalAccess is true.
    ///
    /// In a `for`-loop header, if a declaration contains any such variables, the declaration
    /// keyword will be removed by `RemoveGlobalVarCallback` and any local variables in it must be
    /// pre-declared. Outside a `for`-loop header, global `let`/`const` declarations containing any
    /// such variables are converted to `var`.
    // port: RescopeGlobalSymbolsRewriteCallback#containsRescopedOrInitializedVars
    fn contains_rescoped_or_initialized_vars(
        &self,
        t: &NodeTraversal<'_>,
        declaration: NodeId,
        is_global_declaration: bool,
    ) -> bool {
        let mut child = declaration.get_first_child(t);
        while let Some(c) = child {
            if is_global_declaration
                && c.is_name(t)
                && (self
                    .symbol_info
                    .cross_chunk_names_with_write_from_other_chunk()
                    .contains(&c.get_string(t))
                    || self.wrapped_cross_chunk_names.contains(&c.get_string(t)))
            {
                return true;
            }
            if c.has_children(t) || c.is_destructuring_lhs(t) {
                return true;
            }
            child = c.get_next(t);
        }
        false
    }

    // port: RescopeGlobalSymbolsRewriteCallback#visitName
    fn visit_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        let name = n.get_string(t);

        // Ignore anonymous functions
        if parent.is_function(t) && name.is_empty() {
            return;
        }

        if self.is_extern_var(&name, t) {
            return;
        }

        // When the globalSymbolNamespace is used as a local variable name
        // add suffix to avoid shadowing the namespace. Also add a suffix
        // if a name starts with the name of the globalSymbolNamespace and
        // the suffix.
        let scope = t.get_scope();
        let var = scope
            .get_var(t.get_compiler(), name.clone())
            .expect("NullPointerException");
        let suffix = JsString::from(DISAMBIGUATION_SUFFIX);
        if !var.is_global(t.get_compiler())
            && (name == self.global_symbol_namespace
                || name.starts_with(self.global_symbol_namespace.concat(&suffix)))
        {
            n.set_string(t, name.concat(&suffix));
            t.get_compiler().report_change_to_enclosing_scope(n);
        }

        // We only care about global vars.
        if !(var.is_global(t.get_compiler()) && self.is_cross_chunk_name(t.get_compiler(), &name)) {
            return;
        }

        let current_chunk = t.get_chunk();
        if self
            .optimize_local_access
            .should_wrap_reassignable_symbols()
        {
            self.chunks_using_cross_chunk_symbols
                .entry(name.clone())
                .or_default()
                .insert(current_chunk.clone().expect("NullPointerException"));
        }

        if self.wrapped_cross_chunk_names.contains(&name) {
            // Replace the symbol with an access to the wrapper.
            let target = IR::name(t, name.clone());
            let replacement = IR::getprop(t, target, "_");
            self.replace_symbol(t.get_compiler(), n, &name, replacement);
            return;
        }

        if self.optimize_local_access.should_optimize()
            && !self
                .symbol_info
                .cross_chunk_names_with_write_from_other_chunk()
                .contains(&name)
        {
            let defining_chunk = var
                .get_input(t.get_compiler())
                .expect("NullPointerException")
                .get_chunk();
            if current_chunk == defining_chunk {
                if self
                    .symbol_info
                    .global_names_with_read_in_defining_chunk()
                    .contains(&name)
                {
                    // If the cross-chunk variable is defined in this chunk, not re-assigned in any
                    // other chunk, and not rewritten to wrapper access, then we skip the
                    // replacement and keep the local name.
                    if NodeUtil::is_l_value(t, n)
                        && (!NodeUtil::is_name_declaration(t, Some(parent)) || n.has_children(t))
                    {
                        // Any assignment needs to be also set on the global namespace symbol.
                        self.add_global_namespace_alias(t.get_compiler(), n, &name);
                    }
                    // Early return to skip replacing the symbol.
                    return;
                }
                // If the variable is only initialized in its defining chunk and never read there,
                // replace it directly with an access on the global namespace symbol instead of
                // keeping an unused local variable.
                let target = IR::name(t, self.global_symbol_namespace.clone());
                let replacement = IR::getprop(t, target, name.clone());
                self.replace_symbol(t.get_compiler(), n, &name, replacement);
                return;
            } else if self.optimize_local_access.should_optimize_all_chunks() {
                // Variables defined in a different chunk are still safe for local aliasing if all
                // of the following conditions are met.
                // 1. The variable is not written to in any other chunk than in the defining chunk.
                // 2. The variable is not written to in an inner scope in the defining chunk, so
                //     all assignments are guaranteed to happen during the initial execution of the
                //     chunk.
                // 3. The current chunk depends on the defining chunk, so all writes are
                //     guaranteed to have occurred already when the current chunk is loaded.
                let current = current_chunk.clone().expect("NullPointerException");
                let defining = defining_chunk.expect("NullPointerException");
                if !self
                    .symbol_info
                    .global_names_with_inner_scope_write_in_defining_chunk()
                    .contains(&name)
                    && t.get_compiler()
                        .get_chunk_graph()
                        .expect("NullPointerException")
                        .depends_on(&current, &defining)
                {
                    self.local_aliases_for_unwrapped_cross_chunk_names
                        .entry(current)
                        .or_default()
                        .insert(name);
                    // Early return to skip replacing the symbol.
                    return;
                }
            }
        }

        if self
            .optimize_local_access
            .should_wrap_reassignable_symbols()
        {
            // Java: throw new IllegalStateException(..)
            panic!(
                "java.lang.IllegalStateException: Unexpected unaliased cross-chunk symbol under \
                 ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS: {name}"
            );
        }

        // Otherwise replace the symbol with an access on the global namespace symbol.
        let target = IR::name(t, self.global_symbol_namespace.clone());
        let replacement = IR::getprop(t, target, name.clone());
        self.replace_symbol(t.get_compiler(), n, &name, replacement);
    }

    /// Replaces a symbol with an access to the replacement.
    // port: RescopeGlobalSymbolsRewriteCallback#replaceSymbol
    fn replace_symbol(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        name: &JsString,
        replacement: NodeId,
    ) {
        let parent = node.get_parent(compiler).unwrap();
        replacement.srcref_tree(compiler, node);
        node.replace_with(compiler, replacement);
        compiler.report_change_to_enclosing_scope(replacement);
        if parent.is_call(compiler) && !self.symbol_info.maybe_references_this().contains(name) {
            // Do not write calls like this: (0, _a)() but rather as _.a(). The
            // this inside the function will be wrong, but it doesn't matter
            // because the this is never read.
            parent.put_boolean_prop(compiler, Prop::FREE_CALL, false);
        }
        compiler.report_change_to_enclosing_scope(parent);
    }

    /// Aliases the local variable's value to the global namespace, so that it can be accessed from
    /// other chunks.
    ///
    /// For simple assignments and variable declarations with initializers, this chains the
    /// assignment to avoid an extra statement (e.g. converts `a = x` to `a = _.a = x` and
    /// `var a = x` to `var a = _.a = x`). For other writes (e.g. compound assignments,
    /// destructuring, increments), it inserts a separate assignment statement after the current
    /// one (e.g. appends `_.a = a;`).
    // port: RescopeGlobalSymbolsRewriteCallback#addGlobalNamespaceAlias
    fn add_global_namespace_alias(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        name: &JsString,
    ) {
        let stmt = NodeUtil::get_enclosing_statement(compiler, n);
        let Some(stmt) = stmt else {
            return;
        };
        let parent = n.get_parent(compiler).unwrap();
        if parent.is_assign(compiler) && parent.get_first_child(compiler) == Some(n) {
            // Optimized path for simple assignments (e.g. `a = x`).
            // We chain the assignment to the global namespace to reduce output size:
            // `a = _.a = x` instead of `a = x; _.a = a;`.
            let rhs = parent.get_second_child(compiler).unwrap();
            let ns = IR::name(compiler, self.global_symbol_namespace.clone());
            let getprop = IR::getprop(compiler, ns, name.clone());
            let detached = rhs.detach(compiler);
            let global_namespace_assign = IR::assign(compiler, getprop, detached);
            global_namespace_assign.srcref_tree(compiler, n);
            parent.add_child_to_back(compiler, global_namespace_assign);
            compiler.report_change_to_enclosing_scope(parent);
        } else if NodeUtil::is_name_declaration(compiler, Some(parent)) {
            // Optimized path for variable declarations with initializers (e.g. `var a = x`).
            // We chain the assignment to the global namespace: `var a = _.a = x`.
            let rhs = n.remove_first_child(compiler).unwrap();
            let ns = IR::name(compiler, self.global_symbol_namespace.clone());
            let getprop = IR::getprop(compiler, ns, name.clone());
            let global_namespace_assign = IR::assign(compiler, getprop, rhs);
            global_namespace_assign.srcref_tree(compiler, n);
            n.add_child_to_back(compiler, global_namespace_assign);
            compiler.report_change_to_enclosing_scope(n);
        } else {
            // Fallback path for other writes (e.g. `a++`, `a += x`, destructuring).
            // In these cases, we cannot easily chain the assignment without changing semantics
            // or making the code overly complex. Instead, we append a separate assignment
            // statement (`_.a = a;`).
            let ns = IR::name(compiler, self.global_symbol_namespace.clone());
            let getprop = IR::getprop(compiler, ns, name.clone());
            let value = IR::name(compiler, name.clone());
            let global_namespace_assign = IR::assign(compiler, getprop, value);
            let alias_statement = IR::expr_result(compiler, global_namespace_assign);
            alias_statement.srcref_tree(compiler, n);
            alias_statement.insert_after(compiler, stmt);
            let stmt_parent = stmt.get_parent(compiler).unwrap();
            compiler.report_change_to_enclosing_scope(stmt_parent);
        }
    }

    /// Adds back declarations for variables that do not cross chunk boundaries, and declares local
    /// aliases and wrappers. Must be called after RemoveGlobalVarCallback.
    // port: RescopeGlobalSymbolsRewriteCallback#addDeclarations
    pub fn add_declarations(&mut self, compiler: &mut AbstractCompiler) {
        self.declare_chunk_globals(compiler);
        if self.optimize_local_access.should_optimize_all_chunks() {
            self.declare_local_aliases_and_wrappers(compiler);
        }
    }

    /// Adds back declarations for variables that do not cross chunk boundaries. Must be called
    /// after RemoveGlobalVarCallback.
    // port: RescopeGlobalSymbolsRewriteCallback#declareChunkGlobals
    fn declare_chunk_globals(&mut self, compiler: &mut AbstractCompiler) {
        for global in &self.pre_declarations {
            if global.root.has_children(compiler)
                && global
                    .root
                    .get_first_child(compiler)
                    .unwrap()
                    .is_var(compiler)
            {
                global
                    .root
                    .get_first_child(compiler)
                    .unwrap()
                    .add_child_to_back(compiler, global.name);
            } else {
                let var = IR::var(compiler, global.name).srcref(compiler, global.name);
                global.root.add_child_to_front(compiler, var);
            }
            compiler.report_change_to_enclosing_scope(global.root);
        }
    }

    /// Declares local aliases (e.g. `var {a} = _;` or `var a = _.a;`) and wrapper objects (e.g.
    /// `_.a = {};` or `var a = _.a = {};`) at the beginning of chunks for cross-chunk variables
    /// that can be safely accessed locally.
    // port: RescopeGlobalSymbolsRewriteCallback#declareLocalAliasesAndWrappers
    fn declare_local_aliases_and_wrappers(&mut self, compiler: &mut AbstractCompiler) {
        let chunk_graph = compiler.get_chunk_graph().expect("NullPointerException");
        let chunks = chunk_graph.get_all_chunks().to_vec();
        let default_root_chunk = chunks[0].clone();

        // Compute the chunk in which to define the wrapper for each wrapped symbol.
        let mut wrapper_assignment_chunks: IndexMap<JsString, Option<JSChunk>> =
            IndexMap::<_, _>::default();
        if !self.wrapped_cross_chunk_names.is_empty() {
            for name in &self.wrapped_cross_chunk_names {
                let using_chunks: Vec<JSChunk> = self
                    .chunks_using_cross_chunk_symbols
                    .get(name)
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default();
                let wrapper_assignment_chunk = if using_chunks.is_empty() {
                    Some(default_root_chunk.clone())
                } else {
                    chunk_graph.get_deepest_common_dependency_inclusive_collection(&using_chunks)
                };
                wrapper_assignment_chunks.insert(name.clone(), wrapper_assignment_chunk);
            }
        }

        // Prepend local aliases and wrapper assignments to each chunk.
        let cleanup_helper_name = self.get_cleanup_helper_property_name();
        let mut needs_cleanup_helper = false;
        for chunk in chunks {
            if self.declare_local_aliases_and_wrappers_for_chunk(
                compiler,
                &chunk,
                &wrapper_assignment_chunks,
                &cleanup_helper_name,
            ) {
                needs_cleanup_helper = true;
            }
        }
        if needs_cleanup_helper {
            self.declare_cleanup_helper(compiler, &cleanup_helper_name);
        }
    }

    // port: RescopeGlobalSymbolsRewriteCallback#getCleanupHelperPropertyName
    fn get_cleanup_helper_property_name(&self) -> JsString {
        let mut name = JsString::from("_");
        let suffix = JsString::from(DISAMBIGUATION_SUFFIX);
        while self.chunks_using_cross_chunk_symbols.contains_key(&name) {
            name = name.concat(&suffix);
        }
        name
    }

    /// Declares a helper function on the global namespace object in the root chunk to delete
    /// symbol entries by comma-separated name list, e.g.:
    ///
    /// ```text
    /// _._ = function(a) { for (let b of a.split(',')) { delete _[b]; } };
    /// ```
    // port: RescopeGlobalSymbolsRewriteCallback#declareCleanupHelper
    fn declare_cleanup_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        cleanup_helper_name: &JsString,
    ) {
        let script = compiler.get_node_for_code_insertion(None);
        let param_name = if self.global_symbol_namespace == "a" {
            "b"
        } else {
            "a"
        };
        let loop_var = if param_name != "b" && self.global_symbol_namespace != "b" {
            "b"
        } else {
            "c"
        };
        let param = IR::name(compiler, param_name);
        let split = IR::getprop(compiler, param, "split");
        let comma = IR::string(compiler, ",");
        let split_call = IR::call(compiler, split, &[comma]);
        let ns = IR::name(compiler, self.global_symbol_namespace.clone());
        let loop_name = IR::name(compiler, loop_var);
        let getelem = IR::getelem(compiler, ns, loop_name);
        let delprop = IR::delprop(compiler, getelem);
        let delete_stmt = IR::expr_result(compiler, delprop);
        let loop_decl_name = IR::name(compiler, loop_var);
        let loop_decl = IR::r#let(compiler, loop_decl_name);
        let loop_body = IR::block_with_child(compiler, delete_stmt);
        let for_of =
            compiler.new_node_with_children3(Token::FOR_OF, loop_decl, split_call, loop_body);
        let fn_name = IR::name(compiler, "");
        let fn_param = IR::name(compiler, param_name);
        let param_list = IR::param_list(compiler, &[fn_param]);
        let fn_body = IR::block_with_child(compiler, for_of);
        let helper_fn = IR::function(compiler, fn_name, param_list, fn_body);
        let helper_ns = IR::name(compiler, self.global_symbol_namespace.clone());
        let helper_target = IR::getprop(compiler, helper_ns, cleanup_helper_name.clone());
        let helper_assign = IR::assign(compiler, helper_target, helper_fn);
        let helper_decl = IR::expr_result(compiler, helper_assign);
        let insertion_point = script.get_first_child(compiler);
        Self::add_declaration(compiler, script, insertion_point, helper_decl);
        NodeUtil::mark_new_scopes_changed(compiler, helper_fn);
        NodeUtil::add_feature_to_script(compiler, script, Feature::FOR_OF);
        NodeUtil::add_feature_to_script(compiler, script, Feature::LET_DECLARATIONS);
        compiler.report_change_to_enclosing_scope(script);
    }

    /// Declares all local aliases and wrapper assignments that need to be prepended to the given
    /// chunk. Returns true if the chunk emitted a call to the cleanup helper.
    // port: RescopeGlobalSymbolsRewriteCallback#declareLocalAliasesAndWrappersForChunk
    fn declare_local_aliases_and_wrappers_for_chunk(
        &mut self,
        compiler: &mut AbstractCompiler,
        chunk: &JSChunk,
        wrapper_assignment_chunks: &IndexMap<JsString, Option<JSChunk>>,
        cleanup_helper_name: &JsString,
    ) -> bool {
        let inputs = chunk.get_inputs();
        if inputs.is_empty() {
            return false;
        }
        let script = inputs[0].get_ast_root(compiler);
        let insertion_point = script.get_first_child(compiler);
        let mut changed = false;
        let mut used_cleanup_helper = false;

        let mut wrapped_reassignable_local_aliases: IndexSet<JsString> = IndexSet::<_>::default();
        if self
            .optimize_local_access
            .should_wrap_reassignable_symbols()
        {
            wrapped_reassignable_local_aliases = IndexSet::<_>::default();

            for name in &self.wrapped_cross_chunk_names {
                let assignment_chunk = wrapper_assignment_chunks.get(name).cloned().flatten();
                let using_chunks = self.chunks_using_cross_chunk_symbols.get(name);
                if Some(chunk) == assignment_chunk.as_ref() {
                    // Define the wrapper object for the wrapped symbol in this chunk.
                    if using_chunks.is_some_and(|u| u.contains(chunk)) {
                        // The symbol is used in this chunk, so we also declare a local alias:
                        // `var name = _.name = {};`.
                        let lhs = IR::name(compiler, name.clone());
                        let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                        let getprop = IR::getprop(compiler, ns, name.clone());
                        let objectlit = IR::objectlit(compiler, &[]);
                        let assign = IR::assign(compiler, getprop, objectlit);
                        let decl = IR::var_with_value(compiler, lhs, assign);
                        Self::add_declaration(compiler, script, insertion_point, decl);
                    } else {
                        // The symbol is not used in this chunk, so we only define the wrapper
                        // object: `_.name = {};`.
                        let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                        let getprop = IR::getprop(compiler, ns, name.clone());
                        let objectlit = IR::objectlit(compiler, &[]);
                        let assign = IR::assign(compiler, getprop, objectlit);
                        let decl = IR::expr_result(compiler, assign);
                        Self::add_declaration(compiler, script, insertion_point, decl);
                    }
                    changed = true;
                } else if using_chunks.is_some_and(|u| u.contains(chunk)) {
                    // If the wrapper is defined in a different chunk, but the symbol is used in
                    // this chunk, then only declare a local alias.
                    wrapped_reassignable_local_aliases.insert(name.clone());
                }
            }
        }

        // Combine all local aliases into a single destructuring assignment
        // (e.g. `var {a, b, c} = _;`) if output is ES2015+, otherwise declare
        // individual local aliases (e.g. `var a = _.a;`).
        let all_local_aliases: Vec<JsString> = self
            .local_aliases_for_unwrapped_cross_chunk_names
            .get(chunk)
            .into_iter()
            .flatten()
            .cloned()
            .chain(wrapped_reassignable_local_aliases.iter().cloned())
            .collect();
        if !all_local_aliases.is_empty() {
            let mut aliases_to_keep: Vec<JsString> = Vec::new();
            let mut aliases_to_delete: Vec<JsString> = Vec::new();
            for name in all_local_aliases {
                if self
                    .optimize_local_access
                    .should_delete_globals_on_last_access()
                    && self.can_delete_from_global_namespace(compiler, chunk, &name)
                {
                    aliases_to_delete.push(name);
                } else {
                    aliases_to_keep.push(name);
                }
            }
            // Place aliasesToDelete contiguously at the end of the destructuring assignment so
            // that the comma-separated symbol list in `_._('x,y')` is an exact substring
            // immediately following `var {..., x, y} = _;`, minimizing compressed binary size.
            let local_aliases: Vec<JsString> = aliases_to_keep
                .iter()
                .chain(aliases_to_delete.iter())
                .cloned()
                .collect();
            let output_features = compiler.get_options().get_output_feature_set();
            if output_features.contains(Feature::OBJECT_DESTRUCTURING)
                && output_features.contains(Feature::FOR_OF)
                && output_features.contains(Feature::LET_DECLARATIONS)
            {
                let object_pattern = IR::object_pattern(compiler, &[]);
                for name in &local_aliases {
                    let value = IR::name(compiler, name.clone());
                    let string_key = IR::string_key_with_value(compiler, name.clone(), value);
                    string_key.set_shorthand_property(compiler, true);
                    object_pattern.add_child_to_back(compiler, string_key);
                }
                let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                let decl = IR::var_with_value(compiler, object_pattern, ns);
                Self::add_declaration(compiler, script, insertion_point, decl);
                NodeUtil::add_feature_to_script(compiler, script, Feature::OBJECT_DESTRUCTURING);
                if !aliases_to_delete.is_empty() {
                    let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                    let callee = IR::getprop(compiler, ns, cleanup_helper_name.clone());
                    let mut joined = JsString::from("");
                    let comma = JsString::from(",");
                    for (i, name) in aliases_to_delete.iter().enumerate() {
                        if i > 0 {
                            joined = joined.concat(&comma);
                        }
                        joined = joined.concat(name);
                    }
                    let names = IR::string(compiler, joined);
                    let call = IR::call(compiler, callee, &[names]);
                    let cleanup_call = IR::expr_result(compiler, call);
                    Self::add_declaration(compiler, script, insertion_point, cleanup_call);
                    used_cleanup_helper = true;
                }
            } else {
                for name in &aliases_to_keep {
                    let lhs = IR::name(compiler, name.clone());
                    let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                    let getprop = IR::getprop(compiler, ns, name.clone());
                    let decl = IR::var_with_value(compiler, lhs, getprop);
                    Self::add_declaration(compiler, script, insertion_point, decl);
                }
                for name in &aliases_to_delete {
                    let lhs = IR::name(compiler, name.clone());
                    let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                    let getprop = IR::getprop(compiler, ns, name.clone());
                    let decl = IR::var_with_value(compiler, lhs, getprop);
                    Self::add_declaration(compiler, script, insertion_point, decl);
                    let ns = IR::name(compiler, self.global_symbol_namespace.clone());
                    let getprop = IR::getprop(compiler, ns, name.clone());
                    let delprop = IR::delprop(compiler, getprop);
                    let delete_stmt = IR::expr_result(compiler, delprop);
                    Self::add_declaration(compiler, script, insertion_point, delete_stmt);
                }
            }
            changed = true;
        }

        if changed {
            compiler.report_change_to_enclosing_scope(script);
        }
        used_cleanup_helper
    }

    /// Returns true if `name` can be safely deleted from the global symbol namespace in
    /// `current_chunk` right after its local alias is initialized, i.e. when all other chunks that
    /// reference `name` are transitive dependencies (parent chunks) of `current_chunk`.
    // port: RescopeGlobalSymbolsRewriteCallback#canDeleteFromGlobalNamespace
    fn can_delete_from_global_namespace(
        &self,
        compiler: &AbstractCompiler,
        current_chunk: &JSChunk,
        name: &JsString,
    ) -> bool {
        if compiler
            .get_coding_convention()
            .is_exported(name, /* local= */ false)
        {
            return false;
        }
        let Some(using_chunks) = self.chunks_using_cross_chunk_symbols.get(name) else {
            return false;
        };
        if !using_chunks.contains(current_chunk) {
            return false;
        }
        let chunk_graph = compiler.get_chunk_graph().expect("NullPointerException");
        for other_chunk in using_chunks {
            if other_chunk != current_chunk && !chunk_graph.depends_on(current_chunk, other_chunk) {
                return false;
            }
        }
        true
    }

    // port: RescopeGlobalSymbolsRewriteCallback#addDeclaration
    fn add_declaration(
        compiler: &mut AbstractCompiler,
        script: NodeId,
        insertion_point: Option<NodeId>,
        decl: NodeId,
    ) {
        decl.srcref_tree(compiler, script);
        match insertion_point {
            None => script.add_child_to_back(compiler, decl),
            Some(insertion_point) => decl.insert_before(compiler, insertion_point),
        }
    }
}

impl Callback for RescopeGlobalSymbolsRewriteCallback {
    // port: RescopeGlobalSymbolsRewriteCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if NodeUtil::is_name_declaration(t, Some(n)) {
            self.visit_name_declaration(t, n);
        }
        true
    }

    // port: RescopeGlobalSymbolsRewriteCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) {
            self.visit_name(t, n, parent.unwrap());
        }
    }
}

/// Variable that doesn't cross chunk boundaries.
// port: RescopeGlobalSymbolsRewriteCallback.ChunkGlobal
struct ChunkGlobal {
    root: NodeId,
    name: NodeId,
}

impl ChunkGlobal {
    // port: RescopeGlobalSymbolsRewriteCallback.ChunkGlobal#ChunkGlobal
    fn new(root: NodeId, name: NodeId) -> Self {
        Self { root, name }
    }
}
