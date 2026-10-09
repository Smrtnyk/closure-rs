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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CheckMissingReturn.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    control_flow_graph::{Branch, ControlFlowGraph},
    diagnostic_type::DiagnosticType,
    graph::{
        check_paths_between_nodes::CheckPathsBetweenNodes, di_graph::DiGraphEdge, graph::GraphEdge,
        graph_node::GraphNode,
    },
    js_error::JSError,
    js_iterables::JsIterables,
    node_traversal::{AbstractCfgCallback, Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    promises::Promises,
};
use closure_jstype::prelude::{FunctionType, JSType, JSTypeNative, TypeId};
use closure_rhino::{
    jscomp_base::tri::Tri,
    node::{Ast, NodeId},
};

// port: CheckMissingReturn#MISSING_RETURN_STATEMENT
pub static MISSING_RETURN_STATEMENT: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_RETURN_STATEMENT",
    "Missing return statement. Function expected to return {0}.",
);

/// Checks functions for missing return statements. Return statements are only expected for
/// functions with return type information. Functions with empty bodies are ignored.
pub struct CheckMissingReturn<'a> {
    base: AbstractCfgCallback<'a>,
}

impl CheckMissingReturn<'_> {
    // port: CheckMissingReturn#goesThroughTrueCondition
    /* Skips all exception edges and impossible edges. */
    fn goes_through_true_condition(
        ast: &Ast,
        graph: &ControlFlowGraph<NodeId>,
        input: DiGraphEdge,
    ) -> bool {
        // First skill all exceptions.
        let branch = *input.get_value(graph);
        if branch == Branch::ON_EX {
            return false;
        } else if branch.is_conditional() {
            let condition = NodeUtil::get_condition_expression(
                ast,
                input.get_source(graph).get_value(graph).unwrap(),
            );
            // TODO(user): We CAN make this bit smarter just looking at
            // constants. We DO have a full blown ReverseAbstractInterupter and
            // type system that can evaluate some impressions' boolean value but
            // for now we will keep this pass lightweight.
            if let Some(condition) = condition {
                let val = NodeUtil::get_boolean_value(ast, condition);
                if val != Tri::UNKNOWN {
                    return val.to_boolean(true) == (branch == Branch::ON_TRUE);
                }
            }
        }
        true
    }

    // port: CheckMissingReturn#CheckMissingReturn
    pub fn new() -> Self {
        let mut base = AbstractCfgCallback::new();
        base.set_enter_scope_with_cfg(Self::enter_scope_with_cfg);
        Self { base }
    }

    // port: CheckMissingReturn#enterScopeWithCfg
    fn enter_scope_with_cfg(this: &mut AbstractCfgCallback<'_>, t: &mut NodeTraversal<'_>) {
        let n = t.get_scope_root().unwrap();
        let return_type = Self::get_explicit_return_type_if_expected(t.get_compiler(), n);

        let Some(return_type) = return_type else {
            // No return value is expected, so nothing to check.
            return;
        };

        if n.is_arrow_function(t) {
            let function_body = NodeUtil::get_function_body(t, n);
            if !function_body.is_block(t) {
                // Body is an expression, which is the implicit return value.
                return;
            }
        }

        let compiler = t.get_compiler();
        let cfg = this.get_control_flow_graph_mut(compiler);
        if Self::fast_all_paths_return_check(compiler, cfg) {
            return;
        }

        let all_paths_satisfy_predicate = {
            let ast: &Ast = compiler;
            let test = CheckPathsBetweenNodes::<Option<NodeId>, Branch, _, _>::new(
                cfg.get_entry(),
                cfg.get_implicit_return(),
                |input: &Option<NodeId>| input.is_some_and(|input| input.is_return(ast)),
                |graph: &ControlFlowGraph<NodeId>, input: DiGraphEdge| {
                    Self::goes_through_true_condition(ast, graph, input)
                },
            );
            test.all_paths_satisfy_predicate(cfg)
        };

        if !all_paths_satisfy_predicate {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let return_type_string = return_type.to_string(registry, ast);
            let error = JSError::make(
                compiler,
                n,
                &MISSING_RETURN_STATEMENT,
                &[return_type_string.as_str()],
            );
            compiler.report(error);
        }
    }

    // port: CheckMissingReturn#fastAllPathsReturnCheck
    /// Fast check to see if all execution paths contain a return statement. May spuriously report
    /// that a return statement is missing.
    ///
    /// Returns true if all paths return, converse not necessarily true.
    fn fast_all_paths_return_check(
        compiler: &mut AbstractCompiler,
        cfg: &ControlFlowGraph<NodeId>,
    ) -> bool {
        let (convention, registry, ast) = compiler.get_coding_convention_type_registry_and_ast();
        for s in cfg.get_implicit_return().get_in_edges(cfg) {
            let n = s.get_source(cfg).get_value(cfg).unwrap();
            // NOTE(dimvar): it is possible to change ControlFlowAnalysis.java, so
            // that the calls that always throw are treated in the same way as THROW
            // in the CFG. Then, we would not need to use the coding convention here.
            if !n.is_return(ast)
                && !convention.is_function_call_that_always_throws(ast, registry, n)
            {
                return false;
            }
        }
        true
    }

    // port: CheckMissingReturn#getExplicitReturnTypeIfExpected
    /// Determines if the given scope should explicitly return. All functions with non-void or
    /// non-unknown return types must have explicit returns.
    ///
    /// Exception: Constructors which specifically specify a return type are used to allow
    /// invocation without requiring the "new" keyword. They have an implicit return type. See
    /// unit tests.
    ///
    /// If a return type is expected, returns it. Otherwise, returns null.
    fn get_explicit_return_type_if_expected(
        compiler: &mut AbstractCompiler,
        scope_root: NodeId,
    ) -> Option<TypeId> {
        if !scope_root.is_function(compiler) {
            // Nothing to do in a global/module/block scope.
            return None;
        }
        // JSType.toMaybeFunctionType(JSType) is null-safe.
        let scope_type = match scope_root.get_jstype(compiler) {
            Some(type_) => type_.to_maybe_function_type(compiler.get_type_registry()),
            None => None,
        };

        let scope_type = scope_type?;

        if Self::is_empty_function(compiler, scope_root) {
            return None;
        }

        if scope_type.is_constructor(compiler.get_type_registry()) {
            return None;
        }

        // FunctionType#getReturnType is never null in the arena API (Java's null check has no
        // counterpart).
        let mut return_type = scope_type.get_return_type(compiler.get_type_registry());

        if scope_root.is_async_function(compiler) {
            // Unwrap the declared return type (e.g. "!Promise<number>" becomes "number")
            let (registry, ast) = compiler.get_type_registry_and_ast();
            return_type = Promises::get_template_type_of_thenable(registry, ast, return_type);
        } else if scope_root.is_generator_function(compiler) {
            // Unwrap the declared return type (e.g. "!Generator<string, number>" becomes "number")
            let (registry, ast) = compiler.get_type_registry_and_ast();
            return_type = JsIterables::get_return_element_type(return_type, registry, ast);
        }

        if !Self::is_void_or_unknown(compiler, return_type) {
            return Some(return_type);
        }

        None
    }

    // port: CheckMissingReturn#isEmptyFunction
    /// Returns `true` if function represents a JavaScript function with an empty body.
    fn is_empty_function(ast: &Ast, function: NodeId) -> bool {
        function.has_x_children(ast, 3)
            && !function
                .get_second_child(ast)
                .unwrap()
                .get_next(ast)
                .unwrap()
                .has_children(ast)
    }

    // port: CheckMissingReturn#isVoidOrUnknown
    /// Returns `true` if returnType is void, unknown, or a union containing void or unknown.
    fn is_void_or_unknown(compiler: &mut AbstractCompiler, return_type: TypeId) -> bool {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let void_type = registry.get_native_type(JSTypeNative::VOID_TYPE);
        void_type.is_subtype_of(registry, ast, return_type)
    }
}

impl Default for CheckMissingReturn<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Callback for CheckMissingReturn<'_> {
    // port: CheckMissingReturn#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckMissingReturn#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(&mut self.base)
    }
}
