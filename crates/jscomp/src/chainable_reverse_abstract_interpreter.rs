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
//   src/com/google/javascript/jscomp/type/ChainableReverseAbstractInterpreter.java.

//! Chainable reverse abstract interpreter providing basic functionality.
//!
//! Java's abstract class `ChainableReverseAbstractInterpreter` is the trait of the same name: its
//! fields `firstLink` and `nextLink` are [`ChainLinks`], which every link embeds, and its concrete
//! methods are provided methods. Java's `typeRegistry` field is not kept (DESIGN §8): the methods
//! take the compiler, or the registry and its Ast.
//!
//! Links are shared `Arc` objects, like Java's chain objects. A link holds its next link strongly
//! and its first link weakly: Java's first link is its own `firstLink`, and a strong reference
//! would be an `Arc` cycle. Whoever uses a chain holds its first link (`Compiler` keeps the result
//! of `getFirst()`, a lone link is its own first link).
//!
//! Java's abstract visitor classes `RestrictByTypeOfResultVisitor`,
//! `RestrictByTrueTypeOfResultVisitor` and `RestrictByFalseTypeOfResultVisitor` are the trait
//! [`RestrictByTypeOfResultVisitor`]: a concrete visitor names the class it extends with
//! [`RestrictByTypeOfResultVisitor::kind`], inherits that class's bodies as default methods and
//! overrides the same methods as in Java. [`visit`] runs one over a type (Java's
//! `type.visit(visitor)`).
use crate::{
    abstract_compiler::AbstractCompiler, flow_scope::FlowScope,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
};
use closure_jstype::{
    JSTypeNative, JSTypeRegistry, TypeId,
    prelude::{EnumElementType, JSType, ProxyObjectType, UnionType},
    visitor::Visitor,
};
use closure_rhino::{
    check_argument, check_not_null,
    js_string::JsString,
    node::{Ast, NodeId},
    outcome::Outcome,
    token::Token,
};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

/// Java's `firstLink` and `nextLink` fields of a ChainableReverseAbstractInterpreter.
pub struct ChainLinks {
    first_link: Mutex<Weak<dyn ChainableReverseAbstractInterpreter>>,
    next_link: Mutex<Option<Arc<dyn ChainableReverseAbstractInterpreter>>>,
}

impl ChainLinks {
    // port: ChainableReverseAbstractInterpreter#ChainableReverseAbstractInterpreter
    /// Constructs the fields of an interpreter which is the only link in a chain. Interpreters can
    /// be appended using `append`. `this` is the link under construction (`Arc::new_cyclic`).
    pub fn new(this: Weak<dyn ChainableReverseAbstractInterpreter>) -> Self {
        Self {
            first_link: Mutex::new(this),
            next_link: Mutex::new(None),
        }
    }

    fn first(&self) -> MutexGuard<'_, Weak<dyn ChainableReverseAbstractInterpreter>> {
        self.first_link
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn next(&self) -> MutexGuard<'_, Option<Arc<dyn ChainableReverseAbstractInterpreter>>> {
        self.next_link
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Java's `firstLink` field.
    fn first_link(&self) -> Arc<dyn ChainableReverseAbstractInterpreter> {
        self.first()
            .upgrade()
            .expect("the first link of a reverse abstract interpreter chain was dropped")
    }

    /// Java's `nextLink` field.
    fn next_link(&self) -> Option<Arc<dyn ChainableReverseAbstractInterpreter>> {
        self.next().clone()
    }
}

pub trait ChainableReverseAbstractInterpreter: ReverseAbstractInterpreter {
    /// Rust-only: the `firstLink`/`nextLink` fields of this link.
    fn links(&self) -> &ChainLinks;

    // port: ChainableReverseAbstractInterpreter#append
    /// Appends a link to `this`, returning the updated last link.
    ///
    /// The pattern `new X().append(new Y())...append(new Z())` forms a chain starting with X, then
    /// Y, then ... Z. `last_link` is a chainable interpreter, with no next link.
    fn append(
        &self,
        last_link: Arc<dyn ChainableReverseAbstractInterpreter>,
    ) -> Arc<dyn ChainableReverseAbstractInterpreter> {
        check_argument!(last_link.links().next_link().is_none());
        *self.links().next() = Some(last_link.clone());
        let first_link = self.links().first().clone();
        *last_link.links().first() = first_link;
        last_link
    }

    // port: ChainableReverseAbstractInterpreter#getFirst
    /// Gets the first link of this chain.
    fn get_first(&self) -> Arc<dyn ChainableReverseAbstractInterpreter> {
        self.links().first_link()
    }

    // port: ChainableReverseAbstractInterpreter#firstPreciserScopeKnowingConditionOutcome
    /// Calculates the preciser scope starting with the first link.
    fn first_preciser_scope_knowing_condition_outcome(
        &self,
        compiler: &mut AbstractCompiler,
        condition: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        self.links()
            .first_link()
            .get_preciser_scope_knowing_condition_outcome(compiler, condition, blind_scope, outcome)
    }

    // port: ChainableReverseAbstractInterpreter#nextPreciserScopeKnowingConditionOutcome
    /// Delegates the calculation of the preciser scope to the next link. If there is no next link,
    /// returns the blind scope.
    fn next_preciser_scope_knowing_condition_outcome(
        &self,
        compiler: &mut AbstractCompiler,
        condition: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        match self.links().next_link() {
            Some(next_link) => next_link.get_preciser_scope_knowing_condition_outcome(
                compiler,
                condition,
                blind_scope,
                outcome,
            ),
            None => blind_scope,
        }
    }

    // port: ChainableReverseAbstractInterpreter#getTypeIfRefinable
    /// Returns the type of a node in the given scope if the node corresponds to a name whose type
    /// is capable of being refined.
    ///
    /// Returns the current type of the node if it can be refined, null otherwise.
    fn get_type_if_refinable(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        scope: &Arc<dyn FlowScope>,
    ) -> Option<TypeId> {
        match node.get_token(&compiler.ast) {
            Token::NAME => {
                let name = node.get_string(&compiler.ast);
                let name_var = scope.get_slot(compiler, &name);
                if let Some(name_var) = name_var {
                    let mut name_var_type = name_var.get_type(compiler);
                    if name_var_type.is_none() {
                        name_var_type = node.get_jstype(&compiler.ast);
                    }
                    return name_var_type;
                }
                return None;
            }
            Token::GETPROP => {
                let qualified_name = node.get_qualified_name(&compiler.ast)?;
                let prop_var = scope.get_slot(compiler, &qualified_name);
                let mut prop_var_type = None;
                if let Some(prop_var) = prop_var {
                    prop_var_type = prop_var.get_type(compiler);
                }
                if prop_var_type.is_none() {
                    prop_var_type = node.get_jstype(&compiler.ast);
                }
                if prop_var_type.is_none() {
                    prop_var_type =
                        Some(self.get_native_type(
                            compiler.get_type_registry(),
                            JSTypeNative::UNKNOWN_TYPE,
                        ));
                }
                return prop_var_type;
            }
            _ => {}
        }
        None
    }

    // port: ChainableReverseAbstractInterpreter#declareNameInScope
    /// Declares a refined type in `scope` for the name represented by `node`. It must be possible
    /// to refine the type of the given node in the given scope, as determined by
    /// `get_type_if_refinable`. Returns an updated flow scope, which may be different from the
    /// passed-in scope if any changes occur.
    #[must_use]
    fn declare_name_in_scope(
        &self,
        compiler: &mut AbstractCompiler,
        scope: Arc<dyn FlowScope>,
        node: NodeId,
        type_: TypeId,
    ) -> Arc<dyn FlowScope> {
        match node.get_token(&compiler.ast) {
            Token::NAME => {
                let name = node.get_string(&compiler.ast);
                scope.infer_slot_type(compiler, &name, Some(type_))
            }
            Token::GETPROP => {
                let qualified_name = check_not_null!(node.get_qualified_name(&compiler.ast));

                let orig_type = node.get_jstype(&compiler.ast);
                let orig_type = match orig_type {
                    None => self
                        .get_native_type(compiler.get_type_registry(), JSTypeNative::UNKNOWN_TYPE),
                    Some(orig_type) => orig_type,
                };
                scope.infer_qualified_slot(
                    compiler,
                    node,
                    &qualified_name,
                    Some(orig_type),
                    type_,
                    false,
                )
            }
            // "this" references aren't currently modeled in the CFG.
            Token::THIS => scope,
            _ => panic!(
                "IllegalArgumentException: Node cannot be refined. \n{}",
                node.to_string_tree(&compiler.ast)
            ),
        }
    }

    // port: ChainableReverseAbstractInterpreter#getRestrictedByTypeOfResult
    /// Returns a version of `type` that is restricted by some knowledge about the result of the
    /// `typeof` operation.
    ///
    /// The behavior of the `typeof` operator can be summarized by the following table:
    ///
    /// | type | result |
    /// |---|---|
    /// | `undefined` | "undefined" |
    /// | `null` | "object" |
    /// | `boolean` | "boolean" |
    /// | `number` | "number" |
    /// | `string` | "string" |
    /// | `Object` (which doesn't implement [[Call]]) | "object" |
    /// | `Object` (which implements [[Call]]) | "function" |
    ///
    /// `value` is a value known to be equal or not equal to the result of the `typeof` operation;
    /// `result_equals_value` is `true` if the `typeOf` result is known to equal `value`, `false` if
    /// it is known *not* to equal `value`. Returns the restricted type or null if no version of the
    /// type matches the restriction.
    fn get_restricted_by_type_of_result(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: Option<TypeId>,
        value: &JsString,
        result_equals_value: bool,
    ) -> Option<TypeId> {
        let Some(type_) = type_ else {
            if result_equals_value {
                let result = get_native_type_for_type_of(reg, ast, value);
                return Some(match result {
                    None => self.get_native_type(reg, JSTypeNative::CHECKED_UNKNOWN_TYPE),
                    Some(result) => result,
                });
            } else {
                return None;
            }
        };
        visit(
            type_,
            reg,
            ast,
            &mut RestrictByOneTypeOfResultVisitor::new(value.clone(), result_equals_value),
        )
    }

    // port: ChainableReverseAbstractInterpreter#getNativeType
    fn get_native_type(&self, reg: &JSTypeRegistry, type_id: JSTypeNative) -> TypeId {
        reg.get_native_type(type_id)
    }
}

// port: ChainableReverseAbstractInterpreter#getNativeTypeForTypeOf
/// If we definitely know what a type is based on the typeof result, return it. Otherwise, return
/// null.
///
/// The typeof operation in JS is poorly defined, and this function works for both the native typeof
/// and goog.typeOf. It should not be made public, because its semantics are informally defined, and
/// would be wrong in the general case.
fn get_native_type_for_type_of(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    value: &JsString,
) -> Option<TypeId> {
    if *value == "number" {
        Some(reg.get_native_type(JSTypeNative::NUMBER_TYPE))
    } else if *value == "boolean" {
        Some(reg.get_native_type(JSTypeNative::BOOLEAN_TYPE))
    } else if *value == "string" {
        Some(reg.get_native_type(JSTypeNative::STRING_TYPE))
    } else if *value == "symbol" {
        Some(reg.get_native_type(JSTypeNative::SYMBOL_TYPE))
    } else if *value == "undefined" {
        Some(reg.get_native_type(JSTypeNative::VOID_TYPE))
    } else if *value == "object" {
        // NOTE: This is broader than it needs to be if it's from goog.typeof, but (a) it's more
        // consistent with common usage of the native builtin typeof, (b) it's more consistent
        // with
        // TypeScript, and (c) it's more useful than simply not narrowing.
        let object_type = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
        let null_type = reg.get_native_type(JSTypeNative::NULL_TYPE);
        Some(reg.create_union_type(ast, &[object_type, null_type]))
    } else if *value == "function" {
        Some(reg.get_native_type(JSTypeNative::FUNCTION_TYPE))
    } else {
        None
    }
}

/// The Java abstract visitor class a concrete [`RestrictByTypeOfResultVisitor`] extends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestrictByTypeOfResultVisitorKind {
    /// `RestrictByTypeOfResultVisitor` itself: the visitor overrides every abstract case.
    Base,
    /// `RestrictByTrueTypeOfResultVisitor`: all base cases return null.
    True,
    /// `RestrictByFalseTypeOfResultVisitor`: all base cases return their type.
    False,
}

/// A class common to all visitors that need to restrict the type based on `typeof`-like
/// conditions (Java's `RestrictByTypeOfResultVisitor implements Visitor<JSType>`, with its two
/// abstract subclasses; see the module documentation).
pub trait RestrictByTypeOfResultVisitor {
    /// Rust-only: which of Java's abstract visitor classes this visitor extends.
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind;

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseTopType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseTopType
    /// Abstracts away the similarities between visiting the unknown type and the all type.
    /// `top_type` is `UNKNOWN_TYPE` or `ALL_TYPE`; returns the restricted type.
    fn case_top_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        top_type: TypeId,
    ) -> Option<TypeId> {
        match self.kind() {
            RestrictByTypeOfResultVisitorKind::False => Some(top_type),
            kind => unreachable!("abstract caseTopType of a {kind:?} visitor"),
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseAllType
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        let top_type = reg.get_native_type(JSTypeNative::ALL_TYPE);
        self.case_top_type(reg, ast, top_type)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseUnknownType
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        let top_type = reg.get_native_type(JSTypeNative::CHECKED_UNKNOWN_TYPE);
        self.case_top_type(reg, ast, top_type)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseUnionType
    fn case_union_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        let mut restricted: Option<TypeId> = None;
        let alternates = type_.get_alternates(reg, ast);
        for alternate in alternates.iter().copied() {
            let restricted_alternate = visit(alternate, reg, ast, self);
            if let Some(restricted_alternate) = restricted_alternate {
                restricted = Some(match restricted {
                    None => restricted_alternate,
                    Some(restricted) => {
                        restricted_alternate.get_least_supertype(reg, ast, restricted)
                    }
                });
            }
        }
        restricted
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseNoType
    fn case_no_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        Some(type_)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseEnumElementType
    fn case_enum_element_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        enum_element_type: TypeId,
    ) -> Option<TypeId> {
        // NOTE(nicksantos): This is a white lie. Suppose we have:
        // /** @enum {string|number} */ var MyEnum = ...;
        // if (goog.isNumber(myEnumInstance)) {
        //   /* what is myEnumInstance here? */
        // }
        // There is no type that represents {MyEnum - string}. What we really
        // need is a notion of "enum subtyping", so that we could dynamically
        // create a subtype of MyEnum restricted by string. In any case,
        // this should catch the common case.
        let primitive_type = enum_element_type.get_primitive_type(reg);
        let type_ = visit(primitive_type, reg, ast, self);
        if type_.is_some()
            && enum_element_type
                .get_primitive_type(reg)
                .equals(reg, ast, type_)
        {
            Some(enum_element_type)
        } else {
            type_
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseTemplatizedType
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.case_object_type(reg, ast, type_)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseTemplateType
    fn case_template_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        template_type: TypeId,
    ) -> Option<TypeId> {
        self.case_object_type(reg, ast, template_type)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseNamedType
    fn case_named_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.case_proxy_object_type(reg, ast, type_)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTypeOfResultVisitor#caseProxyObjectType
    fn case_proxy_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        type_.visit_reference_type(reg, ast, &mut RestrictByTypeOfResultAdapter(self))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseNoObjectType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseNoObjectType
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE)))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseBooleanType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseBooleanType
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::BOOLEAN_TYPE)))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseFunctionType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseFunctionType
    fn case_function_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.base_case(Some(type_))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseNullType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseNullType
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::NULL_TYPE)))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseNumberType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseNumberType
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::NUMBER_TYPE)))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseBigIntType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseBigIntType
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::BIGINT_TYPE)))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseObjectType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseObjectType
    fn case_object_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.base_case(Some(type_))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseStringType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseStringType
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::STRING_TYPE)))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseSymbolType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseSymbolType
    fn case_symbol_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.base_case(Some(type_))
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByTrueTypeOfResultVisitor#caseVoidType
    // port: ChainableReverseAbstractInterpreter.RestrictByFalseTypeOfResultVisitor#caseVoidType
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        self.base_case(Some(reg.get_native_type(JSTypeNative::VOID_TYPE)))
    }

    /// Rust-only: a base case of the abstract class this visitor extends. All base cases of
    /// `RestrictByTrueTypeOfResultVisitor` return null; those of
    /// `RestrictByFalseTypeOfResultVisitor` return their type (`false_case`). A
    /// `RestrictByTypeOfResultVisitor` that extends neither overrides every base case.
    fn base_case(&self, false_case: Option<TypeId>) -> Option<TypeId> {
        match self.kind() {
            RestrictByTypeOfResultVisitorKind::True => None,
            RestrictByTypeOfResultVisitorKind::False => false_case,
            RestrictByTypeOfResultVisitorKind::Base => {
                unreachable!("abstract base case of a RestrictByTypeOfResultVisitor")
            }
        }
    }
}

/// Java's `type.visit(visitor)` for a [`RestrictByTypeOfResultVisitor`].
pub fn visit<V: RestrictByTypeOfResultVisitor + ?Sized>(
    type_: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut V,
) -> Option<TypeId> {
    type_.visit(reg, ast, &mut RestrictByTypeOfResultAdapter(visitor))
}

/// Rust-only: the closure-jstype `Visitor<JSType>` a [`RestrictByTypeOfResultVisitor`] is in Java
/// (a foreign trait cannot be implemented for every implementor of a local one).
struct RestrictByTypeOfResultAdapter<'v, V: ?Sized>(&'v mut V);

impl<V: RestrictByTypeOfResultVisitor + ?Sized> Visitor<Option<TypeId>>
    for RestrictByTypeOfResultAdapter<'_, V>
{
    fn case_no_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_no_type(reg, ast, type_)
    }
    fn case_enum_element_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_enum_element_type(reg, ast, type_)
    }
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_all_type(reg, ast)
    }
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_boolean_type(reg, ast)
    }
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_no_object_type(reg, ast)
    }
    fn case_function_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_function_type(reg, ast, type_)
    }
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_object_type(reg, ast, type_)
    }
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_unknown_type(reg, ast)
    }
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_null_type(reg, ast)
    }
    fn case_named_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_named_type(reg, ast, type_)
    }
    fn case_proxy_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_proxy_object_type(reg, ast, type_)
    }
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_number_type(reg, ast)
    }
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_big_int_type(reg, ast)
    }
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_string_type(reg, ast)
    }
    fn case_symbol_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_symbol_type(reg, ast, type_)
    }
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.0.case_void_type(reg, ast)
    }
    fn case_union_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_union_type(reg, ast, type_)
    }
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_templatized_type(reg, ast, type_)
    }
    fn case_template_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        self.0.case_template_type(reg, ast, type_)
    }
}

/// See [`ChainableReverseAbstractInterpreter::get_restricted_by_type_of_result`].
struct RestrictByOneTypeOfResultVisitor {
    /// A value known to be equal or not equal to the result of the `typeOf` operation.
    value: JsString,
    /// `true` if the `typeOf` result is known to equal `value`; `false` if it is known *not* to
    /// equal `value`.
    result_equals_value: bool,
}

impl RestrictByOneTypeOfResultVisitor {
    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#RestrictByOneTypeOfResultVisitor
    fn new(value: JsString, result_equals_value: bool) -> Self {
        Self {
            value,
            result_equals_value,
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#matchesExpectation
    /// Computes whether the given result of a `typeof` operator matches expectations, i.e. whether
    /// a type that gives such a result should be kept.
    fn matches_expectation(&self, result: &str) -> bool {
        (self.value == result) == self.result_equals_value
    }
}

impl RestrictByTypeOfResultVisitor for RestrictByOneTypeOfResultVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::Base
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseTopType
    fn case_top_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        top_type: TypeId,
    ) -> Option<TypeId> {
        let mut result = top_type;
        if self.result_equals_value {
            let type_by_name = get_native_type_for_type_of(reg, ast, &self.value);
            if let Some(type_by_name) = type_by_name {
                result = type_by_name;
            }
        }
        Some(result)
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseNoObjectType
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if (self.value == "object" || self.value == "function") == self.result_equals_value {
            Some(reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE))
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseBooleanType
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if self.matches_expectation("boolean") {
            Some(reg.get_native_type(JSTypeNative::BOOLEAN_TYPE))
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseFunctionType
    fn case_function_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        if self.matches_expectation("function") {
            Some(type_)
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseNullType
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if self.matches_expectation("object") {
            Some(reg.get_native_type(JSTypeNative::NULL_TYPE))
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseNumberType
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if self.matches_expectation("number") {
            Some(reg.get_native_type(JSTypeNative::NUMBER_TYPE))
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseBigIntType
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if self.matches_expectation("bigint") {
            Some(reg.get_native_type(JSTypeNative::BIGINT_TYPE))
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseObjectType
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        if self.value == "function" {
            let ctor_type = reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
            if self.result_equals_value {
                // Objects are restricted to "Function", subtypes are left
                return Some(ctor_type.get_greatest_subtype(reg, ast, type_));
            } else {
                // Only filter out subtypes of "function"
                return if type_.is_subtype_of(reg, ast, ctor_type) {
                    None
                } else {
                    Some(type_)
                };
            }
        }
        if self.matches_expectation("object") {
            Some(type_)
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseStringType
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if self.matches_expectation("string") {
            Some(reg.get_native_type(JSTypeNative::STRING_TYPE))
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseSymbolType
    fn case_symbol_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        if self.matches_expectation("symbol") {
            Some(type_)
        } else {
            None
        }
    }

    // port: ChainableReverseAbstractInterpreter.RestrictByOneTypeOfResultVisitor#caseVoidType
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        if self.matches_expectation("undefined") {
            Some(reg.get_native_type(JSTypeNative::VOID_TYPE))
        } else {
            None
        }
    }
}

/// For when `goog.isArray` or `Array.isArray` returns true (Java's `restrictToArrayVisitor`).
pub struct RestrictToArrayVisitor;

impl RestrictByTypeOfResultVisitor for RestrictToArrayVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::True
    }

    // port: ChainableReverseAbstractInterpreter#restrictToArrayVisitor
    fn case_top_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        _ast: &Ast,
        top_type: TypeId,
    ) -> Option<TypeId> {
        Some(if top_type.is_all_type(reg) {
            reg.get_native_type(JSTypeNative::ARRAY_TYPE)
        } else {
            top_type
        })
    }

    // port: ChainableReverseAbstractInterpreter#restrictToArrayVisitor
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        let array_type = reg.get_native_type(JSTypeNative::ARRAY_TYPE);
        if array_type.is_subtype_of(reg, ast, type_) {
            Some(array_type)
        } else {
            None
        }
    }
}

/// For when `goog.isArray` or `Array.isArray` returns false (Java's `restrictToNotArrayVisitor`).
pub struct RestrictToNotArrayVisitor;

impl RestrictByTypeOfResultVisitor for RestrictToNotArrayVisitor {
    fn kind(&self) -> RestrictByTypeOfResultVisitorKind {
        RestrictByTypeOfResultVisitorKind::False
    }

    // port: ChainableReverseAbstractInterpreter#restrictToNotArrayVisitor
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Option<TypeId> {
        let array_type = reg.get_native_type(JSTypeNative::ARRAY_TYPE);
        if type_.is_subtype_of(reg, ast, array_type) {
            None
        } else {
            Some(type_)
        }
    }
}
