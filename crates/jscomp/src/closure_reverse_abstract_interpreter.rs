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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/type/ClosureReverseAbstractInterpreter.java.

//! A reverse abstract interpreter (RAI) for specific closure patterns such as `goog.isObject`.
//!
//! Java's `com.google.javascript.jscomp.type.ClosureReverseAbstractInterpreter`.
use crate::{
    abstract_compiler::AbstractCompiler,
    chainable_reverse_abstract_interpreter::{
        ChainLinks, ChainableReverseAbstractInterpreter, RestrictByTypeOfResultVisitor,
        RestrictByTypeOfResultVisitorKind, visit,
    },
    flow_scope::FlowScope,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
};
use closure_jstype::{JSTypeNative, JSTypeRegistry, TypeId};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
    outcome::Outcome,
};
use std::sync::{Arc, Weak};

/// Java's `Function<TypeRestriction, JSType>`: the restricter lambdas of `restricters`.
type Restricter = fn(
    &ClosureReverseAbstractInterpreter,
    &mut JSTypeRegistry,
    &Ast,
    TypeRestriction,
) -> Option<TypeId>;

pub struct ClosureReverseAbstractInterpreter {
    links: ChainLinks,
    /// Functions used to restrict types.
    restricters: IndexMap<JsString, Restricter>,
}

impl ClosureReverseAbstractInterpreter {
    // port: ClosureReverseAbstractInterpreter#ClosureReverseAbstractInterpreter
    pub fn new(_type_registry: &JSTypeRegistry) -> Arc<Self> {
        let mut restricters: IndexMap<JsString, Restricter> = IndexMap::<_, _>::default();
        restricters.insert(JsString::from("isObject"), Self::is_object);
        Arc::new_cyclic(|this: &Weak<Self>| Self {
            links: ChainLinks::new(this.clone()),
            restricters,
        })
    }

    // port: ClosureReverseAbstractInterpreter#ClosureReverseAbstractInterpreter
    /// The "isObject" lambda of the constructor's `restricters` map.
    fn is_object(&self, reg: &mut JSTypeRegistry, ast: &Ast, p: TypeRestriction) -> Option<TypeId> {
        let Some(type_) = p.type_ else {
            return if p.outcome {
                Some(self.get_native_type(reg, JSTypeNative::OBJECT_TYPE))
            } else {
                None
            };
        };

        if p.outcome {
            visit(type_, reg, ast, &mut RestrictToObjectVisitor)
        } else {
            visit(type_, reg, ast, &mut RestrictToNotObjectVisitor)
        }
    }

    // port: ClosureReverseAbstractInterpreter#restrictParameter
    #[must_use]
    fn restrict_parameter(
        &self,
        compiler: &mut AbstractCompiler,
        parameter: NodeId,
        type_: Option<TypeId>,
        blind_scope: Arc<dyn FlowScope>,
        restriction: Restricter,
        outcome: bool,
    ) -> Arc<dyn FlowScope> {
        // restricting
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let type_ = restriction(self, reg, ast, TypeRestriction::new(type_, outcome));

        // changing the scope
        if let Some(type_) = type_ {
            self.declare_name_in_scope(compiler, blind_scope, parameter, type_)
        } else {
            blind_scope
        }
    }
}

impl ReverseAbstractInterpreter for ClosureReverseAbstractInterpreter {
    // port: ClosureReverseAbstractInterpreter#getPreciserScopeKnowingConditionOutcome
    fn get_preciser_scope_knowing_condition_outcome(
        &self,
        compiler: &mut AbstractCompiler,
        condition: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        let ast = &compiler.ast;
        if condition.is_call(ast) && condition.has_two_children(ast) {
            let callee = condition.get_first_child(ast).unwrap();
            let param = condition.get_last_child(ast).unwrap();
            if callee.is_get_prop(ast) && param.is_qualified_name(ast) {
                let param_type = self.get_type_if_refinable(compiler, param, &blind_scope);
                let ast = &compiler.ast;
                let receiver = callee.get_first_child(ast).unwrap();
                if receiver.is_name(ast) && receiver.get_string_ref(ast) == "goog" {
                    let restricter = self.restricters.get(&callee.get_string(ast)).copied();
                    if let Some(restricter) = restricter {
                        return self.restrict_parameter(
                            compiler,
                            param,
                            param_type,
                            blind_scope,
                            restricter,
                            outcome.is_truthy(),
                        );
                    }
                }
            }
        }
        self.next_preciser_scope_knowing_condition_outcome(
            compiler,
            condition,
            blind_scope,
            outcome,
        )
    }
}

impl ChainableReverseAbstractInterpreter for ClosureReverseAbstractInterpreter {
    fn links(&self) -> &ChainLinks {
        &self.links
    }
}

/// For when `goog.isObject` returns true. This includes functions, but not `null` (Java's
/// `restrictToObjectVisitor`).
struct RestrictToObjectVisitor;

impl RestrictByTypeOfResultVisitor for RestrictToObjectVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::True
    }

    // port: ClosureReverseAbstractInterpreter#restrictToObjectVisitor
    fn case_top_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _top_type: TypeId,
    ) -> Option<TypeId> {
        Some(reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE))
    }

    // port: ClosureReverseAbstractInterpreter#restrictToObjectVisitor
    fn case_object_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        Some(type_)
    }

    // port: ClosureReverseAbstractInterpreter#restrictToObjectVisitor
    fn case_function_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        Some(type_)
    }
}

/// For when `goog.isObject` returns false (Java's `restrictToNotObjectVisitor`).
struct RestrictToNotObjectVisitor;

impl RestrictByTypeOfResultVisitor for RestrictToNotObjectVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::False
    }

    // port: ClosureReverseAbstractInterpreter#restrictToNotObjectVisitor
    fn case_object_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _type: TypeId,
    ) -> Option<TypeId> {
        None
    }

    // port: ClosureReverseAbstractInterpreter#restrictToNotObjectVisitor
    fn case_function_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _type: TypeId,
    ) -> Option<TypeId> {
        None
    }
}

struct TypeRestriction {
    type_: Option<TypeId>,
    outcome: bool,
}

impl TypeRestriction {
    // port: ClosureReverseAbstractInterpreter.TypeRestriction#TypeRestriction
    fn new(type_: Option<TypeId>, outcome: bool) -> Self {
        Self { type_, outcome }
    }
}
