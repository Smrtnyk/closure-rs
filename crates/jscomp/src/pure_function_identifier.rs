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
//   src/com/google/javascript/jscomp/PureFunctionIdentifier.java.

//! Port of `PureFunctionIdentifier.java`: computes function purity and annotates invocation nodes
//! with those purities.
//!
//! A function is pure if it has no outside visible side effects, and the result of the
//! computation does not depend on external factors that are beyond the control of the
//! application; repeated calls to the function should return the same value as long as global
//! state hasn't changed.
//!
//! Functions are not tracked individually but rather in aggregate by their name. If *any*
//! function "foo" has a particular side-effect, *all* invocations "foo" are assumed to trigger
//! it.

use crate::abstract_compiler::AbstractCompiler;
use crate::accessor_summary::PropertyAccessKind;
use crate::ast_analyzer::{AstAnalyzer, AstAnalyzerContext};
use crate::check_level::CheckLevel;
use crate::coding_convention::Cache;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::graph::di_graph::DiGraphNode;
use crate::graph::fixed_point_graph_traversal::FixedPointGraphTraversal;
use crate::graph::graph::Graph;
use crate::graph::graphviz_graph::GraphvizValue;
use crate::graph::linked_directed_graph::LinkedDirectedGraph;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::optimize_calls::{CallGraphCompilerPass, OptimizeCalls, ReferenceMap};
use crate::scope::ScopeId;
use crate::var::VarId;
use crate::warnings_guard::WarningsGuard;
use closure_rhino::ir::IR;
use closure_rhino::java_lang::hash_map as java_hash_map;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId, SideEffectFlags};
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use indexmap::{IndexMap, IndexSet};
use std::cmp::Ordering;
use std::fmt;

// port: PureFunctionIdentifier#UNUSED_ARTIFICIAL_PURE_ANNOTATION
// Enable this error if you are debugging why a `@nosideeffects` annotation seems to be ignored.
pub static UNUSED_ARTIFICIAL_PURE_ANNOTATION: DiagnosticType = DiagnosticType::disabled(
    "JSC_UNUSED_ARTIFICIAL_PURE_ANNOTATION",
    "Artificial @nosideeffects annotation cannot be enforced: found ambiguous definitions of {0}.\n\nSide-effectful definitions:\n  {1}",
);

/// Handle of an [`AmbiguatedFunctionSummary`] in `PureFunctionIdentifier::summaries`. Java keys
/// the reverse call graph and its maps by summary object identity; the handle is that identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SummaryId(usize);

impl fmt::Display for SummaryId {
    // Java's `AmbiguatedFunctionSummary#toString` is `@DoNotCall` (debugging only); the graph
    // only needs a value-to-string function, which this pass never invokes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AmbiguatedFunctionSummary#{}", self.0)
    }
}

/// Java's `Predicate<Node>` over an LHS node.
type Predicate = fn(&Ast, NodeId) -> bool;

pub struct PureFunctionIdentifier {
    ast_analyzer: AstAnalyzer,
    validate_artificial_purity: bool,

    /// Arena of every summary of this instance, indexed by [`SummaryId`].
    summaries: Vec<AmbiguatedFunctionSummary>,

    /// Map of function names to the summary of the functions with that name.
    ///
    /// Variable names are recorded as-is. Property names are prefixed with
    /// `PROP_NAME_PREFIX` to differentiate them from variable names.
    summaries_by_name: IndexMap<JsString, SummaryId>,

    /// Mapping from function node to summaries for all names associated with that node (Java's
    /// `ArrayListMultimap`).
    ///
    /// This is a multimap because you can construct situations in which a function node has
    /// multiple names, and therefore multiple associated summaries.
    summaries_for_all_names_of_function_by_node: IndexMap<NodeId, Vec<SummaryId>>,

    /// Set of FUNCTION nodes which are artificially marked as pure, via a `@nosideeffects` JSDoc
    /// tag in source code, which triggers the compiler to ignore any actual side effects.
    artificially_pure_literals_for_debugging: IndexSet<NodeId>,

    // List of all function call sites. Storing them here during the function analysis traversal
    // prevents us from doing a second traversal to annotate them with side-effects. We can just
    // iterate the list.
    all_function_calls: Vec<NodeId>,

    /// A graph linking the summary of a function callee to the summaries of its callers.
    ///
    /// Each node represents an aggregate summary of every function with a particular name. The
    /// edge values indicate the details of the invocation necessary to propagate function
    /// impurity from callee to caller.
    reverse_call_graph: LinkedDirectedGraph<SummaryId, SideEffectPropagation>,

    /// A summary for a function for which no definition was found.
    ///
    /// We assume it has all possible side-effects. It's useful for references like function
    /// parameters, or inner functions.
    unknown_function_summary: SummaryId,

    /// A function node representing a function implicit in the AST that is known to be pure.
    ///
    /// For example: `class C {}` would get this node. Java's `IMPLICIT_PURE_FN` is one static
    /// detached node; nodes live in a compiler's arena here, so each instance creates its own
    /// detached node there. Only its identity is used.
    implicit_pure_fn: NodeId,

    assume_getters_are_pure: bool,

    has_processed: bool,
}

impl PureFunctionIdentifier {
    // A prefix to differentiate property names from variable names.
    // TODO(nickreid): This pass could be made more efficient if props and variables were
    // maintained in separate datastructures. We wouldn't allocate a bunch of extra strings.
    const PROP_NAME_PREFIX: &'static str = ".";

    /// `new HashMap<>(Maps.capacity(12))` behind Guava's `ArrayListMultimap.create()`.
    const ARRAY_LIST_MULTIMAP_INITIAL_CAPACITY: i32 = 16;

    // port: PureFunctionIdentifier#RHS_IS_ALWAYS_LOCAL
    const RHS_IS_ALWAYS_LOCAL: Predicate = |_, _lhs| true;
    // port: PureFunctionIdentifier#RHS_IS_NEVER_LOCAL
    const RHS_IS_NEVER_LOCAL: Predicate = |_, _lhs| false;
    // port: PureFunctionIdentifier#FIND_RHS_AND_CHECK_FOR_LOCAL_VALUE
    const FIND_RHS_AND_CHECK_FOR_LOCAL_VALUE: Predicate = |ast, lhs| {
        let rhs = NodeUtil::get_r_value_of_l_value(ast, lhs);
        rhs.is_none_or(|rhs| NodeUtil::evaluates_to_local_value(ast, rhs))
    };

    // port: PureFunctionIdentifier#PureFunctionIdentifier
    pub fn new(
        compiler: &mut AbstractCompiler,
        assume_getters_are_pure: bool,
        validate_artificial_purity: bool,
    ) -> Self {
        let mut summaries = Vec::new();
        let mut reverse_call_graph = LinkedDirectedGraph::create_without_annotations();
        let unknown_function_summary = AmbiguatedFunctionSummary::create_in_graph(
            &mut summaries,
            &mut reverse_call_graph,
            JsString::from("<unknown>"),
        );
        summaries[unknown_function_summary.0]
            .set_mutates_global_state_and_all_other_flags(compiler, None);
        // port: PureFunctionIdentifier#IMPLICIT_PURE_FN
        let implicit_pure_fn = {
            let name = IR::name(compiler, "");
            let params = IR::param_list(compiler, &[]);
            let body = IR::block(compiler);
            IR::function(compiler, name, params, body)
        };
        Self {
            ast_analyzer: compiler.get_ast_analyzer(),
            validate_artificial_purity,
            summaries,
            summaries_by_name: IndexMap::new(),
            summaries_for_all_names_of_function_by_node: IndexMap::new(),
            artificially_pure_literals_for_debugging: IndexSet::new(),
            all_function_calls: Vec::new(),
            reverse_call_graph,
            unknown_function_summary,
            implicit_pure_fn,
            assume_getters_are_pure,
            has_processed: false,
        }
    }

    /// Java's `summariesForAllNamesOfFunctionByNode.get(node)`: an empty collection when absent.
    fn summaries_for_function(&self, function: NodeId) -> Vec<SummaryId> {
        self.summaries_for_all_names_of_function_by_node
            .get(&function)
            .cloned()
            .unwrap_or_default()
    }

    /// Java's `summariesForAllNamesOfFunctionByNode.put(node, summary)`.
    fn put_summary_for_function(&mut self, function: NodeId, summary: SummaryId) {
        self.summaries_for_all_names_of_function_by_node
            .entry(function)
            .or_default()
            .push(summary);
    }

    /// Check whether any function literals marked as artificially pure still have an overall
    /// AmbiguatedFunctionSummary with some side-effects, for debugging purposes.
    ///
    /// This can happen when there are multiple definitions of the same function name, and one is
    /// marked as artificially pure but another is side-effect-full.
    // port: PureFunctionIdentifier#validateArtificialPurity
    fn validate_artificial_purity(&self, compiler: &mut AbstractCompiler) {
        if !self.validate_artificial_purity {
            // Avoid unnecessary computation.
            return;
        }
        for &function in &self.artificially_pure_literals_for_debugging {
            for summary in self.summaries_for_function(function) {
                let summary = &self.summaries[summary.0];
                if summary.has_no_flags_set() {
                    // no flags set == the function is successfully treated as pure.
                    continue;
                }
                if summary.name == ".constructor" {
                    // .constructor is a special case - almost all the time, constructors get
                    // invoked by `new Foo();` instead of `new something.constructor();` and
                    // JSCompiler doesn't support this.constructor + property renaming well. So
                    // don't report this error, as it's not particularly helpful.
                    continue;
                }
                // Java streams the lazily initialized set, which is null when no reason was
                // collected (NullPointerException).
                let mut reasons: Vec<Option<NodeId>> =
                    check_not_null!(summary.impure_function_reasons_for_debugging.as_ref())
                        .iter()
                        .copied()
                        .collect();
                reasons.sort_by(|a, b| {
                    Self::source_location_comparator(
                        compiler,
                        check_not_null!(*a),
                        check_not_null!(*b),
                    )
                });
                let side_effectful_definitions = reasons
                    .iter()
                    .map(|node| Self::format_source_location(compiler, check_not_null!(*node)))
                    .collect::<Vec<_>>()
                    .join("\n  ");
                let name = summary.name.to_string_lossy();
                let error = JSError::make(
                    compiler,
                    function,
                    &UNUSED_ARTIFICIAL_PURE_ANNOTATION,
                    &[&name, &side_effectful_definitions],
                );
                compiler.report(error);
            }
        }
    }

    // port: PureFunctionIdentifier#SOURCE_LOCATION_COMPARATOR
    fn source_location_comparator(ast: &Ast, a: NodeId, b: NodeId) -> Ordering {
        // Comparator.comparing(Node::getSourceFileName): natural String order; a null key
        // throws.
        let a_name = JsString::from(check_not_null!(a.get_source_file_name(ast)));
        let b_name = JsString::from(check_not_null!(b.get_source_file_name(ast)));
        a_name
            .compare_to(&b_name)
            .cmp(&0)
            .then_with(|| a.get_lineno(ast).cmp(&b.get_lineno(ast)))
            .then_with(|| a.get_charno(ast).cmp(&b.get_charno(ast)))
    }

    // port: PureFunctionIdentifier#formatSourceLocation
    fn format_source_location(ast: &Ast, node: NodeId) -> String {
        format!(
            "{}:{}:{}",
            node.get_source_file_name(ast)
                .unwrap_or_else(|| "null".to_string()),
            node.get_lineno(ast),
            node.get_charno(ast)
        )
    }

    /// Traverses an `expr` to collect nodes representing potential callables that it may resolve
    /// to well known callables.
    ///
    /// Returns the discovered callables, or `None` if an unexpected possible value was found.
    /// `implicit_pure_fn` is Java's static `IMPLICIT_PURE_FN`.
    // port: PureFunctionIdentifier#collectCallableLeaves
    fn collect_callable_leaves<C: AstAnalyzerContext + ?Sized>(
        cx: &mut C,
        expr: NodeId,
        ast_analyzer: &AstAnalyzer,
        implicit_pure_fn: NodeId,
    ) -> Option<Vec<NodeId>> {
        let mut callables = Vec::new();
        let all_legal = Self::collect_callable_leaves_internal(
            cx,
            expr,
            &mut callables,
            ast_analyzer,
            implicit_pure_fn,
        );
        if all_legal { Some(callables) } else { None }
    }

    /// Traverses an `expr` to collect nodes representing potential callables that it may resolve
    /// to well known callables.
    ///
    /// For example:
    ///
    /// ```text
    ///   `a.c || b` => [a.c, b]
    ///   `x ? a.c : b` => [a.c, b]
    ///   `(function f() { }) && x || class Foo { constructor() { } }` => [function, x, constructor]`
    /// ```
    ///
    /// If a node that isn't understood is detected, false is returned and the caller is expected
    /// to invalidate the entire collection. If true is returned, this method is guaranteed to have
    /// added at least one result to the collection.
    // port: PureFunctionIdentifier#collectCallableLeavesInternal
    fn collect_callable_leaves_internal<C: AstAnalyzerContext + ?Sized>(
        cx: &mut C,
        expr: NodeId,
        results: &mut Vec<NodeId>,
        ast_analyzer: &AstAnalyzer,
        implicit_pure_fn: NodeId,
    ) -> bool {
        match expr.get_token(cx.get_ast()) {
            Token::FUNCTION | Token::GETPROP | Token::OPTCHAIN_GETPROP | Token::NAME => {
                results.push(expr);
                true
            }

            Token::SUPER => {
                // Pretend that `super` is an alias for the superclass reference.
                let ast = cx.get_ast();
                let clazz = check_not_null!(NodeUtil::get_enclosing_class(ast, expr));
                let superclass = clazz.get_second_child(ast).unwrap();
                Self::collect_callable_leaves_internal(
                    cx,
                    superclass,
                    results,
                    ast_analyzer,
                    implicit_pure_fn,
                )
            }

            Token::CLASS => {
                // Instance field initializers run when the constructor is invoked (during class
                // instantiation), so any side-effectful instance field initializers make the
                // constructor invocation impure. Static field initializers run when the class
                // definition itself is evaluated, not upon constructor invocation.
                let members = expr.get_last_child(cx.get_ast());
                if let Some(members) = members
                    && members.is_class_members(cx.get_ast())
                {
                    let mut member = members.get_first_child(cx.get_ast());
                    while let Some(m) = member {
                        let ast = cx.get_ast();
                        if !m.is_static_member(ast) {
                            let mut initializer = None;
                            if m.is_member_field_def(ast) && m.has_children(ast) {
                                initializer = m.get_first_child(ast);
                            } else if m.is_computed_field_def(ast)
                                && m.get_second_child(ast).is_some()
                            {
                                initializer = m.get_second_child(ast);
                            }
                            if let Some(initializer) = initializer
                                && ast_analyzer.may_have_side_effects(cx, initializer)
                            {
                                return false;
                            }
                        }
                        member = m.get_next(cx.get_ast());
                    }
                }

                // Collect the constructor function, or failing that, the superclass reference.
                let ast = cx.get_ast();
                let ctor_def = NodeUtil::get_es6_class_constructor_member_function_def(ast, expr);
                if let Some(ctor_def) = ctor_def {
                    let ctor = ctor_def.get_only_child(ast);
                    Self::collect_callable_leaves_internal(
                        cx,
                        ctor,
                        results,
                        ast_analyzer,
                        implicit_pure_fn,
                    )
                } else if expr.get_second_child(ast).unwrap().is_empty(ast) {
                    results.push(implicit_pure_fn);
                    true // A class an implicit ctor is pure when there is no superclass.
                } else {
                    let superclass = expr.get_second_child(ast).unwrap();
                    Self::collect_callable_leaves_internal(
                        cx,
                        superclass,
                        results,
                        ast_analyzer,
                        implicit_pure_fn,
                    )
                }
            }

            Token::AND | Token::OR | Token::COALESCE => {
                let first = expr.get_first_child(cx.get_ast()).unwrap();
                Self::collect_callable_leaves_internal(
                    cx,
                    first,
                    results,
                    ast_analyzer,
                    implicit_pure_fn,
                ) && {
                    let second = expr.get_second_child(cx.get_ast()).unwrap();
                    Self::collect_callable_leaves_internal(
                        cx,
                        second,
                        results,
                        ast_analyzer,
                        implicit_pure_fn,
                    )
                }
            }

            Token::COMMA | Token::ASSIGN => {
                let second = expr.get_second_child(cx.get_ast()).unwrap();
                Self::collect_callable_leaves_internal(
                    cx,
                    second,
                    results,
                    ast_analyzer,
                    implicit_pure_fn,
                )
            }

            Token::HOOK => {
                let second = expr.get_second_child(cx.get_ast()).unwrap();
                Self::collect_callable_leaves_internal(
                    cx,
                    second,
                    results,
                    ast_analyzer,
                    implicit_pure_fn,
                ) && {
                    let third = expr.get_child_at_index(cx.get_ast(), 2).unwrap();
                    Self::collect_callable_leaves_internal(
                        cx,
                        third,
                        results,
                        ast_analyzer,
                        implicit_pure_fn,
                    )
                }
            }

            // These could be an alias to any function. Treat them as an unknown callable.
            Token::NEW_TARGET | Token::THIS => false,
            _ => false, // Unsupported call type.
        }
    }

    /// Return `true` only if `rvalue` is definitely a reference reading a value.
    ///
    /// For the most part it's sufficient to cover cases where a nominal function reference might
    /// reasonably be expected, since those are the values that matter to analysis.
    ///
    /// It's very important that this never returns `true` for an L-value, including when new
    /// syntax is added to the language. That would cause some impure functions to be considered
    /// pure. Therefore, this method explicitly lists the accepted parents. Anything that's
    /// unrecognized is considered not an R-value. This is insurance against new syntax.
    // port: PureFunctionIdentifier#isDefinitelyRValue
    fn is_definitely_r_value(ast: &Ast, rvalue: NodeId) -> bool {
        let parent = rvalue.get_parent(ast).unwrap();

        match parent.get_token(ast) {
            Token::AND
            | Token::ARRAYLIT
            | Token::CALL
            | Token::COALESCE
            | Token::COMMA
            | Token::EQ
            | Token::GETELEM
            | Token::GETPROP
            | Token::HOOK
            | Token::INSTANCEOF
            | Token::NEW
            | Token::NOT
            | Token::NAME
            | Token::OPTCHAIN_CALL
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_GETPROP
            | Token::OR
            | Token::RETURN
            | Token::SHEQ
            | Token::TAGGED_TEMPLATELIT
            | Token::TYPEOF
            | Token::YIELD => true,
            Token::CASE | Token::IF | Token::SWITCH | Token::WHILE => {
                rvalue.is_first_child_of(ast, Some(parent)) // the condition is always an r-value
            }
            Token::EXPR_RESULT => {
                // Extern declarations are sometimes stubs. These must be considered L-values with
                // no associated R-values.
                !rvalue.is_from_externs(ast)
            }
            // `extends` clause.
            Token::ASSIGN | Token::CLASS => rvalue.is_second_child_of(ast, Some(parent)),
            Token::STRING_KEY => {
                // Assignment to an object literal property. Excludes object destructuring.
                parent.get_parent(ast).unwrap().is_object_lit(ast)
            }
            _ => {
                // Anything not explicitly listed may not be an R-value. We only worry about the
                // likely cases for nominal function values since those are what interest us and
                // its safe to miss some R-values. It's more important that we correctly identify
                // L-values.
                false
            }
        }
    }

    // port: PureFunctionIdentifier#getGoogCacheCallableExpression
    fn get_goog_cache_callable_expression(
        &self,
        compiler: &mut AbstractCompiler,
        cache_call: Cache,
    ) -> Vec<NodeId> {
        // ImmutableList.Builder#addAll(null) throws NullPointerException.
        let mut builder = Vec::new();
        builder.extend(check_not_null!(Self::collect_callable_leaves(
            compiler,
            cache_call.value_fn,
            &self.ast_analyzer,
            self.implicit_pure_fn,
        )));
        if let Some(key_fn) = cache_call.key_fn {
            builder.extend(check_not_null!(Self::collect_callable_leaves(
                compiler,
                key_fn,
                &self.ast_analyzer,
                self.implicit_pure_fn,
            )));
        }
        builder
    }

    // port: PureFunctionIdentifier#getSummariesForCallee
    fn get_summaries_for_callee(
        &self,
        compiler: &mut AbstractCompiler,
        invocation: NodeId,
    ) -> Vec<SummaryId> {
        check_argument!(
            NodeUtil::is_invocation(compiler, invocation),
            "%s",
            invocation.to_string(compiler)
        );

        let cache_call = compiler
            .get_coding_convention()
            .describe_caching_call(compiler, invocation);

        let callees: Option<Vec<NodeId>> = if let Some(cache_call) = cache_call {
            Some(self.get_goog_cache_callable_expression(compiler, cache_call))
        } else if Self::is_invocation_via_call_or_apply(compiler, invocation) {
            Some(vec![invocation.get_first_first_child(compiler).unwrap()])
        } else {
            Self::collect_callable_leaves(
                compiler,
                invocation.get_first_child(compiler).unwrap(),
                &self.ast_analyzer,
                self.implicit_pure_fn,
            )
        };

        let Some(callees) = callees else {
            return vec![self.unknown_function_summary];
        };

        check_state!(
            !callees.is_empty(),
            "Unexpected empty callees for valid result"
        );
        let mut results = Vec::new();
        for callee in callees {
            if callee.is_function(compiler) {
                check_state!(
                    callee.is_function(compiler),
                    "%s",
                    callee.to_string(compiler)
                );

                let summaries_for_function = self.summaries_for_function(callee);
                check_state!(
                    !summaries_for_function.is_empty(),
                    "Function missed during analysis: %s",
                    callee.to_string(compiler)
                );

                results.extend(summaries_for_function);
            } else {
                let callee_name = Self::name_for_reference(compiler, callee);
                results.push(
                    self.summaries_by_name
                        .get(&callee_name)
                        .copied()
                        .unwrap_or(self.unknown_function_summary),
                );
            }
        }
        results
    }

    /// Fill all of the auxiliary data-structures used by this pass based on the results in
    /// `reference_map`.
    ///
    /// This is the first step of analysis. These structures will be used by a traversal that
    /// analyzes the bodies of located functions for side-effects. That traversal is separate
    /// because it needs access to scopes and also depends on global knowledge of functions.
    // port: PureFunctionIdentifier#populateDatastructuresForAnalysisTraversal
    fn populate_datastructures_for_analysis_traversal(
        &mut self,
        compiler: &mut AbstractCompiler,
        reference_map: &ReferenceMap,
    ) {
        // Merge the prop and name references into a single multimap since only the name matters.
        // (Java's ArrayListMultimap: `putAll` of an empty list adds no key.)
        let mut references_by_name: IndexMap<JsString, Vec<NodeId>> = IndexMap::new();
        for (key, value) in reference_map.get_name_references() {
            if !value.is_empty() {
                references_by_name
                    .entry(key.clone())
                    .or_default()
                    .extend(value.iter().copied());
            }
        }
        for (key, value) in reference_map.get_prop_references() {
            if !value.is_empty() {
                references_by_name
                    .entry(JsString::from(Self::PROP_NAME_PREFIX).concat(key))
                    .or_default()
                    .extend(value.iter().copied());
            }
        }
        // ArrayListMultimap.create() is backed by a HashMap (Guava's expected 12 keys: initial
        // capacity 16); its keySet() and asMap() iterate in Java's HashMap order.
        let references_by_name: IndexMap<JsString, Vec<NodeId>> = java_hash_map::iteration_order(
            references_by_name.into_iter().collect(),
            |(name, _)| name.hash_code(),
            Self::ARRAY_LIST_MULTIMAP_INITIAL_CAPACITY,
        )
        .into_iter()
        .collect();
        // Empty function names cause a crash during analysis that is better to detect here.
        // Additionally, functions require a name to be invoked in a statically analyzable way;
        // there's no value in tracking the set of anonymous functions.
        check_state!(!references_by_name.contains_key(&JsString::from("")));
        check_state!(!references_by_name.contains_key(&JsString::from(Self::PROP_NAME_PREFIX)));

        // Create and store a summary for all known names.
        for name in references_by_name.keys() {
            let summary = AmbiguatedFunctionSummary::create_in_graph(
                &mut self.summaries,
                &mut self.reverse_call_graph,
                name.clone(),
            );
            self.summaries_by_name.insert(name.clone(), summary);
        }

        for (name, references) in &references_by_name {
            self.populate_function_definitions(compiler, name, references);
        }
        let implicit = AmbiguatedFunctionSummary::create_in_graph(
            &mut self.summaries,
            &mut self.reverse_call_graph,
            JsString::from("<implicit pure class ctor"),
        );
        self.put_summary_for_function(self.implicit_pure_fn, implicit);
    }

    /// For a name and its set of references, record the set of functions that may define that
    /// name or skiplist the name if there are unclear definitions.
    // port: PureFunctionIdentifier#populateFunctionDefinitions
    fn populate_function_definitions(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        references: &[NodeId],
    ) {
        let summary_for_name = *check_not_null!(self.summaries_by_name.get(name));

        // Make sure we get absolutely every R-value assigned to `name` or at the very least detect
        // there are some we're missing. Overlooking a single R-value would invalidate the
        // analysis.

        let mut invalid = false;
        let mut rvalues_assigned_to_name: Vec<Vec<NodeId>> = Vec::new();
        for &reference in references {
            // Eliminate any references that we're sure are R-values themselves. Otherwise
            // there's a high probability we'll inspect an R-value for futher R-values. We wouldn't
            // find any, and then we'd have to consider `name` impure.
            if !Self::is_definitely_r_value(compiler, reference) {
                // For anything that might be an L-reference, get the expression being assigned to
                // it.
                let rvalue = NodeUtil::get_r_value_of_l_value(compiler, reference);
                let Some(rvalue) = rvalue else {
                    invalid = true;
                    break;
                };
                // If the assigned R-value is an analyzable expression, collect all the possible
                // FUNCTIONs that could result from that expression. If the expression isn't
                // analyzable, represent that with `null` so we can skiplist `name`.
                let callables = Self::collect_callable_leaves(
                    compiler,
                    rvalue,
                    &self.ast_analyzer,
                    self.implicit_pure_fn,
                );
                let Some(callables) = callables else {
                    invalid = true;
                    break;
                };
                if self.validate_artificial_purity && !reference.is_from_externs(compiler) {
                    for &callable in &callables {
                        let jsdoc = NodeUtil::get_best_jsdoc_info(compiler, callable);
                        let is_artificially_pure =
                            jsdoc.is_some_and(|jsdoc| jsdoc.is_no_side_effects());
                        if is_artificially_pure {
                            self.artificially_pure_literals_for_debugging
                                .insert(callable);
                            self.summaries[summary_for_name.0]
                                .set_collect_impure_debugging_reason();
                        }
                    }
                }

                rvalues_assigned_to_name.push(callables);
            }
        }

        if rvalues_assigned_to_name.is_empty() || invalid {
            // Any of:
            // - There are no L-values with this name.
            // - There's a an L-value and we can't find the associated R-values.
            // - There's a an L-value with R-values are not all known to be callable.
            self.summaries[summary_for_name.0]
                .set_mutates_global_state_and_all_other_flags(compiler, None);
        } else {
            for callables in rvalues_assigned_to_name {
                for rvalue in callables {
                    if rvalue.is_function(compiler) {
                        self.put_summary_for_function(rvalue, summary_for_name);
                    } else if NodeUtil::is_undefined(compiler, rvalue) {
                    } else {
                        let rvalue_name = Self::name_for_reference(compiler, rvalue);
                        let rvalue_summary = self
                            .summaries_by_name
                            .get(&rvalue_name)
                            .copied()
                            .unwrap_or(self.unknown_function_summary);

                        self.reverse_call_graph.connect_nodes(
                            self.summaries[rvalue_summary.0].graph_node,
                            SideEffectPropagation::for_alias(),
                            self.summaries[summary_for_name.0].graph_node,
                        );
                    }
                }
            }
        }
    }

    /// Propagate side effect information in `reverse_call_graph` from callees to callers.
    ///
    /// This is an iterative process executed until a fixed point, where no caller summary would
    /// be given new side-effects from from any callee summary, is reached.
    // port: PureFunctionIdentifier#propagateSideEffects
    fn propagate_side_effects(&mut self, ast: &Ast) {
        let summaries = &mut self.summaries;
        FixedPointGraphTraversal::new_traversal(
            |_graph: &mut LinkedDirectedGraph<SummaryId, SideEffectPropagation>,
             source: SummaryId,
             edge: SideEffectPropagation,
             destination: SummaryId| {
                edge.propagate(ast, summaries, source, destination)
            },
        )
        .compute_fixed_point(&mut self.reverse_call_graph);
    }

    /// Set no side effect property at pure-function call sites.
    // port: PureFunctionIdentifier#markPureFunctionCalls
    fn mark_pure_function_calls(&self, compiler: &mut AbstractCompiler) {
        for &call_node in &self.all_function_calls {
            let callee_summaries = self.get_summaries_for_callee(compiler, call_node);

            // Default to side effects, non-local results
            let mut flags = SideEffectFlags::new();
            if callee_summaries.is_empty() {
                flags.set_all_flags();
            } else {
                flags.clear_all_flags();
                for callee_summary in callee_summaries {
                    let callee_summary = &self.summaries[callee_summary.0];
                    if callee_summary.mutates_global_state() {
                        flags.set_mutates_global_state();
                    }

                    if callee_summary.mutates_arguments() {
                        flags.set_mutates_arguments();
                    }

                    if callee_summary.function_throws() {
                        flags.set_throws();
                    }

                    if Self::is_call_or_tagged_template_lit(compiler, call_node)
                        && callee_summary.mutates_this()
                    {
                        // A summary for "f" maps to both "f()" and "f.call()" nodes.
                        if Self::is_invocation_via_call_or_apply(compiler, call_node) {
                            flags.set_mutates_arguments(); // `this` is actually an argument.
                        } else {
                            flags.set_mutates_this();
                        }
                    }
                }
            }

            if call_node
                .get_first_child(compiler)
                .unwrap()
                .is_super(compiler)
            {
                // All `super()` calls (i.e. from subclass constructors) implicitly mutate `this`;
                // they determine its value in the caller scope. Concretely, `super()` calls must
                // not be removed or reordered. Marking them this way ensures that without pinning
                // the enclosing function.
                flags.set_mutates_this();
            }

            // Handle special cases (Math, RegExp)
            if Self::is_call_or_tagged_template_lit(compiler, call_node) {
                if !self
                    .ast_analyzer
                    .function_call_has_side_effects(compiler, call_node)
                {
                    flags.clear_all_flags();
                }
            } else if call_node.is_new(compiler) {
                // Handle known cases now (Object, Date, RegExp, etc)
                if !self
                    .ast_analyzer
                    .constructor_call_has_side_effects(compiler, call_node)
                {
                    flags.clear_all_flags();
                }
            }

            if call_node.get_side_effect_flags(compiler) != flags.value_of() {
                call_node.set_side_effect_flags_from_flags(compiler, &flags);
                compiler.report_change_to_enclosing_scope(call_node);
            }
        }
    }

    // port: PureFunctionIdentifier#isInvocationViaCallOrApply
    fn is_invocation_via_call_or_apply(ast: &Ast, call_site: NodeId) -> bool {
        let receiver = call_site.get_first_first_child(ast);
        let Some(receiver) = receiver else {
            return false;
        };
        if !(receiver.is_name(ast)
            || receiver.is_get_prop(ast)
            || receiver.is_opt_chain_get_prop(ast))
        {
            return false;
        }

        NodeUtil::is_function_object_call(ast, call_site)
            || NodeUtil::is_function_object_apply(ast, call_site)
    }

    // port: PureFunctionIdentifier#isCallOrTaggedTemplateLit
    fn is_call_or_tagged_template_lit(ast: &Ast, invocation: NodeId) -> bool {
        invocation.is_call(ast)
            || invocation.is_opt_chain_call(ast)
            || invocation.is_tagged_template_lit(ast)
    }

    /// Returns the unqualified name associated with an R-value.
    ///
    /// For NAMEs this is the name. For GETPROPs this is the last segment including a leading dot.
    // port: PureFunctionIdentifier#nameForReference
    fn name_for_reference(ast: &Ast, name_ref: NodeId) -> JsString {
        match name_ref.get_token(ast) {
            Token::NAME => name_ref.get_string(ast),
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                JsString::from(Self::PROP_NAME_PREFIX).concat(&name_ref.get_string(ast))
            }
            _ => panic!("Unexpected name reference: {}", name_ref.to_string(ast)),
        }
    }

    // port: PureFunctionIdentifier#getPropertyKind
    fn get_property_kind(
        &self,
        compiler: &AbstractCompiler,
        name: &JsString,
    ) -> PropertyAccessKind {
        if self.assume_getters_are_pure {
            PropertyAccessKind::NORMAL
        } else {
            check_not_null!(compiler.get_accessor_summary()).get_kind(name)
        }
    }
}

impl CallGraphCompilerPass for PureFunctionIdentifier {
    // port: PureFunctionIdentifier#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
        references: &mut ReferenceMap,
    ) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        check_state!(
            !self.has_processed,
            "PureFunctionIdentifier::process may only be called once per instance."
        );
        self.has_processed = true;

        self.populate_datastructures_for_analysis_traversal(compiler, references);

        NodeTraversal::traverse(
            compiler,
            externs,
            &mut ExternFunctionAnnotationAnalyzer { pfi: self },
        );
        NodeTraversal::traverse(compiler, root, &mut FunctionBodyAnalyzer::new(self));

        self.propagate_side_effects(compiler);

        self.validate_artificial_purity(compiler);

        self.mark_pure_function_calls(compiler);
    }
}

/// Inspects function JSDoc for side effects and applies them to the associated
/// [`AmbiguatedFunctionSummary`].
///
/// This callback is only meant for use on externs.
struct ExternFunctionAnnotationAnalyzer<'a> {
    // The enclosing PureFunctionIdentifier instance (Java inner class).
    pfi: &'a mut PureFunctionIdentifier,
}

impl ExternFunctionAnnotationAnalyzer<'_> {
    // port: PureFunctionIdentifier.ExternFunctionAnnotationAnalyzer#updateSideEffectsForExternFunction
    fn update_side_effects_for_extern_function(
        &mut self,
        ast: &Ast,
        extern_function: NodeId,
        summary: SummaryId,
    ) {
        check_argument!(extern_function.is_function(ast));
        check_argument!(extern_function.is_from_externs(ast));
        let summary = &mut self.pfi.summaries[summary.0];

        let info = NodeUtil::get_best_jsdoc_info(ast, extern_function);
        let Some(info) = info else {
            // We don't know anything about this function so we assume it has side effects.
            summary.set_mutates_global_state_and_all_other_flags(ast, Some(extern_function));
            return;
        };

        if info.modifies_this() {
            summary.set_mutates_this(ast, Some(extern_function));
        }
        if info.has_side_effects_arguments_annotation() {
            summary.set_mutates_arguments(ast, Some(extern_function));
        }
        if !info.get_throws_annotations().is_empty() {
            summary.set_throws(ast, Some(extern_function));
        }

        if !info.is_no_side_effects() && summary.has_no_flags_set() {
            // We don't know anything about this function so we assume it has side effects.
            summary.set_mutates_global_state_and_all_other_flags(ast, Some(extern_function));
        }
    }
}

impl Callback for ExternFunctionAnnotationAnalyzer<'_> {
    // port: PureFunctionIdentifier.ExternFunctionAnnotationAnalyzer#shouldTraverse
    fn should_traverse(
        &mut self,
        _traversal: &mut NodeTraversal<'_>,
        _node: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: PureFunctionIdentifier.ExternFunctionAnnotationAnalyzer#visit
    fn visit(&mut self, traversal: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        if !node.is_function(traversal) {
            return;
        }

        for definition_summary in self.pfi.summaries_for_function(node) {
            self.update_side_effects_for_extern_function(traversal, node, definition_summary);
        }
    }
}

/// A single function literal definition and associated side-effect-related information.
///
/// We define separate objects for function literal to simplify handling of nested function
/// literal definitions - we push a new entry onto the stack when entering a new function
/// literal, and pop the entry when leaving the function literal.
struct FunctionStackEntry {
    root: Option<NodeId>,
    skiplisted_vars: IndexSet<VarId>,
    tainted_vars: IndexSet<VarId>,
    // Whether this function was marked as artificially pure, i.e. had a `@nosideeffects`
    // annotation. This tells the compiler to ignore side effects from within the function body
    // when deciding whether the given function as a whole is pure. Note that within the function
    // body, we still record side effects on individual variables accesses. Also note that this
    // doesn't extend to nested function definitions: a nested function does not inherit
    // artificial purity from its enclosing function, which is why we need to track this on the
    // function stack.
    is_artificially_pure: bool,
    catch_depth: i32, // The number of try-catch blocks around the current node.
}

impl FunctionStackEntry {
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer.FunctionStackEntry#FunctionStackEntry
    fn new(root: Option<NodeId>, is_artificially_pure: bool) -> Self {
        Self {
            root,
            skiplisted_vars: IndexSet::new(),
            tainted_vars: IndexSet::new(),
            is_artificially_pure,
            catch_depth: 0,
        }
    }
}

/// Inspects function bodies for side effects and applies them to the associated
/// [`AmbiguatedFunctionSummary`].
///
/// This callback also fills `all_function_calls`.
struct FunctionBodyAnalyzer<'a> {
    // The enclosing PureFunctionIdentifier instance (Java inner class).
    pfi: &'a mut PureFunctionIdentifier,
    // Preloaded with an entry to represent the global scope.
    function_scope_stack: Vec<FunctionStackEntry>,
}

impl<'a> FunctionBodyAnalyzer<'a> {
    fn new(pfi: &'a mut PureFunctionIdentifier) -> Self {
        Self {
            pfi,
            function_scope_stack: vec![FunctionStackEntry::new(None, false)],
        }
    }

    /// Java's `functionScopeStack.getLast()`.
    fn last_entry(&mut self) -> &mut FunctionStackEntry {
        self.function_scope_stack.last_mut().unwrap()
    }

    fn summary(&mut self, summary: SummaryId) -> &mut AmbiguatedFunctionSummary {
        &mut self.pfi.summaries[summary.0]
    }

    /// Java's `NodeUtil.visitLhsNodesInNode(node, lhsNode -> visitLhsNode(...))`: the LHS nodes
    /// are collected in visiting order (the visitor does not change the AST) and then visited in
    /// that order.
    fn lhs_nodes_in_node(compiler: &mut AbstractCompiler, node: NodeId) -> Vec<NodeId> {
        let mut lhs_nodes = Vec::new();
        NodeUtil::visit_lhs_nodes_in_node(
            compiler,
            node,
            &mut |_: &mut AbstractCompiler, lhs_node: NodeId| lhs_nodes.push(lhs_node),
        );
        lhs_nodes
    }

    /// Updates the side effects of summary based on a given node.
    ///
    /// This node should be known to (possibly have) side effects. This method does not check if
    /// the node (possibly) has side effects.
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#updateSideEffectsForNode
    fn update_side_effects_for_node(
        &mut self,
        encloser_summary: SummaryId,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
    ) {
        if self.summary(encloser_summary).mutates_global_state() {
            return; // Functions with MUTATES_GLOBAL_STATE already have all side-effects set.
        }

        match node.get_token(traversal) {
            Token::ASSIGN => {
                // e.g.
                // lhs = rhs;
                // ({x, y} = object);
                let lhs = node.get_first_child(traversal).unwrap();
                // Consider destructured properties or values to be nonlocal.
                let rhs_locality = if lhs.is_destructuring_pattern(traversal) {
                    PureFunctionIdentifier::RHS_IS_NEVER_LOCAL
                } else {
                    PureFunctionIdentifier::FIND_RHS_AND_CHECK_FOR_LOCAL_VALUE
                };
                for lhs_node in Self::lhs_nodes_in_node(traversal.get_compiler(), node) {
                    let scope = traversal.get_scope();
                    self.visit_lhs_node(
                        traversal.get_compiler(),
                        encloser_summary,
                        scope,
                        lhs_node,
                        rhs_locality,
                    );
                }
            }

            // e.g. x++;
            Token::INC | Token::DEC | Token::DELPROP => {
                let scope = traversal.get_scope();
                let only_child = node.get_only_child(traversal);
                self.visit_lhs_node(
                    traversal.get_compiler(),
                    encloser_summary,
                    scope,
                    only_child,
                    // The value assigned by a unary op is always local.
                    PureFunctionIdentifier::RHS_IS_ALWAYS_LOCAL,
                );
            }

            token @ (Token::FOR_AWAIT_OF | Token::FOR_OF) => {
                if token == Token::FOR_AWAIT_OF {
                    // Control is lost during await.
                    self.deprecated_set_side_effects_for_control_loss(
                        traversal.get_compiler(),
                        encloser_summary,
                    );
                    // Fall through.
                }
                // e.g.
                // for (const {prop1, prop2} of iterable) {...}
                // for ({prop1: x.p1, prop2: x.p2} of iterable) {...}
                for lhs_node in Self::lhs_nodes_in_node(traversal.get_compiler(), node) {
                    let scope = traversal.get_scope();
                    self.visit_lhs_node(
                        traversal.get_compiler(),
                        encloser_summary,
                        scope,
                        lhs_node,
                        // The RHS of a for-of must always be an iterable, making it a container,
                        // so we can't consider its contents to be local
                        PureFunctionIdentifier::RHS_IS_NEVER_LOCAL,
                    );
                }
                self.check_iterates_impure_iterable(
                    traversal.get_compiler(),
                    node,
                    encloser_summary,
                );
            }

            Token::FOR_IN => {
                // e.g.
                // for (prop in obj) {...}
                // Also this, though not very useful or readable.
                // for ([char1, char2, ...x.rest] in obj) {...}
                for lhs_node in Self::lhs_nodes_in_node(traversal.get_compiler(), node) {
                    let scope = traversal.get_scope();
                    self.visit_lhs_node(
                        traversal.get_compiler(),
                        encloser_summary,
                        scope,
                        lhs_node,
                        // A for-in always assigns a string, which is a local value by definition.
                        PureFunctionIdentifier::RHS_IS_ALWAYS_LOCAL,
                    );
                }
            }

            Token::OPTCHAIN_CALL | Token::CALL | Token::NEW | Token::TAGGED_TEMPLATELIT => {
                self.visit_call(traversal.get_compiler(), encloser_summary, node);
            }

            Token::DESTRUCTURING_LHS => {
                let parent = node.get_parent(traversal).unwrap();
                if NodeUtil::is_any_for(traversal, parent) {
                    // This case is handled when visiting the enclosing for loop.
                    return;
                }
                // Assume the value assigned to each item is potentially global state. This is
                // overly conservative but necessary because in the common case the rhs is not a
                // literal.
                for lhs_node in Self::lhs_nodes_in_node(traversal.get_compiler(), parent) {
                    let scope = traversal.get_scope();
                    self.visit_lhs_node(
                        traversal.get_compiler(),
                        encloser_summary,
                        scope,
                        lhs_node,
                        PureFunctionIdentifier::RHS_IS_NEVER_LOCAL,
                    );
                }
            }

            Token::NAME => {
                // Local variable declarations are not a side-effect, but we do want to track them.
                if NodeUtil::is_name_declaration(traversal, node.get_parent(traversal)) {
                    let value = node.get_first_child(traversal);
                    // Assignment to local, if the value isn't a safe local value,
                    // new object creation or literal or known primitive result
                    // value, add it to the local skiplist.
                    if let Some(value) = value
                        && !NodeUtil::evaluates_to_local_value(traversal, value)
                    {
                        let scope = traversal.get_scope();
                        let name = node.get_string(traversal);
                        let var = scope.get_var(traversal.get_compiler(), name);
                        // Java adds the possibly-null var; a null entry never matches a lookup.
                        if let Some(var) = var {
                            self.last_entry().skiplisted_vars.insert(var);
                        }
                    }
                }
            }

            Token::THROW => {
                self.record_throws_based_on_context(traversal.get_compiler(), encloser_summary);
            }

            Token::YIELD => {
                // `yield*` triggers iteration.
                self.check_iterates_impure_iterable(
                    traversal.get_compiler(),
                    node,
                    encloser_summary,
                );
                // 'yield' throws if the caller calls `.throw` on the generator object.
                self.deprecated_set_side_effects_for_control_loss(
                    traversal.get_compiler(),
                    encloser_summary,
                );
            }

            Token::AWAIT => {
                // 'await' throws if the promise it's waiting on is rejected.
                self.deprecated_set_side_effects_for_control_loss(
                    traversal.get_compiler(),
                    encloser_summary,
                );
            }

            Token::OBJECT_REST | Token::OBJECT_SPREAD => {
                if !self.pfi.assume_getters_are_pure {
                    // May trigger a getter.
                    let compiler = traversal.get_compiler();
                    self.summary(encloser_summary)
                        .set_mutates_global_state_and_all_other_flags(compiler, Some(node));
                }
            }

            Token::ITER_REST | Token::ITER_SPREAD => {
                self.check_iterates_impure_iterable(
                    traversal.get_compiler(),
                    node,
                    encloser_summary,
                );
            }

            Token::STRING_KEY => {
                if node
                    .get_parent(traversal)
                    .unwrap()
                    .is_object_pattern(traversal)
                {
                    // This is an l-value STRING_KEY.
                    // Assumption: GETELEM (via a COMPUTED_PROP) is never side-effectful.
                    let compiler = traversal.get_compiler();
                    let name = node.get_string(compiler);
                    if self.pfi.get_property_kind(compiler, &name).has_getter() {
                        self.summary(encloser_summary)
                            .set_mutates_global_state_and_all_other_flags(compiler, Some(node));
                    }
                }
            }

            Token::OPTCHAIN_GETPROP | Token::GETPROP => {
                // Assumption: GETELEM and OPTCHAIN_GETELEM are never side-effectful.
                let compiler = traversal.get_compiler();
                let name = node.get_string(compiler);
                if self
                    .pfi
                    .get_property_kind(compiler, &name)
                    .has_getter_or_setter()
                {
                    self.summary(encloser_summary)
                        .set_mutates_global_state_and_all_other_flags(compiler, Some(node));
                }
            }

            Token::DYNAMIC_IMPORT => {
                // Modules may be imported for side-effects only. This is frequently
                // a pattern used to load polyfills.
                let compiler = traversal.get_compiler();
                self.summary(encloser_summary)
                    .set_mutates_global_state_and_all_other_flags(compiler, Some(node));
            }

            _ => {
                if NodeUtil::is_compound_assignment_op(traversal, node) {
                    // e.g.
                    // x += 3;
                    let scope = traversal.get_scope();
                    let first_child = node.get_first_child(traversal).unwrap();
                    self.visit_lhs_node(
                        traversal.get_compiler(),
                        encloser_summary,
                        scope,
                        first_child,
                        // The update assignments (e.g. `+=) always assign primitive, and
                        // therefore local, values.
                        PureFunctionIdentifier::RHS_IS_ALWAYS_LOCAL,
                    );
                    return;
                }

                let compiler = traversal.get_compiler();
                if compiler
                    .get_ast_analyzer()
                    .node_type_may_have_side_effects(compiler, node)
                {
                    // Java: throw new IllegalArgumentException(...)
                    panic!(
                        "Unhandled side effect node type {}",
                        node.to_string(compiler)
                    );
                }
            }
        }
    }

    /// Inspect `node` for impure iteration and assign the appropriate side-effects to
    /// `encloser_summary` if so.
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#checkIteratesImpureIterable
    fn check_iterates_impure_iterable(
        &mut self,
        ast: &Ast,
        node: NodeId,
        encloser_summary: SummaryId,
    ) {
        if !NodeUtil::iterates_impure_iterable(ast, node) {
            return;
        }
        self.summary(encloser_summary)
            .set_mutates_global_state_and_all_other_flags(ast, Some(node));
    }

    /// Assigns the set of side-effects associated with an arbitrary loss of control flow to
    /// `encloser_summary`.
    ///
    /// This function is kept to retain behaviour but marks places where the analysis is
    /// inaccurate (b/135475880).
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#deprecatedSetSideEffectsForControlLoss
    fn deprecated_set_side_effects_for_control_loss(
        &mut self,
        ast: &Ast,
        encloser_summary: SummaryId,
    ) {
        self.record_throws_based_on_context(ast, encloser_summary);
    }

    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#recordThrowsBasedOnContext
    fn record_throws_based_on_context(&mut self, ast: &Ast, encloser_summary: SummaryId) {
        if self.last_entry().catch_depth == 0 {
            let root = self.last_entry().root;
            self.summary(encloser_summary).set_throws(ast, root);
        }
    }

    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#isVarDeclaredInSameContainerScope
    fn is_var_declared_in_same_container_scope(
        compiler: &AbstractCompiler,
        v: Option<VarId>,
        scope: ScopeId,
    ) -> bool {
        v.is_some_and(|v| {
            v.get_scope(compiler)
                .has_same_container_scope(compiler, scope)
        })
    }

    /// Record information about the side effects caused by assigning a value to a given LHS.
    ///
    /// If the operation modifies this or taints global state, mark the enclosing function as
    /// having those side effects.
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#visitLhsNode
    fn visit_lhs_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        encloser_summary: SummaryId,
        scope: ScopeId,
        lhs: NodeId,
        has_local_rhs: Predicate,
    ) {
        if NodeUtil::is_normal_or_opt_chain_get(compiler, lhs) {
            // Although OPTCHAIN_GETPROP can not be an LHS of an assign, it can be a child to
            // DELPROP. e.g. `delete obj?.prop` <==> `obj == null ?  true : delete obj.prop;`
            // Hence the enclosing function's side effects must be recorded.
            if lhs.get_first_child(compiler).unwrap().is_this(compiler) {
                if self.last_entry().root.unwrap().is_arrow_function(compiler) {
                    // `this` in an arrow function is the `this` of the enclosing function, which
                    // is not the receiver the arrow function is called with.
                    self.summary(encloser_summary)
                        .set_mutates_global_state_and_all_other_flags(compiler, Some(lhs));
                } else {
                    self.summary(encloser_summary)
                        .set_mutates_this(compiler, Some(lhs));
                }
            } else {
                let object_node = lhs.get_first_child(compiler).unwrap();
                if object_node.is_name(compiler) {
                    let name = object_node.get_string(compiler);
                    let var = scope.get_var(compiler, name);
                    if Self::is_var_declared_in_same_container_scope(compiler, var, scope) {
                        // Maybe a local object modification.  We won't know for sure until
                        // we exit the scope and can validate the value of the local.
                        self.last_entry().tainted_vars.insert(var.unwrap());
                    } else {
                        self.summary(encloser_summary)
                            .set_mutates_global_state_and_all_other_flags(compiler, Some(lhs));
                    }
                } else {
                    // Don't track multi level locals: local.prop.prop2++;
                    self.summary(encloser_summary)
                        .set_mutates_global_state_and_all_other_flags(compiler, Some(lhs));
                }
            }
        } else {
            check_state!(lhs.is_name(compiler), "%s", lhs.to_string(compiler));
            let name = lhs.get_string(compiler);
            let var = scope.get_var(compiler, name);
            if Self::is_var_declared_in_same_container_scope(compiler, var, scope) {
                if !has_local_rhs(compiler, lhs) {
                    // Assigned value is not guaranteed to be a local value,
                    // so if we see any property assignments on this variable,
                    // they could be tainting a non-local value.
                    self.last_entry().skiplisted_vars.insert(var.unwrap());
                }
            } else {
                self.summary(encloser_summary)
                    .set_mutates_global_state_and_all_other_flags(compiler, Some(lhs));
            }
        }
    }

    /// Record information about a call site.
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#visitCall
    fn visit_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        caller_info: SummaryId,
        invocation: NodeId,
    ) {
        // Handle special cases (Math, RegExp)
        // TODO: This logic can probably be replaced with @nosideeffects annotations in externs.
        if invocation.is_call(compiler)
            && !self
                .pfi
                .ast_analyzer
                .function_call_has_side_effects(compiler, invocation)
        {
            return;
        }

        // Handle known cases now (Object, Date, RegExp, etc)
        if invocation.is_new(compiler)
            && !self
                .pfi
                .ast_analyzer
                .constructor_call_has_side_effects(compiler, invocation)
        {
            return;
        }

        let callee_summaries = self.pfi.get_summaries_for_callee(compiler, invocation);
        if callee_summaries.is_empty() {
            self.summary(caller_info)
                .set_mutates_global_state_and_all_other_flags(compiler, None);
            return;
        }

        let enclosing_function = self.function_scope_stack.last().unwrap();
        let propates_throws = enclosing_function.catch_depth == 0;
        let caller_is_arrow_function = enclosing_function.root.unwrap().is_arrow_function(compiler);
        for callee_info in callee_summaries {
            let edge = SideEffectPropagation::for_invocation(
                compiler,
                invocation,
                caller_is_arrow_function,
                propates_throws,
            );
            self.pfi.reverse_call_graph.connect_nodes(
                self.pfi.summaries[callee_info.0].graph_node,
                edge,
                self.pfi.summaries[caller_info.0].graph_node,
            );
        }
    }

    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#addToCatchDepthIfTryBlock
    fn add_to_catch_depth_if_try_block(&mut self, ast: &Ast, n: NodeId, delta: i32) {
        let parent = n.get_parent(ast);
        if !n.is_block(ast) || !parent.unwrap().is_try(ast) || !n.is_first_child_of(ast, parent) {
            return;
        }

        let js_catch = n.get_next(ast).unwrap().get_first_child(ast);
        if js_catch.is_none() {
            return;
        }

        self.last_entry().catch_depth += delta;
    }
}

impl Callback for FunctionBodyAnalyzer<'_> {
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#shouldTraverse
    fn should_traverse(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        self.add_to_catch_depth_if_try_block(traversal, node, 1);
        true
    }

    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#visit
    fn visit(&mut self, traversal: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        self.add_to_catch_depth_if_try_block(traversal, node, -1);

        if NodeUtil::is_invocation(traversal, node) {
            // We collect these after filtering for side-effects because there's no point
            // re-processing a known pure call. This analysis is run multiple times, but no
            // optimization will make a pure function impure.
            self.pfi.all_function_calls.push(node);
        }

        let enclosing_function = self.function_scope_stack.last().unwrap();
        let root = enclosing_function.root;
        let is_artificially_pure = enclosing_function.is_artificially_pure;
        let Some(root) = root.filter(|_| !is_artificially_pure) else {
            // Within artificially pure function literal bodies, we do not ever need to call
            // updateSideEffectsForNode because we intentionally ignore any side effects, and
            // still consider the function as a whole to be pure.
            // Note: this doesn't mean that individual invocations within the function body are
            // always considered pure - those will still be marked in a later stage of
            // PureFunctionIdentifier. We just don't propagate any impurity to the enclosing
            // function literal.
            return;
        };

        for summary in self.pfi.summaries_for_function(root) {
            self.update_side_effects_for_node(summary, traversal, node);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for FunctionBodyAnalyzer<'_> {
    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let scope = t.get_scope();
        if !scope.is_function_scope(t.get_compiler()) {
            return;
        }

        let function = t.get_scope_root().unwrap();
        check_state!(function.is_function(t), "%s", function.to_string(t));
        let jsdoc = NodeUtil::get_best_jsdoc_info(t, function);
        let is_artificially_pure = jsdoc.is_some_and(|jsdoc| jsdoc.is_no_side_effects());

        self.function_scope_stack.push(FunctionStackEntry::new(
            Some(function),
            is_artificially_pure,
        ));
        if !self
            .pfi
            .summaries_for_all_names_of_function_by_node
            .contains_key(&function)
        {
            // This function was not part of a definition which is why it was not created by
            // populateDatastructuresForAnalysisTraversal. For example, an anonymous function.
            let summary = AmbiguatedFunctionSummary::create_in_graph(
                &mut self.pfi.summaries,
                &mut self.pfi.reverse_call_graph,
                JsString::from("<anonymous>"),
            );
            self.pfi.put_summary_for_function(function, summary);
        }
    }

    // port: PureFunctionIdentifier.FunctionBodyAnalyzer#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        // We want to process block scope as well as function scopes
        let scope = t.get_scope();
        let function_scope = scope.get_closest_container_scope(t.get_compiler());
        if !function_scope.is_function_scope(t.get_compiler()) {
            return;
        }

        let function_scope_root = function_scope.get_root_node(t.get_compiler());
        check_state!(
            Some(function_scope_root) == self.function_scope_stack.last().unwrap().root,
            "%s",
            function_scope_root.to_string(t)
        );
        // Java keeps reading `functionEntry` after removing it from the stack.
        let popped = if t.get_scope_root() == self.function_scope_stack.last().unwrap().root {
            self.function_scope_stack.pop()
        } else {
            None
        };
        let function_entry = popped
            .as_ref()
            .unwrap_or_else(|| self.function_scope_stack.last().unwrap());
        let summaries = &mut self.pfi.summaries;

        // Handle deferred local variable modifications:
        let root = function_entry.root.unwrap();
        for side_effect_info in self
            .pfi
            .summaries_for_all_names_of_function_by_node
            .get(&root)
            .cloned()
            .unwrap_or_default()
        {
            let side_effect_info = &mut summaries[side_effect_info.0];

            if side_effect_info.mutates_global_state() {
                continue;
            }

            let scope = t.get_scope();
            let compiler = t.get_compiler();
            for v in scope.get_var_iterable(compiler) {
                let is_from_destructuring =
                    NodeUtil::is_lhs_by_destructuring(compiler, v.get_name_node(compiler).unwrap());
                if v.is_param(compiler)
                    // Ignore destructuring parameters because they don't directly correspond to
                    // an argument passed to the function for the purposes of
                    // "setMutatesArguments"
                    && !is_from_destructuring
                    && !function_entry.skiplisted_vars.contains(&v)
                    && function_entry.tainted_vars.contains(&v)
                {
                    side_effect_info.set_mutates_arguments(compiler, v.get_node(compiler));
                    continue;
                }

                let mut local_var = false;
                // Parameters and catch values can come from other scopes.
                if !v.is_param(compiler) && !v.is_catch(compiler) {
                    // TODO(johnlenz): create a useful parameter list
                    // sideEffectInfo.addKnownLocal(v.getName());
                    local_var = true;
                }

                // Take care of locals that might have been tainted.
                if (!local_var || function_entry.skiplisted_vars.contains(&v))
                    && function_entry.tainted_vars.contains(&v)
                {
                    // If the function has global side-effects
                    // don't bother with the local side-effects.
                    side_effect_info.set_mutates_global_state_and_all_other_flags(
                        compiler,
                        v.get_node(compiler),
                    );
                    break;
                }
            }
        }
    }
}

/// This class stores all the information about a connection between functions needed to
/// propagate side effects from one instance of [`AmbiguatedFunctionSummary`] to another.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SideEffectPropagation {
    // Whether this propagation represents an aliasing of one name by another. In that case, all
    // side effects of the "callee" just need to be copied onto the "caller".
    caller_is_alias: bool,

    // If all the arguments passed to the callee are local to the caller.
    all_args_unescaped_local: bool,

    /*
     * If you call a function with apply or call, one of the arguments at the call site will be
     * used as 'this' inside the implementation. If this is pass into apply like so:
     * function.apply(this, ...) then 'this' in the caller is tainted.
     */
    callee_this_equals_caller_this: bool,

    /// Whether this propagation includes the "throws" bit.
    ///
    /// In some contexts, such as an invocation inside a "try", the caller is uneffected by the
    /// callee throwing.
    propagate_throws: bool,

    // The token used to invoke the callee by the caller.
    invocation: Option<NodeId>,
}

impl GraphvizValue for SideEffectPropagation {
    // Java's default `Object#toString`; this pass never prints the graph.
    fn to_graphviz_string(&self) -> String {
        format!("{self:?}")
    }
}

impl SideEffectPropagation {
    /// `ast` reads the invocation for Java's argument check; it is `None` only when
    /// `invocation` is `None` (Java's `invocation == null` branch).
    // port: PureFunctionIdentifier.SideEffectPropagation#SideEffectPropagation
    fn new(
        ast: Option<&Ast>,
        caller_is_alias: bool,
        all_args_unescaped_local: bool,
        callee_this_equals_caller_this: bool,
        propagate_throws: bool,
        invocation: Option<NodeId>,
    ) -> Self {
        if let (Some(ast), Some(invocation)) = (ast, invocation) {
            check_argument!(
                NodeUtil::is_invocation(ast, invocation),
                "%s",
                invocation.to_string(ast)
            );
        }

        Self {
            caller_is_alias,
            all_args_unescaped_local,
            callee_this_equals_caller_this,
            propagate_throws,
            invocation,
        }
    }

    // port: PureFunctionIdentifier.SideEffectPropagation#forAlias
    fn for_alias() -> Self {
        Self::new(None, true, false, false, true, None)
    }

    // port: PureFunctionIdentifier.SideEffectPropagation#forInvocation
    fn for_invocation(
        ast: &Ast,
        invocation: NodeId,
        caller_is_arrow_function: bool,
        propagate_throws: bool,
    ) -> Self {
        check_argument!(
            NodeUtil::is_invocation(ast, invocation),
            "%s",
            invocation.to_string(ast)
        );

        Self::new(
            Some(ast),
            false,
            NodeUtil::all_args_unescaped_local(ast, invocation),
            // `this` in an arrow function is the `this` of the enclosing function, which is not
            // tracked.
            Self::callee_and_caller_share_this(ast, invocation) && !caller_is_arrow_function,
            propagate_throws,
            Some(invocation),
        )
    }

    // port: PureFunctionIdentifier.SideEffectPropagation#calleeAndCallerShareThis
    fn callee_and_caller_share_this(ast: &Ast, invocation: NodeId) -> bool {
        if !PureFunctionIdentifier::is_call_or_tagged_template_lit(ast, invocation) {
            return false; // Calling a constructor creates a new object bound to `this`.
        }

        let callee = invocation.get_first_child(ast).unwrap();
        if callee.is_super(ast) {
            return true;
        }

        let this_arg: Option<NodeId> =
            if PureFunctionIdentifier::is_invocation_via_call_or_apply(ast, invocation) {
                // If the call site is actually a `.call` or `.apply`, then `this` will be an
                // argument.
                invocation.get_second_child(ast)
            } else if callee.is_get_prop(ast) || callee.is_opt_chain_get_prop(ast) {
                callee.get_first_child(ast)
            } else {
                None
            };

        let Some(this_arg) = this_arg else {
            return false; // No `this` is being passed.
        };
        if this_arg.is_this(ast) || this_arg.is_super(ast) {
            return true;
        }

        // TODO(nickreid): If `thisArg` is a known local or known arg we could say something more
        // specific about the effect of the callee mutating `this`.

        // We're not sure what `this` is being passed, so make a conservative choice.
        false
    }

    /// Propagate the side effects from the callee to the caller.
    ///
    /// Returns true if the propagation changed the side effects on the caller.
    // port: PureFunctionIdentifier.SideEffectPropagation#propagate
    fn propagate(
        &self,
        ast: &Ast,
        summaries: &mut [AmbiguatedFunctionSummary],
        callee: SummaryId,
        caller: SummaryId,
    ) -> bool {
        // `callee` and `caller` may be the same summary (a self-edge), so the callee's state is
        // read through its handle at each use, as Java reads the shared object.
        let initial_caller_flags = summaries[caller.0].bitmask;

        if self.caller_is_alias {
            let callee_bitmask = summaries[callee.0].bitmask;
            summaries[caller.0].set_mask(callee_bitmask);
            return summaries[caller.0].bitmask != initial_caller_flags;
        }

        if summaries[callee.0].mutates_global_state() {
            // If the callee modifies global state then so does that caller.
            summaries[caller.0].set_mutates_global_state_and_all_other_flags(ast, self.invocation);
        }
        if self.propagate_throws && summaries[callee.0].function_throws() {
            // If the callee throws an exception then so does the caller.
            summaries[caller.0].set_throws(ast, self.invocation);
        }
        if summaries[callee.0].mutates_arguments() && !self.all_args_unescaped_local {
            // If the callee mutates its input arguments and the arguments escape the caller then
            // it has unbounded side effects.
            summaries[caller.0].set_mutates_global_state_and_all_other_flags(ast, self.invocation);
        }
        if summaries[callee.0].mutates_this() {
            if self.invocation.unwrap().is_new(ast) {
                // NEWing a constructor provide a unescaped "this" making side-effects impossible.
            } else if self.callee_this_equals_caller_this {
                summaries[caller.0].set_mutates_this(ast, self.invocation);
            } else {
                summaries[caller.0]
                    .set_mutates_global_state_and_all_other_flags(ast, self.invocation);
            }
        }

        summaries[caller.0].bitmask != initial_caller_flags
    }
}

/// A summary for the set of functions that share a particular name.
///
/// Side-effects of the functions are the most significant aspect of this summary. Because the
/// functions are "ambiguated", the recorded side-effects are the union of all side effects
/// detected in any member of the set.
///
/// Name in this context refers to a short name, not a qualified name; only the last segment of a
/// qualified name is used.
struct AmbiguatedFunctionSummary {
    // The name shared by the set of functions that defined this summary.
    name: JsString,
    // The node holding this summary in the reverse call graph.
    graph_node: DiGraphNode,
    // The side effect flags for this set of functions.
    // TODO(nickreid): Replace this with a `Node.SideEffectFlags`.
    bitmask: i32,
    collect_impure_debugging_reason: bool,
    // FUNCTION nodes with a name corresponding to this summary that are impure.
    // Only ever initialized if `collect_impure_debugging_reason` is true, and if
    // PureFunctionIdentifier finds that this summary has a non-extern function that's
    // "artificially pure", i.e. has a @nosideeffects annotation. Entries are Java's
    // possibly-null `NodeUtil.getEnclosingFunction` results.
    impure_function_reasons_for_debugging: Option<IndexSet<Option<NodeId>>>, // lazily initialized
}

impl AmbiguatedFunctionSummary {
    // Side effect types:
    const THROWS: i32 = 1 << 0;
    const MUTATES_GLOBAL_STATE: i32 = 1 << 1;
    const MUTATES_THIS: i32 = 1 << 2;
    const MUTATES_ARGUMENTS: i32 = 1 << 3;

    /// Adds a new summary node to `graph`, storing the node and returning the summary.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#createInGraph
    fn create_in_graph(
        summaries: &mut Vec<AmbiguatedFunctionSummary>,
        graph: &mut LinkedDirectedGraph<SummaryId, SideEffectPropagation>,
        name: JsString,
    ) -> SummaryId {
        let id = SummaryId(summaries.len());
        let summary = Self::new(graph, id, name);
        summaries.push(summary);
        id
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#AmbiguatedFunctionSummary
    fn new(
        graph: &mut LinkedDirectedGraph<SummaryId, SideEffectPropagation>,
        id: SummaryId,
        name: JsString,
    ) -> Self {
        Self {
            name,
            graph_node: graph.create_node(id),
            bitmask: 0,
            collect_impure_debugging_reason: false,
            impure_function_reasons_for_debugging: None,
        }
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#setMask
    fn set_mask(&mut self, mask: i32) -> &mut Self {
        self.bitmask |= mask;
        self
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#getMask
    fn get_mask(&self, mask: i32) -> bool {
        (self.bitmask & mask) != 0
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#maybeCollectImpureDebuggingReason
    fn maybe_collect_impure_debugging_reason(
        &mut self,
        ast: &Ast,
        debugging_reason: Option<NodeId>,
    ) {
        if !self.collect_impure_debugging_reason {
            return;
        }
        let Some(debugging_reason) = debugging_reason else {
            return;
        };
        self.impure_function_reasons_for_debugging
            .get_or_insert_with(IndexSet::new)
            .insert(NodeUtil::get_enclosing_function(ast, debugging_reason));
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#mutatesThis
    fn mutates_this(&self) -> bool {
        // MUTATES_GLOBAL_STATE implies MUTATES_THIS
        self.get_mask(Self::MUTATES_THIS)
    }

    /// Marks the function as having "modifies this" side effects.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#setMutatesThis
    fn set_mutates_this(&mut self, ast: &Ast, debugging_reason: Option<NodeId>) -> &mut Self {
        self.maybe_collect_impure_debugging_reason(ast, debugging_reason);
        self.set_mask(Self::MUTATES_THIS)
    }

    /// Returns true if function has an explicit "throw".
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#functionThrows
    fn function_throws(&self) -> bool {
        // MUTATES_GLOBAL_STATE implies THROWS
        self.get_mask(Self::THROWS)
    }

    /// Marks the function as having "throw" side effects.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#setThrows
    fn set_throws(&mut self, ast: &Ast, debugging_reason: Option<NodeId>) -> &mut Self {
        self.maybe_collect_impure_debugging_reason(ast, debugging_reason);
        self.set_mask(Self::THROWS)
    }

    /// Returns true if function mutates global state.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#mutatesGlobalState
    fn mutates_global_state(&self) -> bool {
        self.get_mask(Self::MUTATES_GLOBAL_STATE)
    }

    /// Marks the function as having "modifies globals" side effects.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#setMutatesGlobalStateAndAllOtherFlags
    fn set_mutates_global_state_and_all_other_flags(
        &mut self,
        ast: &Ast,
        debugging_reason: Option<NodeId>,
    ) -> &mut Self {
        self.maybe_collect_impure_debugging_reason(ast, debugging_reason);
        self.set_mask(
            Self::THROWS
                | Self::MUTATES_THIS
                | Self::MUTATES_ARGUMENTS
                | Self::MUTATES_GLOBAL_STATE,
        )
    }

    /// Returns true if function mutates its arguments.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#mutatesArguments
    fn mutates_arguments(&self) -> bool {
        // MUTATES_GLOBAL_STATE implies MUTATES_ARGUMENTS
        self.get_mask(Self::MUTATES_ARGUMENTS)
    }

    /// Marks the function as having "modifies arguments" side effects.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#setMutatesArguments
    fn set_mutates_arguments(&mut self, ast: &Ast, debugging_reason: Option<NodeId>) -> &mut Self {
        self.maybe_collect_impure_debugging_reason(ast, debugging_reason);
        self.set_mask(Self::MUTATES_ARGUMENTS)
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#hasNoFlagsSet
    fn has_no_flags_set(&self) -> bool {
        self.bitmask == 0
    }

    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#setCollectImpureDebuggingReason
    fn set_collect_impure_debugging_reason(&mut self) {
        self.collect_impure_debugging_reason = true;
    }
}

impl fmt::Display for AmbiguatedFunctionSummary {
    // Java's toString is @DoNotCall (debugging only): MoreObjects.toStringHelper with the name,
    // the graph node's hash code (here its arena index) and the side effects.
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AmbiguatedFunctionSummary{{name={}, graphNode={}, sideEffects={}}}",
            self.name.to_string_lossy(),
            self.graph_node.0,
            self.side_effects_to_string()
        )
    }
}

impl AmbiguatedFunctionSummary {
    // port: PureFunctionIdentifier.AmbiguatedFunctionSummary#sideEffectsToString
    fn side_effects_to_string(&self) -> String {
        let mut status = Vec::new();
        if self.mutates_this() {
            status.push("this");
        }

        if self.mutates_global_state() {
            status.push("global");
        }

        if self.mutates_arguments() {
            status.push("args");
        }

        if self.function_throws() {
            status.push("throw");
        }

        // List#toString
        format!("[{}]", status.join(", "))
    }
}

/// A compiler pass that constructs a reference graph and drives the PureFunctionIdentifier
/// across it.
#[derive(Default)]
pub struct Driver;

impl Driver {
    // port: PureFunctionIdentifier.Driver#Driver
    pub fn new() -> Self {
        Self
    }
}

impl CompilerPass for Driver {
    // port: PureFunctionIdentifier.Driver#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let should_validate =
            compiler
                .get_options()
                .get_warnings_guard()
                .level(&JSError::make_without_location(
                    &UNUSED_ARTIFICIAL_PURE_ANNOTATION,
                    &["", ""],
                ));
        let assume_getters_are_pure = compiler.get_options().get_assume_getters_are_pure();
        let pass = PureFunctionIdentifier::new(
            compiler,
            assume_getters_are_pure,
            should_validate != Some(CheckLevel::OFF),
        );
        OptimizeCalls::builder()
            .set_compiler(compiler)
            .set_consider_externs(true)
            .add_pass(Box::new(pass))
            .build()
            .process(compiler, externs, root);
    }
}
