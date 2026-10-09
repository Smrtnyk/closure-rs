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
//   src/com/google/javascript/jscomp/PeepholeCollectPropertyAssignments.java.

//! Port of `PeepholeCollectPropertyAssignments.java`.
//!
//! A pass that looks for assignments to properties of an object or array immediately following
//! its creation using the abbreviated syntax.
//!
//! E.g. `var a = [];a[0] = 0` is optimized to `var a = [0]` and similarly for the object
//! constructor.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

#[derive(Default)]
pub struct PeepholeCollectPropertyAssignments {
    fields: AbstractPeepholeOptimizationFields,
}

impl PeepholeCollectPropertyAssignments {
    // port: PeepholeCollectPropertyAssignments#PeepholeCollectPropertyAssignments
    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPeepholeOptimization for PeepholeCollectPropertyAssignments {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.PeepholeCollectPropertyAssignments"
    }

    // port: PeepholeCollectPropertyAssignments#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        if !subtree.is_script(compiler) && !subtree.is_block(compiler) {
            return Some(subtree);
        }

        let mut code_changed = false;

        // Look for variable declarations or simple assignments
        // and start processing there.
        let mut child = subtree.get_first_child(compiler);
        while let Some(cur) = child {
            if !NodeUtil::is_name_declaration(compiler, Some(cur))
                && !NodeUtil::is_expr_assign(compiler, cur)
            {
                child = cur.get_next(compiler);
                continue;
            }
            if !Self::is_property_assignment_to_name(compiler, cur.get_next(compiler)) {
                // Quick check to see if there's anything to collapse.
                child = cur.get_next(compiler);
                continue;
            }

            check_state!(cur.has_one_child(compiler));
            let name = Self::get_name(compiler, cur);
            if !name.is_name(compiler) {
                // The assignment target is not a simple name.
                child = cur.get_next(compiler);
                continue;
            }

            let value = Self::get_value(compiler, cur);
            let Some(value) = value.filter(|value| Self::is_interesting_value(compiler, *value))
            else {
                // No initializer or not an Object or Array literal.
                child = cur.get_next(compiler);
                continue;
            };

            while let Some(property_candidate) = cur.get_next(compiler) {
                // This does not infinitely loop because collectProperty always
                // removes propertyCandidate from its parent when it returns true.
                let name_string = name.get_string(compiler);
                if !self.collect_property(compiler, property_candidate, &name_string, value) {
                    break;
                }
                code_changed = true;
            }
            child = cur.get_next(compiler);
        }

        if code_changed {
            self.report_change_to_enclosing_scope(compiler, subtree);
        }
        Some(subtree)
    }
}

impl PeepholeCollectPropertyAssignments {
    // port: PeepholeCollectPropertyAssignments#getName
    fn get_name(ast: &Ast, n: NodeId) -> NodeId {
        if NodeUtil::is_name_declaration(ast, Some(n)) {
            return n.get_first_child(ast).unwrap();
        } else if NodeUtil::is_expr_assign(ast, n) {
            return n.get_first_first_child(ast).unwrap();
        }
        // throw new IllegalStateException()
        panic!("");
    }

    // port: PeepholeCollectPropertyAssignments#getValue
    fn get_value(ast: &Ast, n: NodeId) -> Option<NodeId> {
        if NodeUtil::is_name_declaration(ast, Some(n)) {
            return n.get_first_first_child(ast);
        } else if NodeUtil::is_expr_assign(ast, n) {
            return n.get_first_child(ast).unwrap().get_last_child(ast);
        }
        // throw new IllegalStateException()
        panic!("");
    }

    // port: PeepholeCollectPropertyAssignments#isInterestingValue
    pub fn is_interesting_value(ast: &Ast, n: NodeId) -> bool {
        n.is_object_lit(ast) || n.is_array_lit(ast)
    }

    // port: PeepholeCollectPropertyAssignments#isPropertyAssignmentToName
    fn is_property_assignment_to_name(ast: &Ast, property_candidate: Option<NodeId>) -> bool {
        let Some(property_candidate) = property_candidate else {
            return false;
        };

        // Must be an assignment...
        if !NodeUtil::is_expr_assign(ast, property_candidate) {
            return false;
        }

        let expr = property_candidate.get_first_child(ast).unwrap();

        // to a property...
        let lhs = expr.get_first_child(ast).unwrap();
        if !NodeUtil::is_normal_get(ast, lhs) {
            return false;
        }

        // of a variable.
        let obj = lhs.get_first_child(ast).unwrap();
        obj.is_name(ast)
    }

    // port: PeepholeCollectPropertyAssignments#collectProperty
    fn collect_property(
        &self,
        compiler: &mut AbstractCompiler,
        property_candidate: NodeId,
        name: &JsString,
        value: NodeId,
    ) -> bool {
        if !Self::is_property_assignment_to_name(compiler, Some(property_candidate)) {
            return false;
        }

        let lhs = property_candidate.get_first_first_child(compiler).unwrap();
        // Must be an assignment to the recent variable...
        if *name != lhs.get_first_child(compiler).unwrap().get_string(compiler) {
            return false;
        }

        let rhs = lhs.get_next(compiler).unwrap();
        // with a value that cannot change the values of the variables,
        if self.may_have_side_effects(compiler, rhs)
            || NodeUtil::can_be_side_effected(compiler, rhs)
        {
            return false;
        }

        // and does not have a reference to a variable initialized after it.
        if !NodeUtil::is_literal_value(compiler, rhs, true)
            && Self::might_contain_forward_reference(compiler, rhs, name)
        {
            return false;
        }

        match value.get_token(compiler) {
            Token::ARRAYLIT => {
                if !Self::collect_array_property(compiler, value, property_candidate) {
                    return false;
                }
            }
            Token::OBJECTLIT => {
                if !self.collect_object_property(compiler, value, property_candidate) {
                    return false;
                }
            }
            // throw new IllegalStateException()
            _ => panic!(""),
        }
        true
    }

    // port: PeepholeCollectPropertyAssignments#collectArrayProperty
    fn collect_array_property(
        ast: &mut Ast,
        array_literal: NodeId,
        property_candidate: NodeId,
    ) -> bool {
        let assignment = property_candidate.get_first_child(ast).unwrap();
        let size_of_array_at_start: i32 = array_literal.get_child_count(ast);
        let mut max_index_assigned: i32 = size_of_array_at_start.wrapping_sub(1);

        let lhs = assignment.get_first_child(ast).unwrap();
        let rhs = lhs.get_next(ast).unwrap();
        if !lhs.is_get_elem(ast) {
            return false;
        }
        let obj = lhs.get_first_child(ast).unwrap();
        let property = obj.get_next(ast).unwrap();
        // The left hand side must have a numeric index
        if !property.is_number(ast) {
            return false;
        }
        // that is a valid array index
        let dindex: f64 = property.get_double(ast);
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(dindex >= 0.0) // Handles NaN and negatives.
            || dindex.is_infinite()
            || dindex > 0x7fffffff_i64 as f64
        {
            return false;
        }
        let index: i32 = dindex as i32;
        if dindex != index as f64 {
            return false;
        }
        // that would not make the array so sparse that they take more space
        // when rendered than x[9]=1.
        if max_index_assigned.wrapping_add(4) < index {
            return false;
        }

        // If the array literal contains a spread element, its runtime length and element
        // offsets are unknown, so subsequent indexed property assignments cannot safely
        // be appended to or folded into the literal.
        if Self::array_literal_has_spread(ast, array_literal) {
            return false;
        }

        if index > max_index_assigned {
            while max_index_assigned < index.wrapping_sub(1) {
                // Pad the array if it is sparse.
                // So if array is [0] and integer 3 is assigned at index is 2, then
                // we want to produce [0,,2].
                let empty_node = IR::empty(ast).srcref(ast, array_literal);
                array_literal.add_child_to_back(ast, empty_node);
                max_index_assigned += 1;
            }
            let detached = rhs.detach(ast);
            array_literal.add_child_to_back(ast, detached);
        } else {
            // An out of order assignment.  Allow it if it's a hole.
            let current_value = array_literal.get_child_at_index(ast, index).unwrap();
            if !current_value.is_empty(ast) {
                // We've already collected a value for this index.
                return false;
            }
            let detached = rhs.detach(ast);
            current_value.replace_with(ast, detached);
        }

        property_candidate.detach(ast);
        true
    }

    // port: PeepholeCollectPropertyAssignments#arrayLiteralHasSpread
    fn array_literal_has_spread(ast: &Ast, array_literal: NodeId) -> bool {
        let script = NodeUtil::get_enclosing_script(ast, array_literal);
        let script_features =
            script.and_then(|script| NodeUtil::get_feature_set_of_script(ast, script));
        if let Some(script_features) = script_features
            && !script_features.has(Feature::SPREAD_EXPRESSIONS)
        {
            return false;
        }
        let mut child = array_literal.get_first_child(ast);
        while let Some(cur) = child {
            if cur.is_spread(ast) {
                return true;
            }
            child = cur.get_next(ast);
        }
        false
    }

    // port: PeepholeCollectPropertyAssignments#collectObjectProperty
    fn collect_object_property(
        &self,
        compiler: &mut AbstractCompiler,
        object_literal: NodeId,
        property_candidate: NodeId,
    ) -> bool {
        let assignment = property_candidate.get_first_child(compiler).unwrap();
        let lhs = assignment.get_first_child(compiler).unwrap();
        let rhs = lhs.get_next(compiler).unwrap();
        let obj = lhs.get_first_child(compiler).unwrap();
        check_state!(
            lhs.is_get_prop(compiler) || lhs.is_get_elem(compiler),
            "%s",
            lhs.to_string(compiler)
        );

        // The property must be statically known.
        let property_name: Option<JsString>;
        if lhs.is_get_elem(compiler) {
            let property = obj.get_next(compiler).unwrap();
            if property.is_number(compiler) {
                property_name = self.get_side_effect_free_string_value(compiler, property);
            } else if property.is_string_lit(compiler) {
                property_name = Some(property.get_string(compiler));
            } else {
                return false;
            }
        } else if lhs.is_get_prop(compiler) {
            property_name = Some(lhs.get_string(compiler));
        } else {
            return false;
        }

        // Check if the new property already exists in the object literal
        // Note: Duplicate keys are invalid in strict mode
        let mut existing_property: Option<NodeId> = None;
        let mut current_property = object_literal.get_first_child(compiler);
        while let Some(cur) = current_property {
            if cur.is_string_key(compiler) || cur.is_member_function_def(compiler) {
                // Get the name of the current property
                let current_property_name = cur.get_string(compiler);
                // Get the value of the property
                let current_value = cur.get_first_child(compiler).unwrap();
                // Compare the current property name with the new property name
                if Some(&current_property_name) == property_name.as_ref() {
                    existing_property = Some(cur);

                    // Check if the current value and the new value are side-effect
                    let is_current_value_side_effect =
                        NodeUtil::can_be_side_effected(compiler, current_value);
                    let is_new_value_side_effect = NodeUtil::can_be_side_effected(compiler, rhs);

                    // If they are side-effect free then replace the current value with the new
                    // one
                    if is_current_value_side_effect || is_new_value_side_effect {
                        return false;
                    }
                    // Break the loop if the property exists
                    break;
                }
            } else if cur.is_getter_def(compiler) || cur.is_setter_def(compiler) {
                let current_property_name = cur.get_string(compiler);
                if Some(&current_property_name) == property_name.as_ref() {
                    return false;
                }
            }
            current_property = cur.get_next(compiler);
        }

        let new_property = IR::string_key(compiler, property_name.unwrap())
            .srcref_if_missing(compiler, property_candidate);
        // Preserve the quotedness of a property reference
        if lhs.is_get_elem(compiler) {
            new_property.set_quoted_string_key(compiler);
        }
        let new_value = rhs.detach(compiler);
        new_property.add_child_to_back(compiler, new_value);

        if let Some(existing_property) = existing_property {
            self.delete_node(compiler, existing_property);
        }
        // If the property does not already exist we can safely add it
        object_literal.add_child_to_back(compiler, new_property);
        property_candidate.detach(compiler);
        true
    }

    // port: PeepholeCollectPropertyAssignments#mightContainForwardReference
    fn might_contain_forward_reference(ast: &Ast, node: NodeId, var_name: &JsString) -> bool {
        if node.is_name(ast) {
            return *var_name == node.get_string(ast);
        }
        let mut child = node.get_first_child(ast);
        while let Some(cur) = child {
            if Self::might_contain_forward_reference(ast, cur, var_name) {
                return true;
            }
            child = cur.get_next(ast);
        }
        false
    }
}
