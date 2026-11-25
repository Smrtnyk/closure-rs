/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InlineSimpleMethods.java.

use crate::{
    abstract_compiler::AbstractCompiler, ast_analyzer::AstAnalyzer,
    coding_convention::CodingConvention, compiler_pass::CompilerPass,
    node_traversal::NodeTraversal, node_util::NodeUtil, parallel_parse::parallel_thread_count,
    source_file::bundled_script::BundledScript,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId, SnapshotUse},
    token::Token,
};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

/// Inlines methods that take no arguments and have only a return statement returning a property.
/// Because it works on method names rather than type inference, a method with multiple
/// definitions will be inlined if each definition is identical.
///
/// ```text
/// A.prototype.foo = function() { return this.b; }
/// B.prototype.foo = function() { return this.b; }
/// ```
///
/// will inline foo, but
///
/// ```text
/// A.prototype.foo = function() { return this.b; }
/// B.prototype.foo = function() { return this.c; }
/// ```
///
/// will not.
///
/// Declarations are not removed because we do not find all possible call sites. For examples,
/// calls of the form foo["bar"] are not detected.
///
/// This pass is not on by default because it is not safe in simple mode. If the prototype method
/// is mutated and we don't detect that, inlining it is unsafe. We enable it whenever function
/// inlining is enabled.
pub struct InlineSimpleMethods {
    /// Rust-only (performance divergence): shared interned extern property set instead of
    /// copying all extern properties into `non_inlineable_properties` on every pass run.
    extern_properties: Arc<IndexSet<JsString>>,

    // List of property names that we know are not safe to inline
    // This includes:
    //   - extern properties (we can't see the actual method definition for externs)
    //   - non-method properties
    //   - methods with @noinline
    //   - methods with multiple, non-equivalent definitions
    non_inlineable_properties: IndexSet<JsString>,

    // Use a linked map here to keep the output deterministic.  Otherwise,
    // the choice of method bodies is random when multiple identical definitions
    // are found which causes problems in the source maps.
    // (Java: LinkedHashMultimap<String, Node>)
    // Rust-only (performance divergence): pairs each definition set with the `mutation_epoch`
    // at which it last passed equivalence and inlineability checks.
    method_definitions: IndexMap<JsString, (IndexSet<NodeId>, u32)>,
    mutation_epoch: u32,

    ast_analyzer: AstAnalyzer,
}

impl InlineSimpleMethods {
    // port: InlineSimpleMethods#InlineSimpleMethods
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let ast_analyzer = compiler.get_ast_analyzer();
        let extern_properties = check_not_null!(compiler.get_extern_property_set_js()).clone();
        Self {
            extern_properties,
            non_inlineable_properties: IndexSet::<_>::default(),
            method_definitions: IndexMap::<_, _>::default(),
            mutation_epoch: 1,
            ast_analyzer,
        }
    }

    #[inline]
    fn is_non_inlineable(&self, name: &JsString) -> bool {
        self.extern_properties.contains(name) || self.non_inlineable_properties.contains(name)
    }

    /// Returns true if the provided node is a getprop for which the left child is this or a valid
    /// property tree and for which the right side is a string.
    // port: InlineSimpleMethods#isPropertyTree
    fn is_property_tree(ast: &closure_rhino::node::Ast, expected_getprop: NodeId) -> bool {
        if !expected_getprop.is_get_prop(ast) {
            return false;
        }

        let getprop_lhs = expected_getprop.get_first_child(ast).unwrap();
        getprop_lhs.is_this(ast) || Self::is_property_tree(ast, getprop_lhs)
    }

    /// Finds the occurrence of "this" in the provided property tree and replaces it with
    /// replacement
    // port: InlineSimpleMethods#replaceThis
    fn replace_this(
        ast: &mut closure_rhino::node::Ast,
        expected_getprop: NodeId,
        replacement: NodeId,
    ) {
        let left_child = expected_getprop.get_first_child(ast).unwrap();
        if left_child.is_this(ast) {
            left_child.replace_with(ast, replacement);
        } else {
            Self::replace_this(ast, left_child, replacement);
        }
    }

    /// Return the node that represents the expression returned by the method, given a FUNCTION
    /// node.
    // port: InlineSimpleMethods#returnedExpression
    fn returned_expression(ast: &closure_rhino::node::Ast, r#fn: NodeId) -> Option<NodeId> {
        let expected_block = NodeUtil::get_function_body(ast, r#fn);
        if !expected_block.has_one_child(ast) {
            return None;
        }

        let expected_return = expected_block.get_first_child(ast).unwrap();
        if !expected_return.is_return(ast) {
            return None;
        }

        if !expected_return.has_one_child(ast) {
            return None;
        }

        Some(expected_return.get_only_child(ast))
    }

    /// Return whether the given FUNCTION node is an empty method definition.
    ///
    /// Must be private, or moved to NodeUtil.
    // port: InlineSimpleMethods#isEmptyMethod
    fn is_empty_method(ast: &closure_rhino::node::Ast, r#fn: NodeId) -> bool {
        NodeUtil::is_empty_block(ast, NodeUtil::get_function_body(ast, r#fn))
    }

    /// Given a set of method definitions, verify they are the same.
    // port: InlineSimpleMethods#allDefinitionsEquivalent
    fn all_definitions_equivalent(
        &self,
        compiler: &AbstractCompiler,
        definitions: &IndexSet<NodeId>,
    ) -> bool {
        let mut first: Option<NodeId> = None;
        for &n in definitions {
            match first {
                None => first = Some(n),
                Some(first) => {
                    if !compiler.are_nodes_equal_for_inlining(first, n) {
                        return false;
                    } // else continue
                }
            }
        }
        true
    }

    // port: InlineSimpleMethods#anyDefinitionsNotInlineable
    fn any_definitions_not_inlineable(
        &self,
        compiler: &mut AbstractCompiler,
        definitions: &IndexSet<NodeId>,
    ) -> bool {
        for &n in definitions {
            if n.is_async_function(compiler) || n.is_generator_function(compiler) {
                // async and generator functions cannot be trivially inlined because they return a
                // Promise or generator object, not the return value directly. For now just don't
                // inline them - it's unclear it's worth the complexity.
                return true;
            }
            if NodeUtil::function_parameters_may_have_side_effects(compiler, n, &self.ast_analyzer)
            {
                // Methods with parameters that may have side effects or throw on destructuring are
                // not inlined to prevent altering runtime behavior.
                return true;
            }
            let js_doc_info =
                NodeUtil::get_best_jsdoc_info(compiler, n.get_parent(compiler).unwrap());
            if js_doc_info.is_some_and(|info| info.is_no_inline()) {
                return true;
            }
        }
        false
    }

    /// Replace the provided method call with the tree specified in returnedValue
    ///
    /// Parse tree of a call is name call getprop obj string
    // port: InlineSimpleMethods#inlinePropertyReturn
    fn inline_property_return(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        returned_value: NodeId,
    ) {
        let get_prop = returned_value.clone_tree(compiler);
        let replacement = call
            .get_first_child(compiler)
            .unwrap()
            .remove_first_child(compiler)
            .unwrap();
        Self::replace_this(compiler, get_prop, replacement);
        call.replace_with(compiler, get_prop);
        compiler.report_change_to_enclosing_scope(get_prop);
        self.mutation_epoch = self.mutation_epoch.wrapping_add(1);
    }

    /// Replace the provided object and its method call with the tree specified in returnedValue.
    /// Should be called only if the object reference has no side effects.
    // port: InlineSimpleMethods#inlineConstReturn
    fn inline_const_return(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        returned_value: NodeId,
    ) {
        let ret_value = returned_value.clone_tree(compiler);
        call.replace_with(compiler, ret_value);
        compiler.report_change_to_enclosing_scope(ret_value);
        self.mutation_epoch = self.mutation_epoch.wrapping_add(1);
    }

    /// Remove the provided object and its method call.
    // port: InlineSimpleMethods#inlineEmptyMethod
    fn inline_empty_method(
        &mut self,
        compiler: &mut AbstractCompiler,
        parent: NodeId,
        call: NodeId,
    ) {
        // If the return value of the method call is read,
        // replace it with "void 0". Otherwise, remove the call entirely.

        let replacement = if NodeUtil::is_expr_call(compiler, parent) {
            let empty = IR::empty(compiler);
            parent.replace_with(compiler, empty);
            NodeUtil::mark_functions_deleted(compiler, parent);
            empty
        } else {
            let src_location = call;
            let undefined = NodeUtil::new_undefined_node(compiler, Some(src_location));
            call.replace_with(compiler, undefined);
            NodeUtil::mark_functions_deleted(compiler, call);
            undefined
        };
        compiler.report_change_to_enclosing_scope(replacement);
        self.mutation_epoch = self.mutation_epoch.wrapping_add(1);
    }

    /// Check whether the given method call's arguments have side effects. `call` is the call node
    /// of a method invocation.
    // port: InlineSimpleMethods#argsMayHaveSideEffects
    fn args_may_have_side_effects(&self, compiler: &mut AbstractCompiler, call: NodeId) -> bool {
        let mut current_child = call.get_second_child(compiler);
        while let Some(child) = current_child {
            if self.ast_analyzer.may_have_side_effects(compiler, child) {
                return true;
            }
            current_child = child.get_next(compiler);
        }

        false
    }

    // port: InlineSimpleMethods#getPropName
    fn get_prop_name_ref(
        ast: &closure_rhino::node::Ast,
        get_prop_elem: NodeId,
    ) -> Option<&JsString> {
        if get_prop_elem.is_get_prop(ast) {
            Some(get_prop_elem.get_string_ref(ast))
        } else if get_prop_elem
            .get_second_child(ast)
            .unwrap()
            .is_string_lit(ast)
        {
            Some(
                get_prop_elem
                    .get_second_child(ast)
                    .unwrap()
                    .get_string_ref(ast),
            )
        } else {
            None
        }
    }
}

impl CompilerPass for InlineSimpleMethods {
    // port: InlineSimpleMethods#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let stage = compiler.get_life_cycle_stage();
        check_state!(stage.is_normalized(), "%s", format!("{stage:?}"));
        check_state!(self.method_definitions.is_empty());
        // checkState(externs != null): a NodeId is never null.

        let scope_root = check_not_null!(externs.get_parent(compiler));
        check_state!(root.get_parent(compiler) == Some(scope_root));
        self.gather_signatures_direct(compiler, externs, root);

        // Rust-only (performance divergence): strip properties already blacklisted during
        // GatherSignatures and skip the second traversal if no inlineable candidates remain.
        self.method_definitions.retain(|name, (defs, _)| {
            !defs.is_empty()
                && !self.extern_properties.contains(name)
                && !self.non_inlineable_properties.contains(name)
        });
        if !self.method_definitions.is_empty() {
            let mut bloom = [0u64; 64];
            for name in self.method_definitions.keys() {
                let h = name.hash_code() as usize;
                bloom[(h >> 6) & 63] |= 1u64 << (h & 63);
            }
            let mut inliner = InlineTrivialAccessors {
                outer: self,
                name_bloom: bloom,
                current_script: None,
                current_node: None,
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                inliner.walk_node(compiler, root, None);
            }));
            if let Err(payload) = result {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    inliner.current_script,
                    inliner.current_node,
                    payload,
                );
            }
        }
    }
}

struct WalkFailure {
    script: Option<NodeId>,
    node: Option<NodeId>,
    payload: Box<dyn Any + Send>,
}

/// Rust-only (performance divergence): per-bucket signature collector for parallel
/// `GatherSignatures`.
struct BucketGatherer<'a> {
    ast: &'a Ast,
    coding_convention: &'a dyn CodingConvention,
    extern_properties: &'a IndexSet<JsString>,
    non_inlineable_properties: IndexSet<JsString>,
    method_definitions: IndexMap<JsString, IndexSet<NodeId>>,
    current_script: Option<NodeId>,
    current_node: Option<NodeId>,
}

impl<'a> BucketGatherer<'a> {
    fn new(
        ast: &'a Ast,
        coding_convention: &'a dyn CodingConvention,
        extern_properties: &'a IndexSet<JsString>,
    ) -> Self {
        Self {
            ast,
            coding_convention,
            extern_properties,
            non_inlineable_properties: IndexSet::default(),
            method_definitions: IndexMap::default(),
            current_script: None,
            current_node: None,
        }
    }

    #[inline]
    fn is_non_inlineable(&self, name: &JsString) -> bool {
        self.extern_properties.contains(name) || self.non_inlineable_properties.contains(name)
    }

    // port: InlineSimpleMethods#addPossibleSignature
    // port: InlineSimpleMethods#addSignature
    fn add_possible_signature(&mut self, name: &JsString, node: Option<NodeId>) {
        if let Some(node) = node.filter(|node| node.is_function(self.ast)) {
            if !self.is_non_inlineable(name) {
                if let Some(set) = self.method_definitions.get_mut(name) {
                    set.insert(node);
                } else {
                    let mut set = IndexSet::default();
                    set.insert(node);
                    self.method_definitions.insert(name.clone(), set);
                }
            }
        } else if !self.is_non_inlineable(name) {
            self.non_inlineable_properties.insert(name.clone());
        }
    }

    // port: InlineSimpleMethods.GatherSignatures#processPrototypeParent
    fn process_prototype_parent(&mut self, n: NodeId) {
        let ast = self.ast;
        match n.get_token(ast) {
            Token::GETPROP | Token::GETELEM => {
                let grandparent = n.get_grandparent(ast).unwrap();
                let name = InlineSimpleMethods::get_prop_name_ref(ast, n);
                if let Some(name) = name
                    && grandparent.is_assign(ast)
                {
                    let assignee = grandparent.get_second_child(ast);
                    self.add_possible_signature(name, assignee);
                }
            }
            _ => {}
        }
    }

    fn walk_node(&mut self, n: NodeId, parent: Option<NodeId>) {
        let ast = self.ast;
        let token = n.get_token(ast);
        match token {
            Token::SCRIPT => {
                let prev = self.current_script;
                self.current_script = Some(n);
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    self.walk_node(c, Some(n));
                    child = c.get_next(ast);
                }
                self.current_script = prev;
                return;
            }
            Token::CLASS => {
                let class_name = n.get_first_child(ast).unwrap();
                let extends_clause = class_name.get_next(ast).unwrap();
                let body = extends_clause.get_next(ast).unwrap();
                self.walk_node(extends_clause, Some(n));
                self.walk_node(class_name, Some(n));
                self.walk_node(body, Some(n));
                return;
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if c.is_computed_prop(ast) || c.is_computed_field_def(ast) {
                        self.walk_node(c.get_first_child(ast).unwrap(), Some(c));
                    }
                    child = c.get_next(ast);
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    match c.get_token(ast) {
                        Token::COMPUTED_PROP => {
                            self.walk_node(c.get_last_child(ast).unwrap(), Some(c));
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(ast) {
                                self.walk_node(c.get_last_child(ast).unwrap(), Some(c));
                            }
                        }
                        _ => self.walk_node(c, Some(n)),
                    }
                    child = c.get_next(ast);
                }
            }
            _ => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    self.walk_node(c, Some(n));
                    child = c.get_next(ast);
                }
            }
        }
        self.visit(n, parent, token);
    }

    // port: InlineSimpleMethods.GatherSignatures#visit
    #[inline]
    fn visit(&mut self, n: NodeId, parent: Option<NodeId>, token: Token) {
        let ast = self.ast;
        match token {
            Token::GETPROP | Token::GETELEM => {
                let Some(name) = InlineSimpleMethods::get_prop_name_ref(ast, n) else {
                    return;
                };
                let parent = parent.unwrap();

                if *name == "prototype" {
                    self.process_prototype_parent(parent);
                } else if parent.is_assign(ast) && n.is_first_child_of(ast, Some(parent)) {
                    let next = n.get_next(ast);
                    self.add_possible_signature(name, next);
                }
            }
            Token::OBJECTLIT | Token::CLASS_MEMBERS => {
                self.current_node = Some(n);
                let mut key = n.get_first_child(ast);
                while let Some(k) = key {
                    match k.get_token(ast) {
                        Token::MEMBER_FUNCTION_DEF
                        | Token::MEMBER_FIELD_DEF
                        | Token::STRING_KEY => {
                            let name = k.get_string_ref(ast);
                            let first = k.get_first_child(ast);
                            self.add_possible_signature(name, first);
                        }
                        Token::SETTER_DEF | Token::GETTER_DEF => {
                            let name = k.get_string_ref(ast);
                            if !self.is_non_inlineable(name) {
                                self.non_inlineable_properties.insert(name.clone());
                            }
                        }
                        Token::COMPUTED_PROP
                        | Token::OBJECT_SPREAD
                        | Token::COMPUTED_FIELD_DEF
                        | Token::BLOCK => {}
                        _ => panic!("Unexpected {} key: {}", token, k.to_string(ast)),
                    }
                    key = k.get_next(ast);
                }
            }
            Token::CALL
                if self
                    .coding_convention
                    .is_property_rename_function(ast, n.get_first_child(ast).unwrap()) =>
            {
                let name = n.get_second_child(ast).unwrap().get_string(ast);
                self.non_inlineable_properties.insert(name);
            }
            _ => {}
        }
    }
}

impl InlineSimpleMethods {
    /// Rust-only (performance divergence): direct `&Ast` walk for `GatherSignatures`, fanning out
    /// script chunks across worker threads on large multi-script inputs while merging in exact AST
    /// order.
    fn gather_signatures_direct(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        let mut units: Vec<(NodeId, Option<NodeId>)> = Vec::new();
        for (is_externs, tree) in [(true, externs), (false, root)] {
            if tree.is_root(compiler) {
                let mut child = tree.get_first_child(compiler);
                while let Some(c) = child {
                    // Rust-only (performance divergence): unchanged bundled extern scripts whose
                    // properties are already in `self.extern_properties` and contain no calls
                    // cannot add any method definitions or new non-inlineable properties.
                    if is_externs
                        && c.is_script(compiler)
                        && let Some((facts, suffix)) = compiler
                            .parsed_script_record::<BundledScript>(
                                c,
                                SnapshotUse::SyntaxAndNonParamNames,
                            )
                        && !facts.may_have_calls
                        && facts
                            .properties
                            .iter()
                            .all(|&p| self.extern_properties.contains(&JsString::from(p)))
                    {
                        let mut st = suffix;
                        while let Some(s) = st {
                            units.push((s, Some(c)));
                            st = s.get_next(compiler);
                        }
                        child = c.get_next(compiler);
                        continue;
                    }
                    // Chunk large non-module code scripts by top-level statement so single-bundle
                    // inputs also parallelize across worker threads.
                    if !is_externs
                        && c.is_script(compiler)
                        && let Some(first) = c.get_first_child(compiler)
                        && !first.is_module_body(compiler)
                        && c.get_next(compiler).is_none()
                        && units.is_empty()
                    {
                        let mut st = Some(first);
                        while let Some(s) = st {
                            units.push((s, Some(c)));
                            st = s.get_next(compiler);
                        }
                    } else {
                        units.push((c, Some(tree)));
                    }
                    child = c.get_next(compiler);
                }
            } else {
                units.push((tree, tree.get_parent(compiler)));
            }
        }

        let threads = if compiler.ast.node_count() >= 20_000 {
            parallel_thread_count(units.len(), 2, 8)
        } else {
            1
        };

        if threads <= 1 {
            let outcome = {
                let ast: &Ast = &compiler.ast;
                let coding_convention = compiler.get_coding_convention();
                let mut gatherer =
                    BucketGatherer::new(ast, coding_convention, &self.extern_properties);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    for &(unit, parent) in &units {
                        if let Some(p) = parent
                            && p.is_script(ast)
                        {
                            gatherer.current_script = Some(p);
                        }
                        gatherer.walk_node(unit, parent);
                    }
                }));
                match result {
                    Ok(()) => Ok((
                        gatherer.non_inlineable_properties,
                        gatherer.method_definitions,
                    )),
                    Err(payload) => Err(WalkFailure {
                        script: gatherer.current_script,
                        node: gatherer.current_node,
                        payload,
                    }),
                }
            };
            match outcome {
                Ok((non_inlineable, method_defs)) => {
                    self.non_inlineable_properties.extend(non_inlineable);
                    for (name, defs) in method_defs {
                        if !self.is_non_inlineable(&name) {
                            self.method_definitions.insert(name, (defs, 0));
                        }
                    }
                }
                Err(fail) => {
                    NodeTraversal::throw_unexpected_exception_at(
                        compiler,
                        fail.script,
                        fail.node,
                        fail.payload,
                    );
                }
            }
            return;
        }

        let num_buckets = (threads * 2).min(units.len());
        let bucket_size = units.len().div_ceil(num_buckets);
        let slices: Vec<&[(NodeId, Option<NodeId>)]> = units.chunks(bucket_size).collect();
        type BucketSignatures = (IndexSet<JsString>, IndexMap<JsString, IndexSet<NodeId>>);
        let mut results: Vec<Option<Result<BucketSignatures, WalkFailure>>> =
            (0..slices.len()).map(|_| None).collect();

        {
            let ast: &Ast = &compiler.ast;
            let coding_convention = compiler.get_coding_convention();
            let extern_properties_ref = &*self.extern_properties;
            let worker_batches = crate::parallel_parse::run_scoped_indexed(
                "closure-ism",
                threads,
                &slices,
                |_idx, slice| {
                    let mut gatherer =
                        BucketGatherer::new(ast, coding_convention, extern_properties_ref);
                    let res = catch_unwind(AssertUnwindSafe(|| {
                        for &(unit, parent) in *slice {
                            if let Some(p) = parent
                                && p.is_script(ast)
                            {
                                gatherer.current_script = Some(p);
                            }
                            gatherer.walk_node(unit, parent);
                        }
                    }));
                    let out = match res {
                        Ok(()) => Ok((
                            gatherer.non_inlineable_properties,
                            gatherer.method_definitions,
                        )),
                        Err(payload) => Err(WalkFailure {
                            script: gatherer.current_script,
                            node: gatherer.current_node,
                            payload,
                        }),
                    };
                    Some(out)
                },
            );
            for batch in worker_batches {
                for (idx, out) in batch {
                    results[idx] = Some(out);
                }
            }
        }

        for slot in results {
            match slot.expect("every bucket processed") {
                Ok((non_inlineable, method_defs)) => {
                    self.non_inlineable_properties.extend(non_inlineable);
                    for (name, defs) in method_defs {
                        if !self.is_non_inlineable(&name) {
                            match self.method_definitions.entry(name) {
                                indexmap::map::Entry::Vacant(e) => {
                                    e.insert((defs, 0));
                                }
                                indexmap::map::Entry::Occupied(mut e) => {
                                    e.get_mut().0.extend(defs);
                                }
                            }
                        }
                    }
                }
                Err(fail) => {
                    NodeTraversal::throw_unexpected_exception_at(
                        compiler,
                        fail.script,
                        fail.node,
                        fail.payload,
                    );
                }
            }
        }
    }
}

/// For each method call, see if it is a candidate for inlining, and do the inlining if so.
struct InlineTrivialAccessors<'a> {
    outer: &'a mut InlineSimpleMethods,
    name_bloom: [u64; 64],
    current_script: Option<NodeId>,
    current_node: Option<NodeId>,
}

impl InlineTrivialAccessors<'_> {
    fn walk_node(&mut self, compiler: &mut AbstractCompiler, n: NodeId, parent: Option<NodeId>) {
        let token = n.get_token(compiler);
        match token {
            Token::SCRIPT => {
                let prev = self.current_script;
                self.current_script = Some(n);
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    let next = c.get_next(compiler);
                    self.walk_node(compiler, c, Some(n));
                    child = next;
                }
                self.current_script = prev;
                return;
            }
            Token::CLASS => {
                let class_name = n.get_first_child(compiler).unwrap();
                let extends_clause = class_name.get_next(compiler).unwrap();
                let body = extends_clause.get_next(compiler).unwrap();
                self.walk_node(compiler, extends_clause, Some(n));
                self.walk_node(compiler, class_name, Some(n));
                self.walk_node(compiler, body, Some(n));
                return;
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    let next = c.get_next(compiler);
                    if c.is_computed_prop(compiler) || c.is_computed_field_def(compiler) {
                        self.walk_node(compiler, c.get_first_child(compiler).unwrap(), Some(c));
                    }
                    child = next;
                }
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    let next = c.get_next(compiler);
                    match c.get_token(compiler) {
                        Token::COMPUTED_PROP => {
                            self.walk_node(compiler, c.get_last_child(compiler).unwrap(), Some(c));
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(compiler) {
                                self.walk_node(
                                    compiler,
                                    c.get_last_child(compiler).unwrap(),
                                    Some(c),
                                );
                            }
                        }
                        _ => self.walk_node(compiler, c, Some(n)),
                    }
                    child = next;
                }
                return;
            }
            _ => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    let next = c.get_next(compiler);
                    self.walk_node(compiler, c, Some(n));
                    child = next;
                }
            }
        }
        if token == Token::CALL {
            self.visit_call(compiler, n, parent);
        }
    }

    // port: InlineSimpleMethods.InlineTrivialAccessors#visit
    #[inline]
    fn visit_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        parent: Option<NodeId>,
    ) {
        let callee = call_node.get_first_child(compiler).unwrap();
        if !callee.is_get_prop(compiler) {
            return;
        }
        let call_name_ref = callee.get_string_ref(compiler);
        let h = call_name_ref.hash_code() as usize;
        if (self.name_bloom[(h >> 6) & 63] & (1u64 << (h & 63))) == 0 {
            return;
        }
        if !self.outer.method_definitions.contains_key(call_name_ref) {
            return;
        }
        self.current_node = Some(call_node);
        let call_name = call_name_ref.clone();
        let outer = &mut *self.outer;
        if outer.is_non_inlineable(&call_name) {
            return;
        }
        let Some((definitions, verified_epoch)) = outer.method_definitions.get(&call_name) else {
            return;
        };
        if definitions.is_empty() {
            return;
        }
        let first_definition = *definitions.iter().next().unwrap();
        let epoch_match = *verified_epoch == outer.mutation_epoch;
        if !epoch_match {
            // Exit early if all method definitions are not equivalent, and mark this method as not
            // inlineable.
            let not_equiv =
                definitions.len() > 1 && !outer.all_definitions_equivalent(compiler, definitions);
            let not_inlineable =
                not_equiv || outer.any_definitions_not_inlineable(compiler, definitions);
            if not_inlineable {
                outer.method_definitions.swap_remove(&call_name);
                outer.non_inlineable_properties.insert(call_name);
                return;
            }
            outer.method_definitions.get_mut(&call_name).unwrap().1 = outer.mutation_epoch;
        }

        // Fast check of the definition's body shape before checking call arguments.
        let returned = InlineSimpleMethods::returned_expression(compiler, first_definition);
        let is_prop_return = returned.is_some_and(|r| {
            InlineSimpleMethods::is_property_tree(compiler, r)
                && !first_definition.is_arrow_function(compiler)
        });
        let is_const_return = !is_prop_return
            && returned.is_some_and(|r| NodeUtil::is_literal_value(compiler, r, false));
        let is_empty_return =
            returned.is_none() && InlineSimpleMethods::is_empty_method(compiler, first_definition);
        if !is_prop_return && !is_const_return && !is_empty_return {
            return;
        }

        // Do not inline if the callsite is a derived class calling a base method using `super.`
        let receiver = check_not_null!(
            check_not_null!(call_node.get_first_child(compiler)).get_first_child(compiler)
        );
        if !outer.args_may_have_side_effects(compiler, call_node) && !receiver.is_super(compiler) {
            if is_prop_return {
                outer.inline_property_return(compiler, call_node, returned.unwrap());
            } else if is_const_return {
                let callee = call_node.get_first_child(compiler).unwrap();
                if !outer.ast_analyzer.may_have_side_effects(compiler, callee) {
                    outer.inline_const_return(compiler, call_node, returned.unwrap());
                }
            } else if is_empty_return {
                let callee = call_node.get_first_child(compiler).unwrap();
                if !outer.ast_analyzer.may_have_side_effects(compiler, callee) {
                    outer.inline_empty_method(compiler, parent.unwrap(), call_node);
                }
            }
        }
    }
}
