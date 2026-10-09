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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/FunctionRewriter.java.

//! Port of `FunctionRewriter.java`.
//!
//! Reduces the size of common function expressions.
//!
//! This pass will rewrite:
//!
//! C.prototype.getA = function() { return this.a_ }; C.prototype.setA = function(newValue) {
//! this.a_ = newValue };
//!
//! as:
//!
//! C.prototype.getA = JSCompiler_get("a"); C.prototype.setA = JSCompiler_set("a");
//!
//! if by doing so we will save bytes, after the helper functions are added and renaming is done.
//!
//! NOTE: JSCompiler_get and JSCompiler_set turn dotted accesses to computed accesses, which
//! causes JS engines to use dictionary lookups. Because of this perf regression, this pass is off
//! by default in advanced mode even though it improves code size.

use crate::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::inline_cost_estimator::InlineCostEstimator;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::ir::IR;
use closure_rhino::node::{Ast, NodeId, Prop};

// Safety margin used to avoid growing simple programs by a few bytes.
// Selected arbitrarily.
// port: FunctionRewriter#SAVINGS_THRESHOLD
const SAVINGS_THRESHOLD: i32 = 16;

pub struct FunctionRewriter;

impl FunctionRewriter {
    // port: FunctionRewriter#FunctionRewriter
    pub fn new() -> Self {
        Self
    }

    // port: FunctionRewriter#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let reducers: Vec<Box<dyn Reducer>> = vec![
            Box::new(ReturnConstantReducer),
            Box::new(GetterReducer),
            Box::new(SetterReducer),
            Box::new(EmptyFunctionReducer),
            Box::new(IdentityReducer),
        ];

        // Java HashMultimap<Reducer, Reduction>: the values of one reducer are an identity
        // hash set; the savings sum does not depend on their order and each reduction replaces
        // a different node, so the reductions are kept in discovery order.
        let mut reduction_map: Vec<Vec<Reduction>> = reducers.iter().map(|_| Vec::new()).collect();

        // Accumulate possible reductions in the reduction multi-map.  They
        // will be applied in the loop below.
        NodeTraversal::traverse(
            compiler,
            root,
            &mut ReductionGatherer::new(&reducers, &mut reduction_map),
        );

        // Apply reductions iff they will provide some savings.
        for (i, reducer) in reducers.iter().enumerate() {
            let reductions = &reduction_map[i];
            if reductions.is_empty() {
                continue;
            }

            let Some(helper_code) = self.parse_helper_code(compiler, reducer.as_ref()) else {
                continue;
            };

            let helper_code_cost = InlineCostEstimator::get_cost(compiler, helper_code);

            // Estimate savings
            let mut savings: i32 = 0;
            for reduction in reductions {
                savings = savings.wrapping_add(reduction.estimate_savings(compiler));
            }

            // Compare estimated savings against the helper cost.  Apply
            // reductions if doing so will result in some savings.
            if savings > (helper_code_cost + SAVINGS_THRESHOLD) {
                for reduction in reductions {
                    reduction.apply(compiler);
                }

                let adding_root = compiler.get_node_for_code_insertion(None);
                NodeUtil::mark_new_scopes_changed(compiler, helper_code);
                adding_root.add_child_to_front(compiler, helper_code);
                compiler.report_change_to_enclosing_scope(adding_root);
            }
        }
    }

    /// Parse helper code needed by a reducer.
    ///
    /// Returns the helper code root. If parse fails, return null.
    // port: FunctionRewriter#parseHelperCode
    pub fn parse_helper_code(
        &mut self,
        compiler: &mut AbstractCompiler,
        reducer: &dyn Reducer,
    ) -> Option<NodeId> {
        // Java: reducer.getClass() + ":helper" (Class#toString is "class " + getName())
        let root = compiler.parse_synthetic_code(
            &format!("class {}:helper", reducer.java_class_name()),
            reducer.get_helper_source(),
        );
        root.remove_first_child(compiler)
    }
}

impl Default for FunctionRewriter {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for FunctionRewriter {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}

// port: FunctionRewriter#isReduceableFunctionExpression
fn is_reduceable_function_expression(ast: &Ast, n: NodeId) -> bool {
    let parent = n.get_parent(ast).unwrap();
    NodeUtil::is_function_expression(ast, n)
        && !NodeUtil::is_get_or_set_key(ast, parent)
        && !parent.is_member_function_def(ast)
        && !n.is_async_function(ast)
        && !n.is_generator_function(ast)
        && !NodeUtil::has_non_simple_parameters(ast, n)
}

/// Information needed to apply a reduction.
// port: FunctionRewriter.Reduction
struct Reduction {
    old_child: NodeId,
    new_child: NodeId,
}

impl Reduction {
    // port: FunctionRewriter.Reduction#Reduction
    fn new(old_child: NodeId, new_child: NodeId) -> Self {
        Self {
            old_child,
            new_child,
        }
    }

    /// Apply the reduction by replacing the old child with the new child.
    // port: FunctionRewriter.Reduction#apply
    fn apply(&self, compiler: &mut AbstractCompiler) {
        self.old_child.replace_with(compiler, self.new_child);
        NodeUtil::mark_functions_deleted(compiler, self.old_child);
        compiler.report_change_to_enclosing_scope(self.new_child);
    }

    /// Estimate number of bytes saved by applying this reduction.
    // port: FunctionRewriter.Reduction#estimateSavings
    fn estimate_savings(&self, ast: &Ast) -> i32 {
        InlineCostEstimator::get_cost(ast, self.old_child)
            - InlineCostEstimator::get_cost(ast, self.new_child)
    }
}

/// Gathers a list of reductions to apply later by doing an in-order AST traversal. If a suitable
/// reduction is found, stop traversal in that branch.
// port: FunctionRewriter.ReductionGatherer
struct ReductionGatherer<'a> {
    reducers: &'a [Box<dyn Reducer>],
    reductions: &'a mut Vec<Vec<Reduction>>,
}

impl<'a> ReductionGatherer<'a> {
    /// `reducers`: List of reducers to apply during traversal. `reductions`: Reducer ->
    /// Reduction multimap, populated during traversal.
    // port: FunctionRewriter.ReductionGatherer#ReductionGatherer
    fn new(reducers: &'a [Box<dyn Reducer>], reductions: &'a mut Vec<Vec<Reduction>>) -> Self {
        Self {
            reducers,
            reductions,
        }
    }
}

impl Callback for ReductionGatherer<'_> {
    // port: FunctionRewriter.ReductionGatherer#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        for (i, reducer) in self.reducers.iter().enumerate() {
            let replacement = reducer.reduce(t, node);
            if replacement != node {
                self.reductions[i].push(Reduction::new(node, replacement));
                return false;
            }
        }
        true
    }

    // port: FunctionRewriter.ReductionGatherer#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _node: NodeId, _parent: Option<NodeId>) {}
}

/// Interface implemented by the strength-reduction optimizers below.
// port: FunctionRewriter.Reducer
pub trait Reducer {
    /// Java `getClass().getName()`.
    fn java_class_name(&self) -> &'static str;

    /// Returns JS source for helper methods used by this reduction.
    // port: FunctionRewriter.Reducer#getHelperSource
    fn get_helper_source(&self) -> String;

    /// Returns root of the reduced subtree if a reduction was applied; otherwise returns the
    /// node argument.
    // port: FunctionRewriter.Reducer#reduce
    fn reduce(&self, ast: &mut Ast, node: NodeId) -> NodeId;
}

/// Builds a method call based on the the given method name, argument and history.
///
/// `method_name`: Method to call. `argument_node`: Method argument.
// port: FunctionRewriter.Reducer#buildCallNode
fn build_call_node(
    ast: &mut Ast,
    method_name: &str,
    argument_node: Option<NodeId>,
    srcref: NodeId,
) -> NodeId {
    let name = IR::name(ast, method_name);
    let call = IR::call(ast, name, &[]).srcref_tree(ast, srcref);
    call.put_boolean_prop(ast, Prop::FREE_CALL, true);
    if let Some(argument_node) = argument_node {
        let clone = argument_node.clone_tree(ast);
        call.add_child_to_back(ast, clone);
    }
    call
}

/// Reduces return immutable constant literal methods declarations with calls to a constant
/// return method factory.
///
/// Example: a.prototype.b = function() {} is reduced to: a.prototype.b = emptyFn();
// port: FunctionRewriter.EmptyFunctionReducer
struct EmptyFunctionReducer;

impl EmptyFunctionReducer {
    // port: FunctionRewriter.EmptyFunctionReducer#FACTORY_METHOD_NAME
    const FACTORY_METHOD_NAME: &'static str = "JSCompiler_emptyFn";
    // port: FunctionRewriter.EmptyFunctionReducer#HELPER_SOURCE
    fn helper_source() -> String {
        format!(
            "function {}() {{  return function() {{}}}}",
            Self::FACTORY_METHOD_NAME
        )
    }
}

impl Reducer for EmptyFunctionReducer {
    fn java_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.FunctionRewriter$EmptyFunctionReducer"
    }

    // port: FunctionRewriter.EmptyFunctionReducer#getHelperSource
    fn get_helper_source(&self) -> String {
        Self::helper_source()
    }

    // port: FunctionRewriter.EmptyFunctionReducer#reduce
    fn reduce(&self, ast: &mut Ast, node: NodeId) -> NodeId {
        if is_reduceable_function_expression(ast, node)
            && NodeUtil::is_empty_function_expression(ast, node)
        {
            build_call_node(ast, Self::FACTORY_METHOD_NAME, None, node)
        } else {
            node
        }
    }
}

/// Base class for reducers that match functions that contain a single return statement.
///
/// Returns function return value node if function body contains a single return statement.
/// Otherwise, null.
// port: FunctionRewriter.SingleReturnStatementReducer#maybeGetSingleReturnRValue
fn maybe_get_single_return_r_value(ast: &Ast, function_node: NodeId) -> Option<NodeId> {
    let body = function_node.get_last_child(ast).unwrap();
    if !body.has_one_child(ast) {
        return None;
    }

    let statement = body.get_first_child(ast).unwrap();
    if statement.is_return(ast) {
        return statement.get_first_child(ast);
    }
    None
}

/// Reduces property getter method declarations with calls to a getter method factory.
///
/// Example: a.prototype.b = function(a) {return a} is reduced to: a.prototype.b = getter(a);
// port: FunctionRewriter.IdentityReducer
struct IdentityReducer;

impl IdentityReducer {
    // port: FunctionRewriter.IdentityReducer#FACTORY_METHOD_NAME
    const FACTORY_METHOD_NAME: &'static str = "JSCompiler_identityFn";
    // port: FunctionRewriter.IdentityReducer#HELPER_SOURCE
    fn helper_source() -> String {
        let f = Self::FACTORY_METHOD_NAME;
        format!("function {f}() {{  return function({f}_value) {{return {f}_value}}}}")
    }

    /// Checks if the function matches the pattern: function(<value>, <rest>) {return <value>}
    ///
    /// Returns whether the function matches the pattern.
    // port: FunctionRewriter.IdentityReducer#isIdentityFunction
    fn is_identity_function(&self, ast: &Ast, function_node: NodeId) -> bool {
        let arg_list = function_node.get_second_child(ast).unwrap();
        let param_node = arg_list.get_first_child(ast);
        let Some(param_node) = param_node.filter(|p| p.is_name(ast)) else {
            // no parameters, or first parameter uses destructuring or a default value
            return false;
        };

        let value = maybe_get_single_return_r_value(ast, function_node);
        value.is_some_and(|value| {
            value.is_name(ast) && value.get_string(ast) == param_node.get_string(ast)
        })
    }
}

impl Reducer for IdentityReducer {
    fn java_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.FunctionRewriter$IdentityReducer"
    }

    // port: FunctionRewriter.IdentityReducer#getHelperSource
    fn get_helper_source(&self) -> String {
        Self::helper_source()
    }

    // port: FunctionRewriter.IdentityReducer#reduce
    fn reduce(&self, ast: &mut Ast, node: NodeId) -> NodeId {
        if !is_reduceable_function_expression(ast, node) {
            return node;
        }

        if self.is_identity_function(ast, node) {
            build_call_node(ast, Self::FACTORY_METHOD_NAME, None, node)
        } else {
            node
        }
    }
}

/// Reduces return immutable constant literal methods declarations with calls to a constant
/// return method factory.
///
/// Example: a.prototype.b = function() {return 10} is reduced to: a.prototype.b =
/// returnconst(10);
// port: FunctionRewriter.ReturnConstantReducer
struct ReturnConstantReducer;

impl ReturnConstantReducer {
    // port: FunctionRewriter.ReturnConstantReducer#FACTORY_METHOD_NAME
    const FACTORY_METHOD_NAME: &'static str = "JSCompiler_returnArg";
    // port: FunctionRewriter.ReturnConstantReducer#HELPER_SOURCE
    fn helper_source() -> String {
        let f = Self::FACTORY_METHOD_NAME;
        format!("function {f}({f}_value) {{  return function() {{return {f}_value}}}}")
    }

    /// Checks if the function matches the pattern: function(<args>) {return <immutable value>}
    /// and returns <immutable value> if a match is found.
    ///
    /// Returns the immutable value node; or null.
    // port: FunctionRewriter.ReturnConstantReducer#getValueNode
    fn get_value_node(&self, ast: &Ast, function_node: NodeId) -> Option<NodeId> {
        let value = maybe_get_single_return_r_value(ast, function_node);
        if let Some(value) = value
            && NodeUtil::is_immutable_value(ast, value)
        {
            return Some(value);
        }
        None
    }
}

impl Reducer for ReturnConstantReducer {
    fn java_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.FunctionRewriter$ReturnConstantReducer"
    }

    // port: FunctionRewriter.ReturnConstantReducer#getHelperSource
    fn get_helper_source(&self) -> String {
        Self::helper_source()
    }

    // port: FunctionRewriter.ReturnConstantReducer#reduce
    fn reduce(&self, ast: &mut Ast, node: NodeId) -> NodeId {
        if !is_reduceable_function_expression(ast, node) {
            return node;
        }

        let value_node = self.get_value_node(ast, node);
        if value_node.is_some() {
            build_call_node(ast, Self::FACTORY_METHOD_NAME, value_node, node)
        } else {
            node
        }
    }
}

/// Reduces property getter method declarations with calls to a getter method factory.
///
/// Example: a.prototype.b = function() {return this.b_} is reduced to: a.prototype.b =
/// getter("b_");
// port: FunctionRewriter.GetterReducer
struct GetterReducer;

impl GetterReducer {
    // port: FunctionRewriter.GetterReducer#FACTORY_METHOD_NAME
    const FACTORY_METHOD_NAME: &'static str = "JSCompiler_get";
    // port: FunctionRewriter.GetterReducer#HELPER_SOURCE
    fn helper_source() -> String {
        let f = Self::FACTORY_METHOD_NAME;
        format!("function {f}({f}_name) {{  return function() {{return this[{f}_name]}}}}")
    }

    /// Checks if the function matches the pattern: function(<args>) {return this.<name>} and
    /// returns <name> if a match is found.
    ///
    /// Returns STRING node that is the RHS of a this property get; or null.
    // port: FunctionRewriter.GetterReducer#getGetPropertyName
    fn get_get_property_name(&self, ast: &Ast, function_node: NodeId) -> Option<NodeId> {
        let value = maybe_get_single_return_r_value(ast, function_node);
        if let Some(value) = value
            && value.is_get_prop(ast)
            && value.get_first_child(ast).unwrap().is_this(ast)
        {
            return Some(value);
        }
        None
    }
}

impl Reducer for GetterReducer {
    fn java_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.FunctionRewriter$GetterReducer"
    }

    // port: FunctionRewriter.GetterReducer#getHelperSource
    fn get_helper_source(&self) -> String {
        Self::helper_source()
    }

    // port: FunctionRewriter.GetterReducer#reduce
    fn reduce(&self, ast: &mut Ast, node: NodeId) -> NodeId {
        if !is_reduceable_function_expression(ast, node) || node.is_arrow_function(ast) {
            return node;
        }

        let getprop = self.get_get_property_name(ast, node);
        if let Some(getprop) = getprop {
            if !getprop.is_get_prop(ast) {
                panic!("Expected GETPROP, got {}", getprop.get_token(ast));
            }

            let s = getprop.get_string(ast);
            let arg = IR::string(ast, s).srcref(ast, getprop);
            build_call_node(ast, Self::FACTORY_METHOD_NAME, Some(arg), node)
        } else {
            node
        }
    }
}

/// Reduces property setter method declarations with calls to a setter method factory.
///
/// Example: a.prototype.setB = function(value) {this.b_ = value} reduces to: a.prototype.setB =
/// getter("b_");
// port: FunctionRewriter.SetterReducer
struct SetterReducer;

impl SetterReducer {
    // port: FunctionRewriter.SetterReducer#FACTORY_METHOD_NAME
    const FACTORY_METHOD_NAME: &'static str = "JSCompiler_set";
    // port: FunctionRewriter.SetterReducer#HELPER_SOURCE
    fn helper_source() -> String {
        let f = Self::FACTORY_METHOD_NAME;
        format!(
            "function {f}({f}_name) {{  return function({f}_value) {{this[{f}_name] = {f}_value}}}}"
        )
    }

    /// Checks if the function matches the pattern: function(<value>, <rest>) {this.<name> =
    /// <value>} and returns <name> if a match is found.
    ///
    /// Returns STRING node that is the RHS of a this property get; or null.
    // port: FunctionRewriter.SetterReducer#getSetPropertyName
    fn get_set_property_name(&self, ast: &mut Ast, function_node: NodeId) -> Option<NodeId> {
        let body = function_node.get_last_child(ast).unwrap();
        if !body.has_one_child(ast) {
            return None;
        }

        let arg_list = function_node.get_second_child(ast).unwrap();
        let param_node = arg_list.get_first_child(ast);
        let Some(param_node) = param_node.filter(|p| p.is_name(ast)) else {
            // no parameters, or first parameter uses destructuring or a default value
            return None;
        };

        let statement = body.get_first_child(ast).unwrap();
        if !NodeUtil::is_expr_assign(ast, statement) {
            return None;
        }

        let assign = statement.get_first_child(ast).unwrap();
        let lhs = assign.get_first_child(ast).unwrap();
        if lhs.is_get_prop(ast) && lhs.get_first_child(ast).unwrap().is_this(ast) {
            let rhs = assign.get_last_child(ast).unwrap();
            if rhs.is_name(ast) && rhs.get_string(ast) == param_node.get_string(ast) {
                let s = lhs.get_string(ast);
                let string = IR::string(ast, s);
                return Some(string.clone_props_from(ast, lhs).srcref(ast, lhs));
            }
        }
        None
    }
}

impl Reducer for SetterReducer {
    fn java_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.FunctionRewriter$SetterReducer"
    }

    // port: FunctionRewriter.SetterReducer#getHelperSource
    fn get_helper_source(&self) -> String {
        Self::helper_source()
    }

    // port: FunctionRewriter.SetterReducer#reduce
    fn reduce(&self, ast: &mut Ast, node: NodeId) -> NodeId {
        if !is_reduceable_function_expression(ast, node) || node.is_arrow_function(ast) {
            return node;
        }

        let prop_name = self.get_set_property_name(ast, node);
        if let Some(prop_name) = prop_name {
            if !prop_name.is_string_lit(ast) {
                panic!("Expected STRING, got {}", prop_name.get_token(ast));
            }

            build_call_node(ast, Self::FACTORY_METHOD_NAME, Some(prop_name), node)
        } else {
            node
        }
    }
}
