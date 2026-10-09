/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractPeepholeOptimization.java.

//! Port of `AbstractPeepholeOptimization.java`.
//!
//! An abstract class whose implementations run peephole optimizations: optimizations that look
//! at a small section of code and either remove that code (if it is not needed) or replaces it
//! with smaller code.
//!
//! Java's abstract class becomes a trait whose default methods are the Java base-class methods.
//! The private base-class fields live in [`AbstractPeepholeOptimizationFields`], which every
//! implementation embeds and exposes through `fields`/`fields_mut`. Following DESIGN.md §6 the
//! compiler is never stored: Java's private `compiler` field becomes the `compiler_set` flag
//! (Java's `compiler != null`), and every method that used it takes the compiler as a parameter.

use crate::{
    abstract_compiler::AbstractCompiler, ast_analyzer::AstAnalyzer,
    coding_convention::CodingConvention, diagnostic_type::DiagnosticType, js_error::JSError,
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{check_not_null, check_state, jscomp_base::Tri, node::NodeId};
use num_bigint::BigInt;

/// The private fields of Java's `AbstractPeepholeOptimization`.
#[derive(Default)]
pub struct AbstractPeepholeOptimizationFields {
    /// Intentionally not exposed to subclasses. Java's `compiler != null`.
    compiler_set: bool,
    /// Intentionally not exposed to subclasses
    ast_analyzer: Option<AstAnalyzer>,
    /// New parser features added in some `optimizeSubtree` call.
    ///
    /// `PeepholeOptimizationsPass` uses this to call `NodeUtil#addFeaturesToScript` in closer to
    /// O(1) time, as this class doesn't know what script node it's currently in, and would need
    /// to walk the AST.
    new_features: IndexSet<Feature>,
}

impl AbstractPeepholeOptimizationFields {
    // port: AbstractPeepholeOptimization#AbstractPeepholeOptimization
    pub fn new() -> Self {
        Self::default()
    }

    /// The body of `AbstractPeepholeOptimization#beginTraversal`, callable as `super` by
    /// overriding implementations.
    // port: AbstractPeepholeOptimization#beginTraversal
    pub fn begin_traversal(&mut self, compiler: &mut AbstractCompiler) {
        self.compiler_set = true;
        self.ast_analyzer = Some(compiler.get_ast_analyzer());
    }

    /// The body of `AbstractPeepholeOptimization#endTraversal`, callable as `super` by
    /// overriding implementations; `class_name` is Java's `this.getClass().getName()`.
    // port: AbstractPeepholeOptimization#endTraversal
    pub fn end_traversal(&mut self, class_name: &str) {
        check_state!(
            self.new_features.is_empty(),
            "Expected getNewFeatures() to be empty in endTraversal() but found %s for %s",
            format_feature_set(&self.new_features),
            class_name
        );
        self.compiler_set = false;
        self.ast_analyzer = None;
    }

    // Rust-only: the `checkNotNull(compiler)` of Java's base-class methods.
    fn check_compiler(&self) {
        check_not_null!(self.compiler_set.then_some(()));
    }

    // Rust-only: Java's `astAnalyzer.` field dereference (NullPointerException when unset).
    fn ast_analyzer(&self) -> &AstAnalyzer {
        check_not_null!(self.ast_analyzer.as_ref())
    }
}

pub trait AbstractPeepholeOptimization {
    /// Rust-only accessor for the base-class fields.
    fn fields(&self) -> &AbstractPeepholeOptimizationFields;

    /// Rust-only mutable accessor for the base-class fields.
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields;

    /// Java's `getClass().getName()`, used by `endTraversal`'s precondition message.
    fn get_class_name(&self) -> &'static str;

    /// Given a node to optimize and a traversal, optimize the node. Subclasses should override to
    /// provide their own peephole optimization.
    ///
    /// Returns the new version of the subtree (or `None` if the subtree or one of its parents was
    /// removed from the AST). If the subtree has not changed, this method must return `subtree`.
    // port: AbstractPeepholeOptimization#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId>;

    /// Helper method for reporting an error to the compiler when applying a peephole
    /// optimization.
    // port: AbstractPeepholeOptimization#report
    fn report(
        &self,
        compiler: &mut AbstractCompiler,
        diagnostic: &'static DiagnosticType,
        n: NodeId,
    ) {
        let n_string = n.to_string(compiler);
        let error = JSError::make(compiler, n, diagnostic, &[&n_string]);
        compiler.report(error);
    }

    /// Are the nodes equal for the purpose of inlining? If type aware optimizations are on, type
    /// equality is checked.
    // port: AbstractPeepholeOptimization#areNodesEqualForInlining
    fn are_nodes_equal_for_inlining(
        &self,
        compiler: &AbstractCompiler,
        n1: NodeId,
        n2: NodeId,
    ) -> bool {
        // Our implementation delegates to the compiler. We provide this method because we don't
        // want to expose Compiler to PeepholeOptimizations.
        self.fields().check_compiler();
        compiler.are_nodes_equal_for_inlining(n1, n2)
    }

    /// Is the current AST normalized? (e.g. has the Normalize pass been run and has the
    /// Denormalize pass not yet been run?)
    // port: AbstractPeepholeOptimization#isASTNormalized
    fn is_ast_normalized(&self, compiler: &AbstractCompiler) -> bool {
        self.fields().check_compiler();
        compiler.get_life_cycle_stage().is_normalized()
    }

    /// Informs the optimization that a traversal will begin.
    // port: AbstractPeepholeOptimization#beginTraversal
    fn begin_traversal(&mut self, compiler: &mut AbstractCompiler) {
        self.fields_mut().begin_traversal(compiler);
    }

    /// Informs the optimization that a traversal has ended.
    ///
    /// This class cannot be used after this method is called, unless `beginTraversal` is called
    /// again.
    // port: AbstractPeepholeOptimization#endTraversal
    fn end_traversal(&mut self) {
        let class_name = self.get_class_name();
        self.fields_mut().end_traversal(class_name);
    }

    /// Returns whether the node may create new mutable state, or change existing state.
    // port: AbstractPeepholeOptimization#mayEffectMutableState
    fn may_effect_mutable_state(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        self.fields()
            .ast_analyzer()
            .may_effect_mutable_state(compiler, n)
    }

    /// Returns whether the node may have side effects when executed.
    // port: AbstractPeepholeOptimization#mayHaveSideEffects
    fn may_have_side_effects(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        self.fields()
            .ast_analyzer()
            .may_have_side_effects(compiler, n)
    }

    /// Returns the number value of the node if it has one and it cannot have side effects.
    ///
    /// Returns `None` otherwise.
    // port: AbstractPeepholeOptimization#getSideEffectFreeNumberValue
    fn get_side_effect_free_number_value(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<f64> {
        let mut value = NodeUtil::get_number_value(compiler, n);
        // Calculating the number value, if any, is likely to be faster than calculating side
        // effects, and there are only a very few cases where we can compute a number value, but
        // there could also be side effects. e.g. `void doSomething()` has value NaN, regardless
        // of the behavior of `doSomething()`
        if value.is_some()
            && self
                .fields()
                .ast_analyzer()
                .may_have_side_effects(compiler, n)
        {
            value = None;
        }
        value
    }

    // port: AbstractPeepholeOptimization#getSideEffectFreeNumberValueNoConversion
    fn get_side_effect_free_number_value_no_conversion(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<f64> {
        let mut value = NodeUtil::get_number_value_no_conversions(compiler, n);
        if value.is_some()
            && self
                .fields()
                .ast_analyzer()
                .may_have_side_effects(compiler, n)
        {
            value = None;
        }
        value
    }

    /// Returns the bigint value of the node if it has one and it cannot have side effects.
    ///
    /// Returns `None` otherwise.
    // port: AbstractPeepholeOptimization#getSideEffectFreeBigIntValue
    fn get_side_effect_free_big_int_value(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<BigInt> {
        let mut value = NodeUtil::get_big_int_value(compiler, n);
        // Calculating the bigint value, if any, is likely to be faster than calculating side
        // effects, and there are only a very few cases where we can compute a bigint value, but
        // there could also be side effects. e.g. `void doSomething()` has value NaN, regardless
        // of the behavior of `doSomething()`
        if value.is_some()
            && self
                .fields()
                .ast_analyzer()
                .may_have_side_effects(compiler, n)
        {
            value = None;
        }
        value
    }

    /// Gets the value of a node as a String, or `None` if it cannot be converted.
    ///
    /// This method effectively emulates the `String()` JavaScript cast function when possible
    /// and the node has no side effects. Otherwise, it returns `None`.
    // port: AbstractPeepholeOptimization#getSideEffectFreeStringValue
    fn get_side_effect_free_string_value(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<closure_rhino::js_string::JsString> {
        let mut value = NodeUtil::get_string_value(compiler, n);
        // Calculating the string value, if any, is likely to be faster than calculating side
        // effects, and there are only a very few cases where we can compute a string value, but
        // there could also be side effects. e.g. `void doSomething()` has value 'undefined',
        // regardless of the behavior of `doSomething()`
        if value.is_some()
            && self
                .fields()
                .ast_analyzer()
                .may_have_side_effects(compiler, n)
        {
            value = None;
        }
        value
    }

    /// Calculate the known boolean value for a node if possible and if it has no side effects.
    ///
    /// Returns `Tri::UNKNOWN` if the node has side effects or its value cannot be statically
    /// determined.
    // port: AbstractPeepholeOptimization#getSideEffectFreeBooleanValue
    fn get_side_effect_free_boolean_value(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Tri {
        let mut value = NodeUtil::get_boolean_value(compiler, n);
        // Calculating the boolean value, if any, is likely to be faster than calculating side
        // effects, and there are only a very few cases where we can compute a boolean value, but
        // there could also be side effects. e.g. `void doSomething()` has value `false`,
        // regardless of the behavior of `doSomething()`
        if value != Tri::UNKNOWN
            && self
                .fields()
                .ast_analyzer()
                .may_have_side_effects(compiler, n)
        {
            value = Tri::UNKNOWN;
        }
        value
    }

    /// Returns true if the current node's type implies side effects.
    ///
    /// This is a non-recursive version of the may have side effects check; used to check
    /// wherever the current node's type is one of the reason's why a subtree has side effects.
    // port: AbstractPeepholeOptimization#nodeTypeMayHaveSideEffects
    fn node_type_may_have_side_effects(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        self.fields()
            .ast_analyzer()
            .node_type_may_have_side_effects(compiler, n)
    }

    /// Returns whether the output language is ECMAScript 5 or later. Workarounds for quirks in
    /// browsers that do not support ES5 can be ignored when this is true.
    // port: AbstractPeepholeOptimization#isEcmaScript5OrGreater
    fn is_ecma_script5_or_greater(&self, compiler: &AbstractCompiler) -> bool {
        self.fields().compiler_set
            && compiler
                .get_options()
                .get_output_feature_set()
                .contains(FeatureSet::ES5)
    }

    /// Returns the current coding convention.
    // port: AbstractPeepholeOptimization#getCodingConvention
    fn get_coding_convention<'c>(
        &self,
        compiler: &'c AbstractCompiler,
    ) -> &'c dyn CodingConvention {
        // Note: this assumes a thread safe coding convention object.
        self.fields().check_compiler();
        compiler.get_coding_convention()
    }

    // port: AbstractPeepholeOptimization#reportChangeToEnclosingScope
    fn report_change_to_enclosing_scope(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.fields().check_compiler();
        compiler.report_change_to_enclosing_scope(n);
    }

    /// Calls `NodeUtil#deleteNode(Node, AbstractCompiler)`
    // port: AbstractPeepholeOptimization#deleteNode
    fn delete_node(&self, compiler: &mut AbstractCompiler, property: NodeId) {
        self.fields().check_compiler();
        NodeUtil::delete_node(compiler, property);
    }

    /// Calls `NodeUtil#markFunctionsDeleted(Node, AbstractCompiler)`
    // port: AbstractPeepholeOptimization#markFunctionsDeleted
    fn mark_functions_deleted(&self, compiler: &mut AbstractCompiler, function: NodeId) {
        self.fields().check_compiler();
        NodeUtil::mark_functions_deleted(compiler, function);
    }

    /// Calls `NodeUtil#markNewScopesChanged(Node, AbstractCompiler)`
    // port: AbstractPeepholeOptimization#markNewScopesChanged
    fn mark_new_scopes_changed(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.fields().check_compiler();
        NodeUtil::mark_new_scopes_changed(compiler, n);
    }

    /// Calls `NodeUtil#addFeatureToScript` with the script currently being visited.
    // port: AbstractPeepholeOptimization#addFeatureToEnclosingScript
    fn add_feature_to_enclosing_script(&mut self, feature: Feature) {
        // NOTE: we could implement this as
        //  protected final void addFeatureToEnclosingScript(Node node, Feature feature) {
        //     NodeUtil.addFeatureToScript(NodeUtil.getEnclosingScript(node), ...
        // This is expected to be behaviorally equivalent.
        // However, walking the AST to get the enclosing script is O(depth of the AST) per call.
        // With the  current implementation, we let PeepholeOptimizationsPass to find the script
        // in what's usually O(1) time, or worst case it only calls getEnclosingScript() once per
        // NodeTraversal and then caches the result.
        self.fields_mut().new_features.insert(feature);
    }

    // port: AbstractPeepholeOptimization#getNewFeatures
    fn get_new_features(&self) -> Vec<Feature> {
        self.fields().new_features.iter().copied().collect()
    }

    // port: AbstractPeepholeOptimization#clearNewFeatures
    fn clear_new_features(&mut self) {
        self.fields_mut().new_features.clear();
    }
}

// Rust-only: Java's `LinkedHashSet<Feature>.toString()` (`[a, b]` with each feature's toString).
fn format_feature_set(features: &IndexSet<Feature>) -> String {
    let items: Vec<String> = features.iter().map(|f| f.to_string()).collect();
    format!("[{}]", items.join(", "))
}
