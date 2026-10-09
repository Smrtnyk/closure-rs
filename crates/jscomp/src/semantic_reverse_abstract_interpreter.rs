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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/type/SemanticReverseAbstractInterpreter.java.

//! A reverse abstract interpreter using the semantics of the JavaScript language as a means to
//! reverse interpret computations. This interpreter expects the parse tree inputs to be typed.
//!
//! Java's `com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter`.
use crate::{
    abstract_compiler::AbstractCompiler,
    chainable_reverse_abstract_interpreter::{
        ChainLinks, ChainableReverseAbstractInterpreter, RestrictByTypeOfResultVisitor,
        RestrictByTypeOfResultVisitorKind, RestrictToArrayVisitor, RestrictToNotArrayVisitor,
        visit,
    },
    flow_scope::{FlowScope, FlowSlot},
    graph::{annotation::Annotation, lattice_element::LatticeElement},
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    typed_scope::TypedScope,
};
use closure_jstype::{
    JSTypeNative, JSTypeRegistry, TypeId,
    js_type::{TypePair, to_maybe_function_type},
    object_type,
    prelude::{FunctionType, JSType, UnionType},
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    outcome::Outcome,
    token::Token,
};
use std::{
    any::Any,
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};

/// Java's `Function<TypePair, TypePair>` merging functions.
type Merging = fn(&mut JSTypeRegistry, &Ast, TypePair) -> Option<TypePair>;

pub struct SemanticReverseAbstractInterpreter {
    links: ChainLinks,
}

impl SemanticReverseAbstractInterpreter {
    // port: SemanticReverseAbstractInterpreter#eq
    /// Merging function for equality between types.
    fn eq(reg: &mut JSTypeRegistry, ast: &Ast, p: TypePair) -> Option<TypePair> {
        let (Some(type_a), Some(type_b)) = (p.type_a, p.type_b) else {
            return None;
        };
        Some(type_a.get_types_under_equality(reg, ast, type_b))
    }

    // port: SemanticReverseAbstractInterpreter#ne
    /// Merging function for non-equality between types.
    fn ne(reg: &mut JSTypeRegistry, ast: &Ast, p: TypePair) -> Option<TypePair> {
        let (Some(type_a), Some(type_b)) = (p.type_a, p.type_b) else {
            return None;
        };
        Some(type_a.get_types_under_inequality(reg, ast, type_b))
    }

    // port: SemanticReverseAbstractInterpreter#sheq
    /// Merging function for strict equality between types.
    fn sheq(reg: &mut JSTypeRegistry, ast: &Ast, p: TypePair) -> Option<TypePair> {
        let (Some(type_a), Some(type_b)) = (p.type_a, p.type_b) else {
            return None;
        };
        Some(type_a.get_types_under_shallow_equality(reg, ast, type_b))
    }

    // port: SemanticReverseAbstractInterpreter#shne
    /// Merging function for strict non-equality between types.
    fn shne(reg: &mut JSTypeRegistry, ast: &Ast, p: TypePair) -> Option<TypePair> {
        let (Some(type_a), Some(type_b)) = (p.type_a, p.type_b) else {
            return None;
        };
        Some(type_a.get_types_under_shallow_inequality(reg, ast, type_b))
    }

    // port: SemanticReverseAbstractInterpreter#ineq
    /// Merging function for inequality comparisons between types.
    fn ineq(reg: &mut JSTypeRegistry, ast: &Ast, p: TypePair) -> Option<TypePair> {
        let type_a = p
            .type_a
            .map(|type_a| type_a.restrict_by_not_undefined(reg, ast));
        let type_b = p
            .type_b
            .map(|type_b| type_b.restrict_by_not_undefined(reg, ast));
        Some(TypePair::new(type_a, type_b))
    }

    // port: SemanticReverseAbstractInterpreter#SemanticReverseAbstractInterpreter
    /// Creates a semantic reverse abstract interpreter.
    pub fn new(_type_registry: &JSTypeRegistry) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| Self {
            links: ChainLinks::new(this.clone()),
        })
    }

    // port: SemanticReverseAbstractInterpreter#caseIsArray
    #[must_use]
    fn case_is_array(
        &self,
        compiler: &mut AbstractCompiler,
        value: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        let type_ = self.get_type_if_refinable(compiler, value, &blind_scope);
        if let Some(type_) = type_ {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let restricted = if outcome.is_truthy() {
                visit(type_, reg, ast, &mut RestrictToArrayVisitor)
            } else {
                visit(type_, reg, ast, &mut RestrictToNotArrayVisitor)
            };
            return self.maybe_restrict_name(compiler, blind_scope, value, Some(type_), restricted);
        }
        blind_scope
    }

    // port: SemanticReverseAbstractInterpreter#caseEquality
    #[must_use]
    fn case_equality(
        &self,
        compiler: &mut AbstractCompiler,
        condition: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        merging: Merging,
    ) -> Arc<dyn FlowScope> {
        let left = condition.get_first_child(&compiler.ast).unwrap();
        let right = condition.get_last_child(&compiler.ast).unwrap();
        self.case_equality_of(compiler, left, right, blind_scope, merging)
    }

    // port: SemanticReverseAbstractInterpreter#caseEquality
    /// Java's `caseEquality(Node left, Node right, FlowScope blindScope, Function merging)`.
    #[must_use]
    fn case_equality_of(
        &self,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        merging: Merging,
    ) -> Arc<dyn FlowScope> {
        // left type
        let mut left_type = self.get_type_if_refinable(compiler, left, &blind_scope);
        let left_is_refineable;
        if left_type.is_some() {
            left_is_refineable = true;
        } else {
            left_is_refineable = false;
            left_type = left.get_jstype(&compiler.ast);
        }

        // right type
        let mut right_type = self.get_type_if_refinable(compiler, right, &blind_scope);
        let right_is_refineable;
        if right_type.is_some() {
            right_is_refineable = true;
        } else {
            right_is_refineable = false;
            right_type = right.get_jstype(&compiler.ast);
        }

        // merged types
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let merged = merging(reg, ast, TypePair::new(left_type, right_type));

        // creating new scope
        if let Some(merged) = merged {
            return self.maybe_restrict_two_names(
                compiler,
                blind_scope,
                left,
                left_type,
                if left_is_refineable {
                    merged.type_a
                } else {
                    None
                },
                right,
                right_type,
                if right_is_refineable {
                    merged.type_b
                } else {
                    None
                },
            );
        }
        blind_scope
    }

    // port: SemanticReverseAbstractInterpreter#caseAndOrNotShortCircuiting
    #[must_use]
    fn case_and_or_not_short_circuiting(
        &self,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
        mut blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        // left type
        let mut left_type = self.get_type_if_refinable(compiler, left, &blind_scope);
        let left_is_refineable;
        if left_type.is_some() {
            left_is_refineable = true;
        } else {
            left_is_refineable = false;
            left_type = left.get_jstype(&compiler.ast);
            blind_scope = self.first_preciser_scope_knowing_condition_outcome(
                compiler,
                left,
                blind_scope,
                outcome,
            );
        }

        // restricting left type
        let restricted_left_type = left_type.map(|left_type| {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            left_type.get_restricted_type_given_outcome(reg, ast, outcome)
        });
        if restricted_left_type.is_none() {
            return self.first_preciser_scope_knowing_condition_outcome(
                compiler,
                right,
                blind_scope,
                outcome,
            );
        }
        blind_scope = self.maybe_restrict_name(
            compiler,
            blind_scope,
            left,
            left_type,
            if left_is_refineable {
                restricted_left_type
            } else {
                None
            },
        );

        // right type
        let mut right_type = self.get_type_if_refinable(compiler, right, &blind_scope);
        let right_is_refineable;
        if right_type.is_some() {
            right_is_refineable = true;
        } else {
            right_is_refineable = false;
            right_type = right.get_jstype(&compiler.ast);
            blind_scope = self.first_preciser_scope_knowing_condition_outcome(
                compiler,
                right,
                blind_scope,
                outcome,
            );
        }

        if outcome.is_truthy() {
            let restricted_right_type = right_type.map(|right_type| {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                right_type.get_restricted_type_given_outcome(reg, ast, outcome)
            });
            // creating new scope
            return self.maybe_restrict_name(
                compiler,
                blind_scope,
                right,
                right_type,
                if right_is_refineable {
                    restricted_right_type
                } else {
                    None
                },
            );
        }
        blind_scope
    }

    // port: SemanticReverseAbstractInterpreter#caseAndOrMaybeShortCircuiting
    #[must_use]
    fn case_and_or_maybe_short_circuiting(
        &self,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
        mut blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        // Perform two separate refinements, one for if short-circuiting occurred, and one for if it
        // did not.  Because it's not clear whether short-circuiting occurred, we actually have to
        // ignore both separate result flow scopes individually, but if they both refined the same
        // slot, we can join the two refinements.  TODO(sdh): look into simplifying this.  If
        // joining were more efficient, we should just be able to join the scopes unconditionally?
        let refinements: Refinements = Arc::new(Mutex::new(IndexSet::<_>::default()));
        blind_scope = Arc::new(RefinementTrackingFlowScope::new(
            blind_scope,
            refinements.clone(),
        ));
        let left_scope = self.first_preciser_scope_knowing_condition_outcome(
            compiler,
            left,
            blind_scope.clone(),
            outcome.not(),
        );
        let left_var = match single_refinement(&refinements) {
            Some(name) => left_scope.get_slot(compiler, &name),
            None => None,
        };
        let Some(left_var) = left_var else {
            // If we did create a more precise scope, blindScope has a child and
            // it is frozen. We can't just throw it away to return it. So we
            // must create a child instead.
            return unwrap(blind_scope);
        };
        lock(&refinements).clear();
        // Note: re-wrap the scope, in case it was unwrapped by a nested call to this method.
        let mut right_scope: Arc<dyn FlowScope> = Arc::new(RefinementTrackingFlowScope::new(
            self.first_preciser_scope_knowing_condition_outcome(
                compiler,
                left,
                blind_scope.clone(),
                outcome,
            ),
            refinements.clone(),
        ));
        right_scope = self.first_preciser_scope_knowing_condition_outcome(
            compiler,
            right,
            right_scope,
            outcome.not(),
        );
        let right_var = match single_refinement(&refinements) {
            Some(name) => right_scope.get_slot(compiler, &name),
            None => None,
        };
        let Some(right_var) = right_var else {
            return unwrap(blind_scope);
        };
        if left_var.get_name(compiler) != right_var.get_name(compiler) {
            return unwrap(blind_scope);
        }
        let left_var_type = left_var.get_type(compiler).unwrap();
        let right_var_type = right_var.get_type(compiler).unwrap();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let type_ = left_var_type.get_least_supertype(reg, ast, right_var_type);
        let name = left_var.get_name(compiler);
        unwrap(blind_scope).infer_slot_type(compiler, &name, Some(type_))
    }

    // port: SemanticReverseAbstractInterpreter#maybeRestrictName
    /// If the restrictedType differs from the originalType, then we should branch the current flow
    /// scope and create a new flow scope with the name declared with the new type.
    ///
    /// We try not to create spurious child flow scopes as this makes type inference slower.
    ///
    /// We also do not want spurious slots around in type inference, because we use these as a
    /// signal for "checked unknown" types. A "checked unknown" type is a symbol that the programmer
    /// has already checked and verified that it's defined, even if we don't know what it is.
    ///
    /// It is OK to pass non-name nodes into this method, as long as you pass in `null` for a
    /// restricted type.
    #[must_use]
    fn maybe_restrict_name(
        &self,
        compiler: &mut AbstractCompiler,
        blind_scope: Arc<dyn FlowScope>,
        node: NodeId,
        original_type: Option<TypeId>,
        restricted_type: Option<TypeId>,
    ) -> Arc<dyn FlowScope> {
        if let Some(restricted_type) = restricted_type
            && Some(restricted_type) != original_type
        {
            return self.declare_name_in_scope(compiler, blind_scope, node, restricted_type);
        }
        blind_scope
    }

    // port: SemanticReverseAbstractInterpreter#maybeRestrictTwoNames
    /// See `maybe_restrict_name`.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // Java's signature.
    fn maybe_restrict_two_names(
        &self,
        compiler: &mut AbstractCompiler,
        blind_scope: Arc<dyn FlowScope>,
        left: NodeId,
        original_left_type: Option<TypeId>,
        restricted_left_type: Option<TypeId>,
        right: NodeId,
        original_right_type: Option<TypeId>,
        restricted_right_type: Option<TypeId>,
    ) -> Arc<dyn FlowScope> {
        let should_refine_left =
            restricted_left_type.is_some() && restricted_left_type != original_left_type;
        let should_refine_right =
            restricted_right_type.is_some() && restricted_right_type != original_right_type;
        if should_refine_left || should_refine_right {
            let mut informed = blind_scope;
            if should_refine_left {
                informed = self.declare_name_in_scope(
                    compiler,
                    informed,
                    left,
                    restricted_left_type.unwrap(),
                );
            }
            if should_refine_right {
                informed = self.declare_name_in_scope(
                    compiler,
                    informed,
                    right,
                    restricted_right_type.unwrap(),
                );
            }
            return informed;
        }
        blind_scope
    }

    // port: SemanticReverseAbstractInterpreter#caseNameOrGetProp
    #[must_use]
    fn case_name_or_get_prop(
        &self,
        compiler: &mut AbstractCompiler,
        name: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        let type_ = self.get_type_if_refinable(compiler, name, &blind_scope);
        if let Some(type_) = type_ {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let restricted_type = type_.get_restricted_type_given_outcome(reg, ast, outcome);
            return self.maybe_restrict_name(
                compiler,
                blind_scope,
                name,
                Some(type_),
                Some(restricted_type),
            );
        }
        blind_scope
    }

    // port: SemanticReverseAbstractInterpreter#caseTypeOf
    #[must_use]
    fn case_type_of(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        type_: TypeId,
        value: &JsString,
        result_equals_value: bool,
        blind_scope: Arc<dyn FlowScope>,
    ) -> Arc<dyn FlowScope> {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let restricted = self.get_restricted_by_type_of_result(
            reg,
            ast,
            Some(type_),
            value,
            result_equals_value,
        );
        self.maybe_restrict_name(compiler, blind_scope, node, Some(type_), restricted)
    }

    // port: SemanticReverseAbstractInterpreter#caseInstanceOf
    #[must_use]
    fn case_instance_of(
        &self,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        let left_type = self.get_type_if_refinable(compiler, left, &blind_scope);
        let Some(left_type) = left_type else {
            return blind_scope;
        };
        let right_type = right.get_jstype(&compiler.ast);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let mut target_type = reg.get_native_object_type(JSTypeNative::UNKNOWN_TYPE);
        if let Some(right_type) = right_type
            && right_type.is_function_type(reg)
        {
            target_type = right_type.to_maybe_function_type(reg).unwrap();
        }
        let restricted = if outcome.is_truthy() {
            visit(
                left_type,
                reg,
                ast,
                &mut RestrictByTrueInstanceOfResultVisitor::new(target_type),
            )
        } else {
            visit(
                left_type,
                reg,
                ast,
                &mut RestrictByFalseInstanceOfResultVisitor::new(target_type),
            )
        };
        self.maybe_restrict_name(compiler, blind_scope, left, Some(left_type), restricted)
    }

    // port: SemanticReverseAbstractInterpreter#caseIn
    /// Given 'property in object', ensures that the object has the property in the informed scope
    /// by defining it as a qualified name if the object type lacks the property and it's not in the
    /// blind scope.
    ///
    /// `object` is the node of the right-side of the in; `property_name` the string of the
    /// left-side of the in.
    #[must_use]
    fn case_in(
        &self,
        compiler: &mut AbstractCompiler,
        object: NodeId,
        property_name: &JsString,
        blind_scope: Arc<dyn FlowScope>,
    ) -> Arc<dyn FlowScope> {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let mut js_type = object.get_jstype(ast);
        js_type = js_type.map(|js_type| js_type.restrict_by_not_null_or_undefined(reg, ast));

        let mut has_property = false;
        let object_type = object_type::cast(reg, js_type);
        if let Some(object_type) = object_type {
            has_property = object_type.has_property(reg, ast, property_name);
        }
        if !has_property {
            let qualified_name = object.get_qualified_name(&compiler.ast);
            if let Some(qualified_name) = qualified_name {
                let property_qualified_name = qualified_name
                    .concat(&JsString::from("."))
                    .concat(property_name);
                if blind_scope
                    .get_slot(compiler, &property_qualified_name)
                    .is_none()
                {
                    let unknown_type = compiler
                        .get_type_registry()
                        .get_native_type(JSTypeNative::UNKNOWN_TYPE);
                    return blind_scope.infer_qualified_slot(
                        compiler,
                        object,
                        &property_qualified_name,
                        Some(unknown_type),
                        unknown_type,
                        false,
                    );
                }
            }
        }
        blind_scope
    }
}

impl ReverseAbstractInterpreter for SemanticReverseAbstractInterpreter {
    // port: SemanticReverseAbstractInterpreter#getPreciserScopeKnowingConditionOutcome
    fn get_preciser_scope_knowing_condition_outcome(
        &self,
        compiler: &mut AbstractCompiler,
        condition: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        // Check for the typeof operator.
        let operator_token = condition.get_token(&compiler.ast);
        match operator_token {
            Token::EQ | Token::NE | Token::SHEQ | Token::SHNE | Token::CASE => {
                let ast = &compiler.ast;
                let left;
                let right;
                if operator_token == Token::CASE {
                    let parent = condition.get_parent(ast).unwrap();
                    left = parent.get_previous(ast).unwrap(); // the switch condition
                    check_state!(parent.is_switch_body(ast), &parent.to_string(ast));
                    right = condition.get_first_child(ast).unwrap();
                } else {
                    left = condition.get_first_child(ast).unwrap();
                    right = condition.get_last_child(ast).unwrap();
                }

                let mut type_of_node = None;
                let mut string_node = None;
                if left.is_type_of(ast) && right.is_string_lit(ast) {
                    type_of_node = Some(left);
                    string_node = Some(right);
                } else if right.is_type_of(ast) && left.is_string_lit(ast) {
                    type_of_node = Some(right);
                    string_node = Some(left);
                }
                if let (Some(type_of_node), Some(string_node)) = (type_of_node, string_node) {
                    let operand_node = type_of_node.get_first_child(ast).unwrap();
                    let operand_type =
                        self.get_type_if_refinable(compiler, operand_node, &blind_scope);
                    if let Some(operand_type) = operand_type {
                        let mut result_equals_value = operator_token == Token::EQ
                            || operator_token == Token::SHEQ
                            || operator_token == Token::CASE;
                        if !outcome.is_truthy() {
                            result_equals_value = !result_equals_value;
                        }
                        let value = string_node.get_string(&compiler.ast);
                        return self.case_type_of(
                            compiler,
                            operand_node,
                            operand_type,
                            &value,
                            result_equals_value,
                            blind_scope,
                        );
                    }
                }
            }
            _ => {}
        }
        let ast = &compiler.ast;
        match operator_token {
            Token::AND => {
                let first = condition.get_first_child(ast).unwrap();
                let last = condition.get_last_child(ast).unwrap();
                if outcome.is_truthy() {
                    return self.case_and_or_not_short_circuiting(
                        compiler,
                        first,
                        last,
                        blind_scope,
                        Outcome::TRUE,
                    );
                } else {
                    return self.case_and_or_maybe_short_circuiting(
                        compiler,
                        first,
                        last,
                        blind_scope,
                        Outcome::TRUE,
                    );
                }
            }
            Token::OR => {
                let first = condition.get_first_child(ast).unwrap();
                let last = condition.get_last_child(ast).unwrap();
                if !outcome.is_truthy() {
                    return self.case_and_or_not_short_circuiting(
                        compiler,
                        first,
                        last,
                        blind_scope,
                        Outcome::FALSE,
                    );
                } else {
                    return self.case_and_or_maybe_short_circuiting(
                        compiler,
                        first,
                        last,
                        blind_scope,
                        Outcome::FALSE,
                    );
                }
            }
            Token::EQ => {
                if outcome.is_truthy() {
                    return self.case_equality(compiler, condition, blind_scope, Self::eq);
                } else {
                    return self.case_equality(compiler, condition, blind_scope, Self::ne);
                }
            }
            Token::NE => {
                if outcome.is_truthy() {
                    return self.case_equality(compiler, condition, blind_scope, Self::ne);
                } else {
                    return self.case_equality(compiler, condition, blind_scope, Self::eq);
                }
            }
            Token::SHEQ => {
                if outcome.is_truthy() {
                    return self.case_equality(compiler, condition, blind_scope, Self::sheq);
                } else {
                    return self.case_equality(compiler, condition, blind_scope, Self::shne);
                }
            }
            Token::SHNE => {
                if outcome.is_truthy() {
                    return self.case_equality(compiler, condition, blind_scope, Self::shne);
                } else {
                    return self.case_equality(compiler, condition, blind_scope, Self::sheq);
                }
            }
            Token::NAME | Token::GETPROP => {
                return self.case_name_or_get_prop(compiler, condition, blind_scope, outcome);
            }
            Token::ASSIGN => {
                let first = condition.get_first_child(ast).unwrap();
                let second = condition.get_second_child(ast).unwrap();
                let second_scope = self.first_preciser_scope_knowing_condition_outcome(
                    compiler,
                    second,
                    blind_scope,
                    outcome,
                );
                return self.first_preciser_scope_knowing_condition_outcome(
                    compiler,
                    first,
                    second_scope,
                    outcome,
                );
            }
            Token::NOT => {
                let first = condition.get_first_child(ast).unwrap();
                return self.first_preciser_scope_knowing_condition_outcome(
                    compiler,
                    first,
                    blind_scope,
                    outcome.not(),
                );
            }
            Token::LE | Token::LT | Token::GE | Token::GT if outcome.is_truthy() => {
                return self.case_equality(compiler, condition, blind_scope, Self::ineq);
            }
            Token::INSTANCEOF => {
                let first = condition.get_first_child(ast).unwrap();
                let last = condition.get_last_child(ast).unwrap();
                return self.case_instance_of(compiler, first, last, blind_scope, outcome);
            }
            Token::IN => {
                let first = condition.get_first_child(ast).unwrap();
                if outcome.is_truthy() && first.is_string_lit(ast) {
                    let last = condition.get_last_child(ast).unwrap();
                    let property_name = first.get_string(ast);
                    return self.case_in(compiler, last, &property_name, blind_scope);
                }
            }
            Token::CASE => {
                let left = condition
                    .get_parent(ast)
                    .unwrap()
                    .get_previous(ast)
                    .unwrap(); // the switch condition
                let right = condition.get_first_child(ast).unwrap();
                if outcome.is_truthy() {
                    return self.case_equality_of(compiler, left, right, blind_scope, Self::sheq);
                } else {
                    return self.case_equality_of(compiler, left, right, blind_scope, Self::shne);
                }
            }
            Token::CALL => {
                let left = condition.get_first_child(ast).unwrap();
                let left_name = left.get_qualified_name(ast);
                if left_name.is_some_and(|left_name| left_name == "Array.isArray")
                    && let Some(next) = left.get_next(ast)
                {
                    return self.case_is_array(compiler, next, blind_scope, outcome);
                }
            }
            _ => {}
        }

        self.next_preciser_scope_knowing_condition_outcome(
            compiler,
            condition,
            blind_scope,
            outcome,
        )
    }
}

impl ChainableReverseAbstractInterpreter for SemanticReverseAbstractInterpreter {
    fn links(&self) -> &ChainLinks {
        &self.links
    }
}

/// See `SemanticReverseAbstractInterpreter#caseInstanceOf`.
struct RestrictByTrueInstanceOfResultVisitor {
    target: TypeId,
}

impl RestrictByTrueInstanceOfResultVisitor {
    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#RestrictByTrueInstanceOfResultVisitor
    fn new(target: TypeId) -> Self {
        Self { target }
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#applyCommonRestriction
    fn apply_common_restriction(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        if self.target.is_unknown_type(reg, ast) {
            return Some(type_);
        }

        let func_target = self.target.to_maybe_function_type(reg).unwrap();
        if func_target.has_instance_type(reg) {
            let instance_type = func_target.get_instance_type(reg).unwrap();
            return Some(type_.get_greatest_subtype(reg, ast, instance_type));
        }

        None
    }
}

impl RestrictByTypeOfResultVisitor for RestrictByTrueInstanceOfResultVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::True
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#caseTopType
    fn case_top_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.apply_common_restriction(reg, ast, type_)
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#caseUnknownType
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        let func_target = to_maybe_function_type(reg, Some(self.target));
        if let Some(func_target) = func_target
            && func_target.has_instance_type(reg)
        {
            return func_target.get_instance_type(reg);
        }
        Some(reg.get_native_type(JSTypeNative::UNKNOWN_TYPE))
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#caseObjectType
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.apply_common_restriction(reg, ast, type_)
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#caseUnionType
    fn case_union_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.apply_common_restriction(reg, ast, type_)
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByTrueInstanceOfResultVisitor#caseFunctionType
    fn case_function_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.case_object_type(reg, ast, type_)
    }
}

/// See `SemanticReverseAbstractInterpreter#caseInstanceOf`.
struct RestrictByFalseInstanceOfResultVisitor {
    target: TypeId,
}

impl RestrictByFalseInstanceOfResultVisitor {
    // port: SemanticReverseAbstractInterpreter.RestrictByFalseInstanceOfResultVisitor#RestrictByFalseInstanceOfResultVisitor
    fn new(target: TypeId) -> Self {
        Self { target }
    }
}

impl RestrictByTypeOfResultVisitor for RestrictByFalseInstanceOfResultVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::False
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByFalseInstanceOfResultVisitor#caseObjectType
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        if self.target.is_unknown_type(reg, ast) {
            return Some(type_);
        }

        let func_target = self.target.to_maybe_function_type(reg).unwrap();
        if func_target.has_instance_type(reg) {
            let instance_type = func_target.get_instance_type(reg).unwrap();
            if type_.is_subtype_of(reg, ast, instance_type) {
                return None;
            }

            return Some(type_);
        }

        None
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByFalseInstanceOfResultVisitor#caseUnionType
    fn case_union_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        if self.target.is_unknown_type(reg, ast) {
            return Some(type_);
        }

        let func_target = self.target.to_maybe_function_type(reg).unwrap();
        if func_target.has_instance_type(reg) {
            let instance_type = func_target.get_instance_type(reg).unwrap();
            return Some(type_.get_restricted_union(reg, ast, instance_type));
        }

        None
    }

    // port: SemanticReverseAbstractInterpreter.RestrictByFalseInstanceOfResultVisitor#caseFunctionType
    fn case_function_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.case_object_type(reg, ast, type_)
    }
}

/// Java's `Set<String> refinements`, shared by the RefinementTrackingFlowScopes of one refinement.
type Refinements = Arc<Mutex<IndexSet<JsString>>>;

fn lock(refinements: &Refinements) -> MutexGuard<'_, IndexSet<JsString>> {
    refinements.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Java's `refinements.size() == 1 ? refinements.iterator().next() : <no slot>`.
fn single_refinement(refinements: &Refinements) -> Option<JsString> {
    let refinements = lock(refinements);
    if refinements.len() == 1 {
        refinements.get_index(0).cloned()
    } else {
        None
    }
}

// port: SemanticReverseAbstractInterpreter#unwrap
/// Unwraps any RefinementTrackingFlowScopes.
fn unwrap(mut scope: Arc<dyn FlowScope>) -> Arc<dyn FlowScope> {
    loop {
        match scope
            .clone()
            .as_any_arc()
            .downcast::<RefinementTrackingFlowScope>()
        {
            Ok(tracking) => scope = tracking.delegate.clone(),
            Err(_) => return scope,
        }
    }
}

/// A wrapper around FlowScope that keeps track of which vars were refined.
struct RefinementTrackingFlowScope {
    delegate: Arc<dyn FlowScope>,
    refinements: Refinements,
}

impl RefinementTrackingFlowScope {
    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#RefinementTrackingFlowScope
    fn new(delegate: Arc<dyn FlowScope>, refinements: Refinements) -> Self {
        Self {
            delegate,
            refinements,
        }
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#wrap
    fn wrap(self: Arc<Self>, scope: Arc<dyn FlowScope>) -> Arc<dyn FlowScope> {
        if !Arc::ptr_eq(&scope, &self.delegate) {
            Arc::new(RefinementTrackingFlowScope::new(
                scope,
                self.refinements.clone(),
            ))
        } else {
            self
        }
    }
}

impl FlowScope for RefinementTrackingFlowScope {
    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#withSyntacticScope
    fn with_syntactic_scope(
        self: Arc<Self>,
        _compiler: &mut AbstractCompiler,
        _scope: TypedScope,
    ) -> Arc<dyn FlowScope> {
        panic!("UnsupportedOperationException")
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#inferSlotType
    fn infer_slot_type(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        symbol: &JsString,
        type_: Option<TypeId>,
    ) -> Arc<dyn FlowScope> {
        lock(&self.refinements).insert(symbol.clone());
        let scope = self
            .delegate
            .clone()
            .infer_slot_type(compiler, symbol, type_);
        self.wrap(scope)
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#inferQualifiedSlot
    fn infer_qualified_slot(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        symbol: &JsString,
        bottom_type: Option<TypeId>,
        inferred_type: TypeId,
        declare: bool,
    ) -> Arc<dyn FlowScope> {
        lock(&self.refinements).insert(symbol.clone());
        let scope = self.delegate.clone().infer_qualified_slot(
            compiler,
            node,
            symbol,
            bottom_type,
            inferred_type,
            declare,
        );
        self.wrap(scope)
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#getDeclarationScope
    fn get_declaration_scope(&self, compiler: &AbstractCompiler) -> TypedScope {
        self.delegate.get_declaration_scope(compiler)
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#getRootNode
    fn get_root_node(&self, compiler: &AbstractCompiler) -> NodeId {
        self.delegate.get_root_node(compiler)
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#getParentScope
    fn get_parent_scope(&self, _compiler: &AbstractCompiler) -> Option<TypedScope> {
        panic!("UnsupportedOperationException")
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#getSlot
    fn get_slot(&self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<FlowSlot> {
        self.delegate.get_slot(compiler, name)
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#getOwnSlot
    fn get_own_slot(&self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<FlowSlot> {
        self.delegate.get_own_slot(compiler, name)
    }

    // port: SemanticReverseAbstractInterpreter.RefinementTrackingFlowScope#getTypeOfThis
    fn get_type_of_this(&self, compiler: &mut AbstractCompiler) -> Option<TypeId> {
        self.delegate.get_type_of_this(compiler)
    }

    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

impl Annotation for RefinementTrackingFlowScope {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    /// Java's Object#toString; the identity hash is JVM-dependent, so the port prints 0.
    fn to_string(&self) -> String {
        format!(
            "com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter$RefinementTrackingFlowScope@{:x}",
            0
        )
    }
}
impl LatticeElement for RefinementTrackingFlowScope {}
