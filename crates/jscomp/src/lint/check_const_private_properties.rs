/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckConstPrivateProperties.java.

//! This pass looks for properties that are not modified and ensures they use the @const
//! annotation.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    js_string::JsString,
    jsdoc_info::Visibility,
    node::{Ast, NodeId},
    token::Token,
};

// port: CheckConstPrivateProperties#MISSING_CONST_PROPERTY
pub static MISSING_CONST_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_CONST_PROPERTY",
    "Private property {0} is never modified, use the @const annotation",
);

pub struct CheckConstPrivateProperties {
    candidates: Vec<NodeId>,
    modified: IndexSet<JsString>,
    constructors_and_interfaces: IndexSet<JsString>,
}

impl CheckConstPrivateProperties {
    // port: CheckConstPrivateProperties#CheckConstPrivateProperties
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            candidates: Vec::new(),
            modified: IndexSet::<_>::default(),
            constructors_and_interfaces: IndexSet::<_>::default(),
        }
    }

    /// Reports the property definitions that should use the @const annotation.
    // port: CheckConstPrivateProperties#reportMissingConst
    fn report_missing_const(&mut self, t: &mut NodeTraversal<'_>) {
        for &n in &self.candidates {
            let prop_name = n.get_string(t);
            if !self.modified.contains(&prop_name) {
                t.report(n, &MISSING_CONST_PROPERTY, &[&prop_name.to_string()]);
            }
        }
        self.candidates.clear();
        self.modified.clear();
    }

    // port: CheckConstPrivateProperties#recordConstructorOrInterface
    fn record_constructor_or_interface(&mut self, ast: &Ast, n: NodeId) {
        let class_name = NodeUtil::get_best_l_value_name(ast, NodeUtil::get_best_l_value(ast, n));
        if let Some(class_name) = class_name {
            // If className is null, then this pass won't report any diagnostics on static members
            // of this class.
            self.constructors_and_interfaces.insert(class_name);
        }
    }

    /// @return Whether the given node is a @private property declaration that is not marked
    /// constant.
    // port: CheckConstPrivateProperties#isCandidatePropertyDefinition
    fn is_candidate_property_definition(&self, ast: &Ast, n: NodeId) -> bool {
        if !NodeUtil::is_lhs_of_assign(ast, n) {
            return false;
        }
        let target = n.get_first_child(ast).unwrap();
        // Check whether the given property access is on 'this' or a static property on a class.
        if !(target.is_this(ast)
            || target
                .get_qualified_name(ast)
                .is_some_and(|q| self.constructors_and_interfaces.contains(&q)))
        {
            return false;
        }
        let info = NodeUtil::get_best_jsdoc_info(ast, n);
        info.is_some_and(|info| {
            info.get_visibility() == Visibility::PRIVATE
                && !info.is_constant()
                && !info.has_typedef_type()
                && !info.has_enum_parameter_type()
                && !info.is_interface()
                && !Self::is_function_property(ast, n)
        })
    }

    /// @return Whether the given property declaration is assigned to a function.
    // port: CheckConstPrivateProperties#isFunctionProperty
    fn is_function_property(ast: &Ast, n: NodeId) -> bool {
        // TODO(dylandavidson): getAssignedValue does not support GETELEM.
        if n.is_get_elem(ast) {
            return false;
        }
        let assigned_value = NodeUtil::get_assigned_value(ast, n);
        assigned_value.is_some_and(|v| v.is_function(ast))
    }

    /// @return Whether the given property is modified in any way (assignment,
    /// increment/decrement, or 'delete' property).
    // port: CheckConstPrivateProperties#isModificationOp
    fn is_modification_op(ast: &Ast, n: NodeId) -> bool {
        NodeUtil::is_l_value(ast, n) || n.get_parent(ast).unwrap().is_del_prop(ast)
    }
}

impl CompilerPass for CheckConstPrivateProperties {
    // port: CheckConstPrivateProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckConstPrivateProperties {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckConstPrivateProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            // Exiting the script, report any non-const privates not modified in the file.
            Token::SCRIPT => self.report_missing_const(t),
            Token::GETELEM | Token::GETPROP => {
                let prop_node = if n.is_get_prop(t) {
                    n
                } else if n.get_last_child(t).unwrap().is_string_lit(t) {
                    n.get_last_child(t).unwrap()
                } else {
                    return;
                };

                // Only consider non-const @private class properties as candidates
                if self.is_candidate_property_definition(t, n) {
                    self.candidates.push(prop_node);
                } else if Self::is_modification_op(t, n) {
                    // Mark any other modification operation as a modified property, to deal with
                    // lambdas, etc
                    self.modified.insert(prop_node.get_string(t));
                }
            }
            Token::FUNCTION => {
                let info = NodeUtil::get_best_jsdoc_info(t, n);
                if info.is_some_and(|info| info.is_constructor() || info.is_interface()) {
                    self.record_constructor_or_interface(t, n);
                }
            }
            Token::CLASS => self.record_constructor_or_interface(t, n),
            _ => {}
        }
    }
}
