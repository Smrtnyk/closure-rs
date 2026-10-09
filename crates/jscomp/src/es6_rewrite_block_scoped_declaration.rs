/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6RewriteBlockScopedDeclaration.java.

//! Port of `Es6RewriteBlockScopedDeclaration.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    colors::standard_colors,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    scope::ScopeId,
    this_and_arguments_reference_updater::{
        ThisAndArgumentsContext, ThisAndArgumentsReferenceUpdater,
    },
    transpilation_passes::TranspilationPasses,
    var::VarId,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jsdoc_info::Builder as JSDocInfoBuilder,
    node::{Ast, NodeId, Prop},
    token::Token,
};

// port: Es6RewriteBlockScopedDeclaration#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM
        .with(Feature::LET_DECLARATIONS)
        .with(Feature::CONST_DECLARATIONS)
}

const LOOP_PARAM_NAME_PREFIX: &str = "$jscomp$loop_param$";

/// Rewrite "let"s and "const"s as "var"s. Rename block-scoped declarations and their references
/// when necessary.
///
/// Note that this must run after Es6RewriteDestructuring, since it does not process destructuring
/// let/const declarations at all.
///
/// Java keeps the compiler and `compiler.getUniqueIdSupplier()`; here both are reached through the
/// compiler argument or the traversal (DESIGN §6).
pub struct Es6RewriteBlockScopedDeclaration {
    ast_factory: AstFactory,
    let_consts: IndexSet<NodeId>,
}

impl Es6RewriteBlockScopedDeclaration {
    // port: Es6RewriteBlockScopedDeclaration#Es6RewriteBlockScopedDeclaration
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            let_consts: IndexSet::<_>::default(),
        }
    }

    /// Normalizes block-scoped let/const declarations with no initializer in the loop body to be
    /// initialized to undefined.
    // port: Es6RewriteBlockScopedDeclaration#visitBlockScopedNameDeclaration
    fn visit_block_scoped_name_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        decl: NodeId,
        name_node: NodeId,
    ) {
        let parent = decl.get_parent(compiler);
        // Normalize "let x;" to "let x = undefined;" if in a loop, since we later convert x
        // to be $jscomp$loop$0.x and want to reset the property to undefined every loop iteration.
        if !name_node.has_children(compiler)
            && parent.is_none_or(|parent| !parent.is_for_in(compiler))
            && Self::in_loop(compiler, decl)
        {
            let undefined = self
                .ast_factory
                .create_undefined_value(compiler)
                .srcref_tree(compiler, name_node);
            name_node.add_child_to_front(compiler, undefined);
            compiler.report_change_to_enclosing_scope(undefined);
        }
    }

    /// Whether n is inside a loop. If n is inside a function which is inside a loop, we do not
    /// consider it to be inside a loop.
    // port: Es6RewriteBlockScopedDeclaration#inLoop
    fn in_loop(ast: &Ast, n: NodeId) -> bool {
        let enclosing_node = NodeUtil::get_enclosing_node(ast, n, &Self::is_loop_or_function);
        enclosing_node.is_some_and(|enclosing_node| !enclosing_node.is_function(ast))
    }

    // port: Es6RewriteBlockScopedDeclaration#isLoopOrFunction
    fn is_loop_or_function(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast) || NodeUtil::is_loop_structure(ast, n)
    }

    // port: Es6RewriteBlockScopedDeclaration#extractInlineJSDoc
    fn extract_inline_jsdoc(
        ast: &mut Ast,
        src_declaration: NodeId,
        src_name: NodeId,
        dest_declaration: NodeId,
    ) {
        let mut existing_info = src_declaration.get_jsdoc_info(ast);
        if existing_info.is_none() {
            // Extract inline JSDoc from "src" and add it to the "dest" node.
            existing_info = src_name.get_jsdoc_info(ast);
            src_name.set_jsdoc_info(ast, None);
        }
        let mut builder = JSDocInfoBuilder::maybe_copy_from(existing_info.as_deref());
        let built = builder.build();
        dest_declaration.set_jsdoc_info(ast, built);
    }

    // port: Es6RewriteBlockScopedDeclaration#maybeAddConstJSDoc
    fn maybe_add_const_jsdoc(
        ast: &mut Ast,
        src_declaration: NodeId,
        src_name: NodeId,
        dest_declaration: NodeId,
    ) {
        if src_declaration.is_const(ast) {
            Self::extract_inline_jsdoc(ast, src_declaration, src_name, dest_declaration);
            let info = dest_declaration.get_jsdoc_info(ast);
            let mut builder = JSDocInfoBuilder::maybe_copy_from(info.as_deref());
            builder.record_constancy();
            let built = builder.build();
            dest_declaration.set_jsdoc_info(ast, built);
        }
    }

    /// Given a declarationList of let/const declarations, convert all declared names to their own,
    /// separate (normalized) var declarations.
    ///
    /// "const i = 0, j = 0;" becomes "/** @const */ var i = 0; /** @const */ var j = 0;"
    ///
    /// If the declarationList of let/const declarations is in a FOR intializer, moves those into
    /// separate declarations outside the FOR loop (for maintaining normalization).
    ///
    /// We can only have let/const declaration lists (not var) here as this pass runs post
    /// normalize.
    // port: Es6RewriteBlockScopedDeclaration#handleLetConstDeclarationList
    fn handle_let_const_declaration_list(
        &self,
        compiler: &mut AbstractCompiler,
        declaration_list: NodeId,
        parent: NodeId,
    ) {
        if declaration_list.is_var(compiler) {
            // This declaration list is already handled and we can safely return. This happens
            // because this method is also called by {@code replaceDeclarationWithProperty} where it
            // gets repeatedly called for each name in the declarationList. After the first call
            // to this (for the first name), this declarationList would no longer be {@code
            // Token.CONST} (i.e. it would've become a separate var with an {@code /** const */}
            // annotation).
            check_state!(
                !declaration_list.has_children(compiler)
                    || declaration_list.has_one_child(compiler),
                "var declaration can have maximum one child post normalization."
            );
            return;
        }

        if parent.is_vanilla_for(compiler) {
            // This is needed to handle the case where we get:
            //
            //   1. `for (let x = ...)` or
            //   2. `for (const x = ...`.
            //
            // Normalize only moves "var" outside the for initializer, it allows let/const within
            // the initializer. So both #1 and #2 can exist regardless if this pass runs before and
            // after normalize.
            self.handle_let_const_declaration_list_in_vanilla_for_initializer(
                compiler,
                declaration_list,
                parent,
            );
            return;
        }

        if parent.is_for_in(compiler) {
            // This is needed to handle the case where we get:
            //
            //   1. `for (let x in ...)` or
            //   2. `for (const x in ...`.
            //
            // Normalize only moves "var" outside the for initializer, it allows let/const within
            // the initializer. So both #1 and #2 can exist regardless if this pass runs before and
            // after normalize.
            Self::handle_let_const_declaration_list_in_for_in_initializer(
                compiler,
                declaration_list,
                parent,
            );
            return;
        }

        // convert all names to their own, separate (normalized) declarations
        while declaration_list.has_children(compiler) {
            let name = check_not_null!(declaration_list.get_last_child(compiler));
            let detached = name.detach(compiler);
            let new_declaration = IR::var(compiler, detached).srcref(compiler, declaration_list);
            // This method gets called multiple times by {@code replaceDeclarationWithProperty}. In
            // the first call, it splits the declarationList into individual /** @const */ var
            // declarations. i.e. `const a,b` --> `/** @const */ var a; /** @const */ var b;`
            //
            // Then it gets invoked for each individual var declaration. once for each name in the
            // original declarationList). However, after the first call to this (for the first
            // name), this declarationList would no longer be {@code Token.CONST} (i.e. it would've
            // become a separate var with an {@code /** const */} annotation).
            //
            // Previously, this method rewrote those individual var declarations again into new var
            // declarations. Without copying over the JSDoc, those newly created var declarations
            // did not contain `@const`, and produce this:
            //
            // `/** @const */ var a; /** @const */ var b;` -->`var a; var b;`
            //
            // Now, even though the "redundant rewriting" is handled by early-returning from this
            // method whenever this method gets called with a non-declaration var list, it's still
            // important that the individual var declarations created by the first rewriting
            // contain the annotations from original declaration list. Hence we propagate JSDoc
            // from declarationList into the individual var declarations.
            Self::extract_inline_jsdoc(compiler, declaration_list, name, new_declaration);
            Self::maybe_add_const_jsdoc(compiler, declaration_list, name, new_declaration);
            new_declaration.insert_after(compiler, declaration_list);
            compiler.report_change_to_enclosing_scope(parent);
        }

        // declarationList has no children left. Remove.
        declaration_list.detach(compiler);
        compiler.report_change_to_enclosing_scope(parent);
    }

    /// We can only have let/const declaration lists (not var) here as this pass runs post
    /// normalize.
    // port: Es6RewriteBlockScopedDeclaration#handleLetConstDeclarationListInForInInitializer
    fn handle_let_const_declaration_list_in_for_in_initializer(
        compiler: &mut AbstractCompiler,
        declaration_list: NodeId,
        parent: NodeId,
    ) {
        check_state!(parent.is_for_in(compiler));
        let first = declaration_list;
        let lhs = check_not_null!(first.get_first_child(compiler));
        let loop_node = parent;
        if lhs.is_destructuring_lhs(compiler) {
            // This pass relies on destructuring syntax being already removed. Hence we must not
            // enter this case.
            // TODO: b/279640656 Enable this code path once this pass runs unconditionally in
            // stage3.
            check_state!(
                false,
                "Destructuring syntax is unsupported in ES6RewriteBlockScopedDeclarations pass"
            );
            // Transform:
            //    `for (let [a, b = 3] in c) {}` or `for (const [a, b = 3] in c) {}`
            // to:
            //    `var a; var b; for ([a, b = 3] in c) {}`
            // respectively
            NodeUtil::visit_lhs_nodes_in_node(compiler, lhs, &mut |compiler, name| {
                // Add a declaration outside the for loop for the given name.
                check_state!(
                    name.is_name(compiler),
                    "lhs in destructuring declaration should be a simple name. (%s)",
                    name.to_string(compiler)
                );
                let name_string = name.get_string(compiler);
                let new_name = IR::name(compiler, name_string).srcref(compiler, name);
                let new_var = IR::var(compiler, new_name).srcref(compiler, name);
                Self::extract_inline_jsdoc(compiler, declaration_list, name, new_var);
                // if the initializer name was a const, the newName must no longer be a const.
                Self::add_node_before_loop(compiler, new_var, loop_node);
            });

            // Transform `for (var [a, b]... )` to `for ([a, b]...`
            let destructuring_pattern = check_not_null!(lhs.remove_first_child(compiler));
            first.replace_with(compiler, destructuring_pattern);
        } else {
            // Transform:
            //    for (let a in b) {}
            // to:
            //    var a; for (a in b) {};
            // and:
            //    for (const a in b) {}
            // to:
            //    var a; for (a in b) {};
            let new_statement = first.clone_tree(compiler);
            let name =
                check_not_null!(new_statement.get_first_child(compiler)).clone_node(compiler);
            // cloning also copies over any properties
            // Java: `if (name.getBooleanProp(Node.IS_CONSTANT_NAME)) {}` (empty branch):
            // if the initializer name was a const, it must no longer be a marked const. However,
            // marking it as non-const requires us to also change all references to the initializer
            // name in the for-in's body. That is expensive to do. Hence, we leave the initializer
            // name and its references within the loop to be const (trade off optimizability for
            // compile time).
            new_statement.set_token(compiler, Token::VAR);
            let first_name = check_not_null!(first.get_first_child(compiler));
            Self::extract_inline_jsdoc(compiler, first, first_name, new_statement);
            first.replace_with(compiler, name);
            Self::add_node_before_loop(compiler, new_statement, loop_node);
        }
    }

    /// We can only have let/const declaration lists (not var) here as this pass runs post
    /// normalize.
    // port: Es6RewriteBlockScopedDeclaration#handleLetConstDeclarationListInVanillaForInitializer
    fn handle_let_const_declaration_list_in_vanilla_for_initializer(
        &self,
        compiler: &mut AbstractCompiler,
        declaration_list: NodeId,
        parent: NodeId,
    ) {
        check_state!(parent.is_vanilla_for(compiler));
        // if the declarationList is in a FOR initializer, move it outside
        let mut insert_spot = Self::get_insert_spot_before_loop(compiler, parent);
        // convert all names to their own, separate (normalized) declarations
        while declaration_list.has_children(compiler) {
            let name = check_not_null!(declaration_list.get_last_child(compiler));
            let detached = name.detach(compiler);
            let new_declaration = IR::var(compiler, detached).srcref(compiler, declaration_list);
            Self::extract_inline_jsdoc(compiler, declaration_list, name, new_declaration);
            // Java: `if (name.getBooleanProp(Node.IS_CONSTANT_NAME)) {}` (empty branch):
            // if the initializer name was a const, it must no longer be a marked const. However,
            // marking it as non-const requires us to also change all references to the initializer
            // name in the for's body. That is expensive to do. Hence, we leave the initializer
            // name and its references within the loop to be const (trade off optimizability for
            // compile time).

            // generate normalized var initializer (i.e. outside FOR)
            new_declaration.insert_before(compiler, insert_spot);
            insert_spot = new_declaration;
            compiler.report_change_to_enclosing_scope(parent);
        }
        // make FOR initializer empty `for (; cond; incr)`
        let empty = self
            .ast_factory
            .create_empty(compiler)
            .srcref(compiler, declaration_list);
        declaration_list.replace_with(compiler, empty);
        compiler.report_change_to_enclosing_scope(empty);
    }

    // port: Es6RewriteBlockScopedDeclaration#addNodeBeforeLoop
    fn add_node_before_loop(compiler: &mut AbstractCompiler, new_node: NodeId, loop_node: NodeId) {
        let insert_spot = Self::get_insert_spot_before_loop(compiler, loop_node);
        new_node.insert_before(compiler, insert_spot);
        compiler.report_change_to_enclosing_scope(new_node);
    }

    // port: Es6RewriteBlockScopedDeclaration#getInsertSpotBeforeLoop
    fn get_insert_spot_before_loop(ast: &Ast, loop_node: NodeId) -> NodeId {
        let mut insert_spot = loop_node;
        while check_not_null!(insert_spot.get_parent(ast)).is_label(ast) {
            insert_spot = check_not_null!(insert_spot.get_parent(ast));
        }
        insert_spot
    }

    // port: Es6RewriteBlockScopedDeclaration#rewriteDeclsToVars
    fn rewrite_decls_to_vars(&self, compiler: &mut AbstractCompiler) {
        if !self.let_consts.is_empty() {
            for &n in &self.let_consts {
                // The Normalize pass splits simple let/const/var declarations already. But it
                // doesn't split let/const declaration list if it's in a for-loop initializer.
                // Normalize only moves "var" outside the for initializer, it allows let/const
                // within the initializer. But when we're rewriting those initializer let/const
                // delcaration lists into simple "vars" lists here, we split the var declaration
                // list to maintain normalization.
                let parent = check_not_null!(n.get_parent(compiler));
                self.handle_let_const_declaration_list(compiler, n, parent);
            }
        }
    }
}

impl CompilerPass for Es6RewriteBlockScopedDeclaration {
    // port: Es6RewriteBlockScopedDeclaration#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // All declared names are already unique post normalize, so we can safely just switch
        // 'let' or 'const' to 'var' in all non-loop cases.
        // - Gather a list of let & const variables
        // - Also add `= void 0` to any that are not initialized.
        NodeTraversal::traverse(compiler, root, self);
        let mut transformer = LoopClosureTransformer::new();
        NodeTraversal::traverse(compiler, root, &mut transformer);
        transformer.transform_loop_closure(compiler, self);
        self.rewrite_decls_to_vars(compiler);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

impl Callback for Es6RewriteBlockScopedDeclaration {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteBlockScopedDeclaration#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // NOTE: This pass depends on for-of being transpiled away before it runs.
        check_state!(
            parent.is_none_or(|parent| !parent.is_for_of(t)),
            "%s",
            parent.map_or_else(|| "null".to_string(), |parent| parent.to_string(t))
        );

        if n.is_let(t) || n.is_const(t) {
            // nothing to do for vars here
            self.let_consts.insert(n);
            let mut name_node = n.get_first_child(t);
            while let Some(name) = name_node {
                self.visit_block_scoped_name_declaration(t.get_compiler(), n, name);
                name_node = name.get_next(t);
            }
        }
    }
}

const LOOP_OBJECT_NAME: &str = "$jscomp$loop";

/// Transforms let/const declarations captured by loop closures.
///
/// Java's inner class reaches the enclosing pass's fields; here the pass is passed in after the
/// traversal. LoopObjects are identity objects in Java; they live in `loop_objects` and are named
/// by index.
// port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer
struct LoopClosureTransformer {
    loop_objects: Vec<LoopObject>,
    loop_object_map: IndexMap<NodeId, usize>,
    nodes_requiring_loop_objects_closure_map: IndexMap<NodeId, IndexSet<usize>>,
    nodes_handled_for_loop_object_closure: IndexSet<(NodeId, JsString)>,
    reference_map: IndexMap<VarId, IndexSet<NodeId>>,
}

impl LoopClosureTransformer {
    fn new() -> Self {
        Self {
            loop_objects: Vec::new(),
            loop_object_map: IndexMap::<_, _>::default(),
            nodes_requiring_loop_objects_closure_map: IndexMap::<_, _>::default(),
            nodes_handled_for_loop_object_closure: IndexSet::<_>::default(),
            reference_map: IndexMap::<_, _>::default(),
        }
    }

    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#createUniqueObjectName
    fn create_unique_object_name(
        compiler: &mut AbstractCompiler,
        input: &crate::compiler_input::CompilerInput,
    ) -> String {
        format!(
            "{LOOP_OBJECT_NAME}${}",
            compiler.get_unique_id_supplier().get_unique_id(input)
        )
    }

    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#getLoopObjPropName
    fn get_loop_obj_prop_name(compiler: &AbstractCompiler, var: VarId) -> JsString {
        // NOTE: var.getName() would be wrong here, because it will still contain the original
        // and possibly non-unique name for the variable. However, the name node itself will
        // already have the new and guaranteed-globally-unique name.
        check_not_null!(var.get_name_node(compiler)).get_string(compiler)
    }

    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#transformLoopClosure
    fn transform_loop_closure(
        &mut self,
        compiler: &mut AbstractCompiler,
        outer: &mut Es6RewriteBlockScopedDeclaration,
    ) {
        if self.loop_object_map.is_empty() {
            return;
        }

        let wrapper_functions = self.create_wrapper_functions(compiler, outer);

        let loop_nodes: Vec<NodeId> = self.loop_object_map.keys().copied().collect();
        for loop_node in loop_nodes {
            // Introduce objects to reflect the captured scope variables.
            // Fields are initially left as undefined to avoid cases like:
            //   var $jscomp$loop$0 = {$jscomp$loop$prop$i: 0, $jscomp$loop$prop$j: $jscomp$loop$0.i}
            // They are initialized lazily by changing declarations into assignments
            // later.
            let loop_object = self.loop_object_map[&loop_node];
            let object_lit_next_iteration = outer.ast_factory.create_object_lit(compiler, &[]);
            self.rename_vars_to_properties(
                compiler,
                outer,
                loop_object,
                object_lit_next_iteration,
                loop_node,
            );

            let loop_object_name = self.create_loop_object_name_node(compiler, outer, loop_object);
            let update_loop_object = outer.ast_factory.create_assign(
                compiler,
                loop_object_name,
                object_lit_next_iteration,
            );
            // This is the only time we generate a loop object variable for a loopNode.
            let loop_object_name = self.create_loop_object_name_node(compiler, outer, loop_object);
            let empty_object_lit = outer.ast_factory.create_object_lit(compiler, &[]);
            let object_lit = IR::var_with_value(compiler, loop_object_name, empty_object_lit)
                .srcref_tree(compiler, loop_node);
            Es6RewriteBlockScopedDeclaration::add_node_before_loop(compiler, object_lit, loop_node);
            if loop_node.is_vanilla_for(compiler) {
                // For
                self.change_vanilla_for_loop_header(compiler, outer, loop_node, update_loop_object);
            } else {
                let loop_body = check_not_null!(NodeUtil::get_loop_code_block(compiler, loop_node));
                let expr = IR::expr_result(compiler, update_loop_object)
                    .srcref_tree_if_missing(compiler, loop_node);
                loop_body.add_child_to_front(compiler, expr);
            }
            compiler.report_change_to_enclosing_scope(loop_node);

            self.change_loop_local_variables_to_properties(compiler, outer, loop_node, loop_object);
        }

        // At this point, all local variables in the loop have been changed to property accesses on
        // the "loopObject" name. For the wrapper functions that we introduced in the loop, we must
        // change the name references in their body to refer to their parameter name instead of
        // the "loopObject" name.
        // TODO(bradfordcsmith): This is inefficient. We should really choose the names first, then
        // make the changes, instead of changing the same variable references twice.
        Self::update_names_in_wrapper_functions(compiler, &wrapper_functions);
    }

    /// Before:
    ///
    /// ```text
    /// var arr = [];
    /// var LOOP$0 = {};
    /// var i = 0;
    /// for (; i < 10; LOOP$0 = {y: LOOP$0.y}, i++) {
    ///   LOOP$0.y = i;
    ///   arr.push((function(LOOP$0$PARAM$1) {
    ///       return function() { return LOOP$0.y; }; <---- must use param name
    ///   })(LOOP$0));
    /// }
    /// ```
    ///
    /// After:
    ///
    /// ```text
    /// var arr = [];
    /// var LOOP$0 = {};
    /// var i = 0;
    /// for (; i < 10; LOOP$0 = {y: LOOP$0.y}, i++) {
    ///   LOOP$0.y = i;
    ///   arr.push((function(LOOP$0$PARAM$1) {
    ///       return function() { return LOOP$0$PARAM$1.y; }; <--- changed
    ///   })(LOOP$0));
    /// }
    /// ```
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#updateNamesInWrapperFunctions
    fn update_names_in_wrapper_functions(
        compiler: &mut AbstractCompiler,
        wrapper_functions: &IndexSet<NodeId>,
    ) {
        for &func in wrapper_functions {
            // get the param names here (and the loopObject names from it)
            let param_list = check_not_null!(func.get_second_child(compiler));

            let mut param = param_list.get_first_child(compiler);
            while let Some(p) = param {
                let param_string = p.get_string(compiler).to_string();
                let index = check_not_null!(param_string.find(LOOP_PARAM_NAME_PREFIX));
                let loop_object_name = param_string[..index].to_string();
                let block = check_not_null!(func.get_last_child(compiler));
                Self::update_names(compiler, /* block */ block, p, &loop_object_name);
                param = p.get_next(compiler);
            }
        }
    }

    /// Create wrapper functions and call them.
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#createWrapperFunctions
    fn create_wrapper_functions(
        &mut self,
        compiler: &mut AbstractCompiler,
        outer: &mut Es6RewriteBlockScopedDeclaration,
    ) -> IndexSet<NodeId> {
        let mut wrapper_functions = IndexSet::<_>::default();
        let keys: Vec<NodeId> = self
            .nodes_requiring_loop_objects_closure_map
            .keys()
            .copied()
            .collect();
        for function_or_object_lit in keys {
            // Wrapping an object literal in an IIFE would otherwise rebind `this` and `arguments`
            // in object properties/methods to the IIFE. Use ThisAndArgumentsReferenceUpdater to
            // capture and rewrite them to outer aliases before wrapping in the IIFE.
            if function_or_object_lit.is_object_lit(compiler) {
                let enclosing_function =
                    NodeUtil::get_enclosing_function(compiler, function_or_object_lit);
                let scope_body = match enclosing_function {
                    Some(enclosing_function) => {
                        check_not_null!(enclosing_function.get_last_child(compiler))
                    }
                    None => check_not_null!(NodeUtil::get_enclosing_script(
                        compiler,
                        function_or_object_lit
                    )),
                };
                let is_constructor = enclosing_function.is_some_and(|enclosing_function| {
                    NodeUtil::is_es6_constructor(compiler, enclosing_function)
                });
                let script = check_not_null!(NodeUtil::get_enclosing_script(
                    compiler,
                    function_or_object_lit
                ));
                let input_id = check_not_null!(script.get_input_id(compiler));
                let input = check_not_null!(compiler.get_input(&input_id).cloned());
                let unique_id = compiler.get_unique_id_supplier().get_unique_id(&input);
                let mut this_context =
                    ThisAndArgumentsContext::new(scope_body, is_constructor, unique_id);
                {
                    let mut updater = ThisAndArgumentsReferenceUpdater::new(
                        &mut this_context,
                        &outer.ast_factory,
                    );
                    NodeTraversal::traverse(compiler, function_or_object_lit, &mut updater);
                }
                this_context.add_var_declarations(compiler, &outer.ast_factory, script);
                let mut child = scope_body.get_first_child(compiler);
                while let Some(c) = child {
                    if c.is_let(compiler) || c.is_const(compiler) {
                        outer.let_consts.insert(c);
                    }
                    child = c.get_next(compiler);
                }
            }
            let return_node = IR::return_node(compiler);
            let objects: Vec<usize> = self.nodes_requiring_loop_objects_closure_map
                [&function_or_object_lit]
                .iter()
                .copied()
                .collect();
            let mut parameter_names = Vec::with_capacity(objects.len());
            let mut object_names_for_call = Vec::with_capacity(objects.len());
            for object in objects {
                // This name for parameter should be unique to preserve normalization
                parameter_names.push(self.create_unique_parameter_name_to_use_within_loop(
                    compiler,
                    outer,
                    function_or_object_lit,
                    object,
                ));
                // this name must be the same as the loop object name
                object_names_for_call
                    .push(self.create_loop_object_name_node(compiler, outer, object));
            }

            let param_list = IR::param_list(compiler, &parameter_names);
            let body = IR::block_with_child(compiler, return_node);
            let iife = outer.ast_factory.create_function(
                compiler,
                "",
                param_list,
                body,
                AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
            );
            compiler.report_change_to_change_scope(iife);
            let call = outer.ast_factory.create_call(
                compiler,
                iife,
                AstFactory::type_node(function_or_object_lit),
                &object_names_for_call,
            );
            call.put_boolean_prop(compiler, Prop::FREE_CALL, true);
            // Es6RewriteBlockScopedFunctionDeclaration must run before this pass.
            check_state!(
                !NodeUtil::is_function_declaration(compiler, function_or_object_lit),
                "block-scoped function declarations should not exist now: %s",
                function_or_object_lit.to_string(compiler)
            );
            let replacement = call.srcref_tree_if_missing(compiler, function_or_object_lit);
            function_or_object_lit.replace_with(compiler, replacement);
            wrapper_functions.insert(iife);
            return_node.add_child_to_front(compiler, function_or_object_lit);
            compiler.report_change_to_enclosing_scope(replacement);
        }
        wrapper_functions
    }

    /// Renames all references within the functionOrObjectLit body from the old loopObjectName
    /// (e.g. "$jscomp$loop$1") to the given loopObjectParamName
    /// (e.g. $jscomp$loop$1$jscomp$loop_param$m123..456).
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#updateNames
    fn update_names(
        compiler: &mut AbstractCompiler,
        node: NodeId,
        param_name: NodeId,
        loop_object_name: &str,
    ) {
        if node.is_name(compiler)
            && !node.get_string_ref(compiler).is_empty()
            /* not an anonymous function name */
            && node.get_string(compiler) == loop_object_name
        {
            let new_name = param_name.get_string(compiler);
            node.set_string(compiler, new_name);
            return;
        }
        let mut child = node.get_first_child(compiler);
        while let Some(c) = child {
            Self::update_names(compiler, c, param_name, loop_object_name);
            child = c.get_next(compiler);
        }
    }

    /// The initializer is pulled out and placed prior to the loop. The increment is updated with
    /// the new loop object and property assignments
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#changeVanillaForLoopHeader
    fn change_vanilla_for_loop_header(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        loop_node: NodeId,
        update_loop_object: NodeId,
    ) {
        let mut initializer = check_not_null!(loop_node.get_first_child(compiler));
        let empty = IR::empty(compiler);
        initializer.replace_with(compiler, empty);
        if !initializer.is_empty(compiler) {
            if !NodeUtil::is_name_declaration(compiler, Some(initializer)) {
                initializer = IR::expr_result(compiler, initializer).srcref(compiler, initializer);
            }
            Es6RewriteBlockScopedDeclaration::add_node_before_loop(
                compiler,
                initializer,
                loop_node,
            );
        }

        let increment = check_not_null!(loop_node.get_child_at_index(compiler, 2));
        if increment.is_empty(compiler) {
            let replacement = update_loop_object.srcref_tree_if_missing(compiler, loop_node);
            increment.replace_with(compiler, replacement);
        } else {
            let place_holder = IR::empty(compiler);
            increment.replace_with(compiler, place_holder);
            let comma = outer
                .ast_factory
                .create_comma(compiler, update_loop_object, increment)
                .srcref_tree_if_missing(compiler, loop_node);
            place_holder.replace_with(compiler, comma);
        }
    }

    /// For captured variables, change declarations to assignments on the corresponding field of
    /// the introduced object. Rename all references accordingly.
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#changeLoopLocalVariablesToProperties
    fn change_loop_local_variables_to_properties(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &mut Es6RewriteBlockScopedDeclaration,
        loop_node: NodeId,
        loop_object: usize,
    ) {
        let vars: Vec<VarId> = self.loop_objects[loop_object]
            .vars
            .iter()
            .copied()
            .collect();
        for var in vars {
            let new_property_name = Self::get_loop_obj_prop_name(compiler, var);
            let references: Vec<NodeId> = self
                .reference_map
                .get(&var)
                .map(|references| references.iter().copied().collect())
                .unwrap_or_default();
            for reference in references {
                // for-of loops are transpiled away before this pass runs
                check_state!(
                    !loop_node.is_for_of(compiler),
                    "%s",
                    loop_node.to_string(compiler)
                );
                // For-of and for-in declarations are not altered, since they are
                // used as temporary variables for assignment.
                if loop_node.is_for_in(compiler)
                    && loop_node.get_first_child(compiler) == reference.get_parent(compiler)
                {
                    self.assign_loop_var_to_loop_object_property(
                        compiler,
                        outer,
                        loop_node,
                        loop_object,
                        var,
                        &new_property_name,
                        reference,
                    );
                } else if NodeUtil::is_name_declaration(compiler, reference.get_parent(compiler)) {
                    self.replace_declaration_with_property(
                        compiler,
                        outer,
                        loop_object,
                        &new_property_name,
                        reference,
                    );
                } else {
                    self.replace_reference_with_property(
                        compiler,
                        outer,
                        loop_object,
                        &new_property_name,
                        reference,
                    );
                }
            }
        }
    }

    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#replaceDeclarationWithProperty
    fn replace_declaration_with_property(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &mut Es6RewriteBlockScopedDeclaration,
        loop_object: usize,
        new_property_name: &JsString,
        reference: NodeId,
    ) {
        let mut declaration = check_not_null!(reference.get_parent(compiler));
        let grand_parent = check_not_null!(declaration.get_parent(compiler));
        // Record that the let / const declaration statement will get turned into one or more
        // var statements by handleDeclarationList(), so we won't try to change it again later.
        outer.let_consts.shift_remove(&declaration);
        // If the declaration contains multiple declared variables, split it apart.
        // NOTE: This call could be made for each declarationList once, rather than each name in
        // that list
        outer.handle_let_const_declaration_list(compiler, declaration, grand_parent);

        // The variable we're working with may have been moved to a new var statement.
        declaration = check_not_null!(reference.get_parent(compiler));
        if reference.has_children(compiler) {
            // Change declaration to assignment
            let new_reference = self.create_loop_var_reference_replacement(
                compiler,
                outer,
                loop_object,
                reference,
                new_property_name,
            );
            let value = check_not_null!(reference.remove_first_child(compiler));
            let assign = outer
                .ast_factory
                .create_assign(compiler, new_reference, value);
            Es6RewriteBlockScopedDeclaration::extract_inline_jsdoc(
                compiler,
                declaration,
                reference,
                declaration,
            );
            Es6RewriteBlockScopedDeclaration::maybe_add_const_jsdoc(
                compiler,
                declaration,
                reference,
                declaration,
            );
            let info = declaration.get_jsdoc_info(compiler);
            assign.set_jsdoc_info(compiler, info);

            let replacement =
                IR::expr_result(compiler, assign).srcref_tree_if_missing(compiler, declaration);
            declaration.replace_with(compiler, replacement);
        } else {
            // No value is assigned, so just drop the let/const statement entirely
            declaration.detach(compiler);
        }
        compiler.report_change_to_enclosing_scope(grand_parent);
    }

    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#replaceReferenceWithProperty
    fn replace_reference_with_property(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        loop_object: usize,
        new_property_name: &JsString,
        reference: NodeId,
    ) {
        let reference_parent = check_not_null!(reference.get_parent(compiler));
        let replacement = self.create_loop_var_reference_replacement(
            compiler,
            outer,
            loop_object,
            reference,
            new_property_name,
        );
        reference.replace_with(compiler, replacement);
        compiler.report_change_to_enclosing_scope(reference_parent);
    }

    /// Transforms `for (const p in obj) { ... }`
    ///
    /// into `for (const p in obj) { $jscomp$loop$0.$jscomp$loop$prop$0$p = p; ... }`
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#assignLoopVarToLoopObjectProperty
    #[allow(clippy::too_many_arguments)]
    fn assign_loop_var_to_loop_object_property(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        loop_node: NodeId,
        loop_object: usize,
        var: VarId,
        new_property_name: &JsString,
        reference: NodeId,
    ) {
        // reference is the node loopVar in a for-in that looks like this:
        // `for (const loopVar in list) {`
        check_state!(
            Some(reference) == var.get_name_node(compiler),
            "%s",
            reference.to_string(compiler)
        );
        let reference_parent = reference.get_parent(compiler);
        check_state!(
            NodeUtil::is_name_declaration(compiler, reference_parent),
            "%s",
            reference_parent.map_or_else(|| "null".to_string(), |p| p.to_string(compiler))
        );
        check_state!(
            reference.is_name(compiler),
            "%s",
            reference.to_string(compiler)
        );
        // Start transpiled form of
        // `for (const p in obj) { ... }`
        // with this statement to copy the loop variable into the corresponding loop object
        // property.
        // `$jscomp$loop$0.$jscomp$loop$prop$0$p = p;`
        let loop_var_reference = reference.clone_node(compiler);
        // `$jscomp$loop$0.$jscomp$loop$prop$0$p = p;`
        let lhs = self.create_loop_var_reference_replacement(
            compiler,
            outer,
            loop_object,
            reference,
            new_property_name,
        );
        let assign = outer
            .ast_factory
            .create_assign(compiler, lhs, loop_var_reference);
        let for_in_prop_assignment_statemnt =
            IR::expr_result(compiler, assign).srcref_tree_if_missing(compiler, reference);
        // The first statement in the body should be creating a new loop object value
        // $jscomp$loop$0 = {
        //    $jscomp$loop$prop$0$p: $jscomp$loop$0.$jscomp$loop$prop$0$p,
        //    $jscomp$loop$prop$0$otherVar: $jscomp$loop$0.$jscomp$loop$prop$0$p,
        //    // other property update assignments
        // }
        // We need to update the loop variable's value to it immediately after that
        let loop_update_statement = check_not_null!(
            check_not_null!(loop_node.get_last_child(compiler)) // loop body
                .get_first_child(compiler)
        ); // first statement

        for_in_prop_assignment_statemnt.insert_after(compiler, loop_update_statement);
    }

    /// Rename all variables in the loop object to properties
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#renameVarsToProperties
    fn rename_vars_to_properties(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        loop_object: usize,
        object_lit_next_iteration: NodeId,
        scope_root: NodeId,
    ) {
        let vars: Vec<VarId> = self.loop_objects[loop_object]
            .vars
            .iter()
            .copied()
            .collect();
        for var in vars {
            let new_property_name = Self::get_loop_obj_prop_name(compiler, var);
            let new_property_value = if var.get_scope_root(compiler) == scope_root {
                let name_node = check_not_null!(var.get_name_node(compiler));
                self.create_loop_var_reference_replacement(
                    compiler,
                    outer,
                    loop_object,
                    name_node,
                    &new_property_name,
                )
            } else {
                outer.ast_factory.create_undefined_value(compiler)
            };
            let string_key = outer.ast_factory.create_string_key(
                compiler,
                new_property_name,
                new_property_value,
            );
            object_lit_next_iteration.add_child_to_back(compiler, string_key);
        }
    }

    /// Creates a `$jscomp$loop$0.$jscomp$loop$prop$varName$1` replacement for a reference to
    /// `varName`.
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#createLoopVarReferenceReplacement
    fn create_loop_var_reference_replacement(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        loop_object: usize,
        reference: NodeId,
        property_name: &JsString,
    ) -> NodeId {
        let loop_object_name = self.create_loop_object_name_node(compiler, outer, loop_object);
        let replacement = outer.ast_factory.create_get_prop(
            compiler,
            loop_object_name,
            property_name.clone(),
            AstFactory::type_node(reference),
        );
        replacement.srcref_tree(compiler, reference);
        replacement
    }

    /// Gets a name other than the loop object name to use for parameter names of the new wrapper
    /// functions being added in the loop.
    ///
    /// For a loop object "$jscomp$loop$0", this creates a unique parameter name like
    /// "$jscomp$loop$0$jscomp$loop_param$1".
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#createUniqueParameterNameToUseWithinLoop
    fn create_unique_parameter_name_to_use_within_loop(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        node: NodeId,
        loop_object: usize,
    ) -> NodeId {
        let input_id = check_not_null!(NodeUtil::get_input_id(compiler, node));
        let input = check_not_null!(compiler.get_input(&input_id).cloned());
        let unique_id = compiler.get_unique_id_supplier().get_unique_id(&input);
        outer.ast_factory.create_name(
            compiler,
            format!(
                "{}{LOOP_PARAM_NAME_PREFIX}{unique_id}",
                self.loop_objects[loop_object].name
            )
            .as_str(),
            AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
        )
    }

    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#createLoopObjectNameNode
    fn create_loop_object_name_node(
        &self,
        compiler: &mut AbstractCompiler,
        outer: &Es6RewriteBlockScopedDeclaration,
        loop_object: usize,
    ) -> NodeId {
        outer.ast_factory.create_name(
            compiler,
            self.loop_objects[loop_object].name.as_str(),
            AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
        )
    }
}

impl Callback for LoopClosureTransformer {
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if !NodeUtil::is_reference_name(t, n) {
            return true;
        }

        let name = n.get_string(t);
        let referenced_in: ScopeId = t.get_scope();
        let Some(var) = referenced_in.get_var(t.get_compiler(), name.clone()) else {
            return true;
        };

        let compiler = t.get_compiler();
        if !var.is_let(compiler) && !var.is_const(compiler) {
            return true;
        }

        // Traverse nodes up from let/const declaration:
        // If we hit a function or the root before a loop - Not a loop closure.
        // if we hit a loop first - maybe loop closure.
        let declared_in = var.get_scope(compiler);
        let loop_node;
        let mut s = declared_in;
        loop {
            let scope_root = s.get_root_node(compiler);
            if NodeUtil::is_loop_structure(compiler, scope_root) {
                loop_node = scope_root;
                break;
            } else if let Some(scope_root_parent) = scope_root.get_parent(compiler)
                && NodeUtil::is_loop_structure(compiler, scope_root_parent)
            {
                loop_node = scope_root_parent;
                break;
            } else if s.is_function_block_scope(compiler) || s.is_global(compiler) {
                return true;
            }
            s = check_not_null!(s.get_parent(compiler));
        }

        self.reference_map.entry(var).or_default().insert(n);

        // Traverse scopes from reference scope to declaration scope.
        // If we hit a function - loop closure detected.
        let mut outer_most_function_scope: Option<ScopeId> = None;
        let mut s = referenced_in;
        while s != declared_in && s.get_root_node(compiler) != loop_node {
            if s.is_function_scope(compiler) {
                outer_most_function_scope = Some(s);
            }
            s = check_not_null!(s.get_parent(compiler));
        }

        if let Some(outer_most_function_scope) = outer_most_function_scope {
            let enclosing_function = outer_most_function_scope.get_root_node(compiler);

            // There are two categories of functions we might find here:
            //  1. a getter or setter in an object literal. We will wrap the entire object literal
            //     in a closure to capture the value of the let/const.
            //  2. a function declaration or expression. We will wrap the function in a closure.
            // (At this point, class methods/getters/setters and object literal member functions
            // are transpiled away.)
            let enclosing_function_parent =
                check_not_null!(enclosing_function.get_parent(compiler));
            let node_to_wrap_in_closure = if enclosing_function_parent.is_getter_def(compiler)
                || enclosing_function_parent.is_setter_def(compiler)
            {
                let node_to_wrap_in_closure =
                    check_not_null!(enclosing_function.get_grandparent(compiler));
                check_state!(node_to_wrap_in_closure.is_object_lit(compiler));
                node_to_wrap_in_closure
            } else {
                enclosing_function
            };
            if self
                .nodes_handled_for_loop_object_closure
                .contains(&(node_to_wrap_in_closure, name.clone()))
            {
                return true;
            }
            self.nodes_handled_for_loop_object_closure
                .insert((node_to_wrap_in_closure, name));

            let object = match self.loop_object_map.get(&loop_node) {
                Some(&object) => object,
                None => {
                    let input = check_not_null!(t.get_input().cloned());
                    let object_name = Self::create_unique_object_name(t.get_compiler(), &input);
                    self.loop_objects.push(LoopObject::new(object_name));
                    let object = self.loop_objects.len() - 1;
                    self.loop_object_map.insert(loop_node, object);
                    object
                }
            };
            self.loop_objects[object].vars.insert(var);
            self.nodes_requiring_loop_objects_closure_map
                .entry(node_to_wrap_in_closure)
                .or_default()
                .insert(object);
        }
        true
    }

    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

// port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer.LoopObject
struct LoopObject {
    /// The name of the variable having the loop's internal variables as properties, and the label
    /// applied to the block containing the original loop body in cases where these are needed.
    name: String,

    vars: IndexSet<VarId>,
}

impl LoopObject {
    // port: Es6RewriteBlockScopedDeclaration.LoopClosureTransformer.LoopObject#LoopObject
    fn new(name: String) -> Self {
        Self {
            name,
            vars: IndexSet::<_>::default(),
        }
    }
}
