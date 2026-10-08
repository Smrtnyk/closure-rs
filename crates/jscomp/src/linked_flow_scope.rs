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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/LinkedFlowScope.java.

//! A flow scope that tries to store as little symbol information as possible, instead delegating to
//! its parents. Optimized for low memory use.
//!
//! Java's `inputProvider` field is the compiler (DESIGN §6: never stored; every method that used it
//! takes the compiler). Flow scopes are shared immutable objects (`Arc`), compared by identity.
use crate::{
    abstract_compiler::AbstractCompiler,
    flow_scope::{FlowScope, FlowSlot},
    graph::{annotation::Annotation, lattice_element::LatticeElement},
    node_util::NodeUtil,
    typed_scope::TypedScope,
};
use closure_jstype::{TypeId, prelude::JSType};
use closure_rhino::{
    check_not_null, hamt_pmap::HamtPMap, js_string::JsString, jsdoc_info::JSDocInfo, node::NodeId,
};
use std::{any::Any, sync::Arc};

#[derive(Debug)]
pub struct LinkedFlowScope {
    // Map from TypedScope to OverlayScope.
    scopes: HamtPMap<TypedScope, OverlayScopeRef>,
    function_scope: TypedScope,
    // The TypedScope for the block that this flow scope is defined for.
    syntactic_scope: TypedScope,
}

impl LinkedFlowScope {
    // port: LinkedFlowScope#LinkedFlowScope
    /// Creates a flow scope without a direct parent. This can happen in three cases: (1) the
    /// "bottom" scope for a CFG root, (2) a direct child of a parent at the maximum depth, or (3) a
    /// joined scope with more than one direct parent. The parent is non-null only in the second
    /// case.
    fn new(
        scopes: HamtPMap<TypedScope, OverlayScopeRef>,
        syntactic_scope: TypedScope,
        function_scope: TypedScope,
    ) -> Self {
        Self {
            scopes,
            syntactic_scope,
            function_scope,
        }
    }

    // port: LinkedFlowScope#trimScopes
    /// Returns the scope map, trimmed to the common ancestor between this FlowScope's
    /// syntacticScope and the given scope. Any inferred types on variables in deeper scopes cannot
    /// be propagated past this point (since they're no longer in scope), and trimming them eagerly
    /// allows us to ignore these irrelevant types when checking equality and joining.
    fn trim_scopes(
        &self,
        compiler: &AbstractCompiler,
        scope: TypedScope,
    ) -> HamtPMap<TypedScope, OverlayScopeRef> {
        let mut this_scope = Some(self.syntactic_scope);
        let mut that_scope = Some(scope);
        let mut this_depth = self.syntactic_scope.get_depth(compiler);
        let mut that_depth = scope.get_depth(compiler);
        let mut result = self.scopes.clone();
        while that_depth > this_depth {
            that_scope = check_not_null!(that_scope).get_parent(compiler);
            that_depth -= 1;
        }
        while this_depth > that_depth {
            result = result.minus(&check_not_null!(this_scope));
            this_scope = check_not_null!(this_scope).get_parent(compiler);
            this_depth -= 1;
        }
        while this_scope != that_scope && this_scope.is_some() && that_scope.is_some() {
            result = result.minus(&this_scope.unwrap());
            this_scope = this_scope.unwrap().get_parent(compiler);
            that_scope = that_scope.unwrap().get_parent(compiler);
        }
        result
    }

    // port: LinkedFlowScope#flowsFromBottom
    /// Whether this flows from a bottom scope.
    fn flows_from_bottom(&self, compiler: &AbstractCompiler) -> bool {
        self.function_scope.is_bottom(compiler)
    }

    // port: LinkedFlowScope#createEntryLattice
    /// Creates an entry lattice for the flow.
    pub fn create_entry_lattice(scope: TypedScope) -> Arc<LinkedFlowScope> {
        Arc::new(Self::new(HamtPMap::empty(), scope, scope))
    }

    // port: LinkedFlowScope#getRootOfQualifiedName
    fn get_root_of_qualified_name(name: &JsString) -> JsString {
        let index = name.index_of_char(u16::from(b'.'));
        if index < 0 {
            name.clone()
        } else {
            name.substring(0, index as usize)
        }
    }

    // port: LinkedFlowScope#getOverlayScopeForName
    /// Returns the overlay scope corresponding to this qualified name
    fn get_overlay_scope_for_name(
        &self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        create: bool,
    ) -> Option<OverlayScopeRef> {
        let root_var = self
            .syntactic_scope
            .get_var(compiler, Self::get_root_of_qualified_name(name));
        let scope = root_var.map(|root_var| root_var.get_scope(compiler));
        let scope = scope.unwrap_or(self.function_scope);
        self.get_overlay_scope_for_scope(scope, create)
    }

    // port: LinkedFlowScope#getOverlayScopeForScope
    /// Returns the overlay scope corresponding to this syntactic scope.
    fn get_overlay_scope_for_scope(
        &self,
        scope: TypedScope,
        create: bool,
    ) -> Option<OverlayScopeRef> {
        let overlay = self.scopes.get(&scope).cloned();
        if overlay.is_none() && create {
            return Some(OverlayScopeRef(Arc::new(OverlayScope::new(scope))));
        }
        overlay
    }

    // port: LinkedFlowScope#getCommonParentDeclarationScope
    pub fn get_common_parent_declaration_scope(
        compiler: &AbstractCompiler,
        left: &LinkedFlowScope,
        right: &LinkedFlowScope,
    ) -> TypedScope {
        if left.flows_from_bottom(compiler) {
            return right.syntactic_scope;
        } else if right.flows_from_bottom(compiler) {
            return left.syntactic_scope;
        }
        left.syntactic_scope
            .get_common_parent(compiler, right.syntactic_scope)
    }

    // port: LinkedFlowScope#equals
    /// Java's `equals` compares slot types with `differsFrom`, which needs the registry.
    pub fn equals(&self, compiler: &mut AbstractCompiler, other: &dyn FlowScope) -> bool {
        let Some(that) = other.as_any().downcast_ref::<LinkedFlowScope>() else {
            return false;
        };
        // If two flow scopes are in the same function, then they could have
        // two possible function scopes: the real one and the BOTTOM scope.
        // If they have different function scopes, we *should* iterate through all
        // the variables in each scope and compare. However, 99.9% of the time,
        // they're not equal. And the other .1% of the time, we can pretend
        // they're equal--this just means that data flow analysis will have
        // to propagate the entry lattice a little bit further than it
        // really needs to. Everything will still come out ok.
        self.function_scope == that.function_scope
            && self.scopes.equivalent(&that.scopes, |left, right| {
                Self::equal_scopes(compiler, left, right)
            })
    }

    // port: LinkedFlowScope#equalScopes
    fn equal_scopes(
        compiler: &mut AbstractCompiler,
        left: &OverlayScopeRef,
        right: &OverlayScopeRef,
    ) -> bool {
        if left == right {
            return true;
        }
        left.0.slots.equivalent(&right.0.slots, |slot_a, slot_b| {
            Self::equal_slots(compiler, slot_a, slot_b)
        })
    }

    // port: LinkedFlowScope#equalSlots
    /// Determines whether two slots are meaningfully different for the purposes of data flow
    /// analysis.
    fn equal_slots(
        compiler: &mut AbstractCompiler,
        slot_a: &OverlaySlotRef,
        slot_b: &OverlaySlotRef,
    ) -> bool {
        if slot_a == slot_b {
            return true;
        }
        let (reg, ast) = compiler.get_type_registry_and_ast();
        !check_not_null!(slot_a.0.type_).differs_from(reg, ast, check_not_null!(slot_b.0.type_))
    }

    // port: LinkedFlowScope#join
    #[allow(clippy::unnecessary_unwrap)] // Java's `fnSlotType == null || ..` branch shape
    fn join(
        compiler: &mut AbstractCompiler,
        linked_a: &LinkedFlowScope,
        linked_b: &LinkedFlowScope,
        common_parent: TypedScope,
    ) -> HamtPMap<TypedScope, OverlayScopeRef> {
        let flows_from_bottom_a = linked_a.flows_from_bottom(compiler);
        let flows_from_bottom_b = linked_b.flows_from_bottom(compiler);
        let trimmed_a = linked_a.trim_scopes(compiler, common_parent);
        let trimmed_b = linked_b.trim_scopes(compiler, common_parent);
        trimmed_a.reconcile(
            &trimmed_b,
            &mut |_scope_key: &TypedScope,
                  scope_a: Option<&OverlayScopeRef>,
                  scope_b: Option<&OverlayScopeRef>| {
                let empty_slots = HamtPMap::empty();
                let slots_a = scope_a.map_or(&empty_slots, |scope| &scope.0.slots);
                let slots_b = scope_b.map_or(&empty_slots, |scope| &scope.0.slots);
                // TODO(sdh): Simplify this logic: we want the best non-bottom scope we can get,
                // for the purpose of (a) passing to the joined OverlayScope constructor, and
                // (b) joining types only present in one scope.
                let typed_scope_a = if flows_from_bottom_a {
                    None
                } else if let Some(scope_a) = scope_a {
                    Some(scope_a.0.scope)
                } else {
                    Some(check_not_null!(scope_b).0.scope)
                };
                let typed_scope_b = if flows_from_bottom_b {
                    None
                } else if let Some(scope_b) = scope_b {
                    Some(scope_b.0.scope)
                } else {
                    Some(check_not_null!(scope_a).0.scope)
                };
                let best_scope = typed_scope_a.or(typed_scope_b);
                let best_scope = best_scope.unwrap_or_else(|| match scope_a {
                    Some(scope_a) => scope_a.0.scope,
                    None => check_not_null!(scope_b).0.scope,
                });
                OverlayScopeRef(Arc::new(OverlayScope::new_with_slots(
                    best_scope,
                    slots_a.reconcile(
                        slots_b,
                        &mut |_slot_key: &JsString,
                              slot_a: Option<&OverlaySlotRef>,
                              slot_b: Option<&OverlaySlotRef>| {
                            // There are 5 different join cases:
                            // 1) The type is present in joinedScopeA, not in joinedScopeB,
                            //    and not in functionScope. Just use the one in A.
                            // 2) The type is present in joinedScopeB, not in joinedScopeA,
                            //    and not in functionScope. Just use the one in B.
                            // 3) The type is present in functionScope and joinedScopeA, but
                            //    not in joinedScopeB. Join the two types.
                            // 4) The type is present in functionScope and joinedScopeB, but
                            //    not in joinedScopeA. Join the two types.
                            // 5) The type is present in joinedScopeA and joinedScopeB. Join
                            //    the two types.
                            let name = match slot_a {
                                Some(slot_a) => slot_a.0.name.clone(),
                                None => check_not_null!(slot_b).0.name.clone(),
                            };
                            if slot_b.is_none_or(|slot_b| slot_b.0.type_.is_none()) {
                                let slot_a = check_not_null!(slot_a);
                                let fn_slot = typed_scope_b
                                    .and_then(|scope| scope.get_slot(compiler, name.clone()));
                                let fn_slot_type =
                                    fn_slot.and_then(|fn_slot| fn_slot.get_type(compiler));
                                if fn_slot_type.is_none() || fn_slot_type == slot_a.0.type_ {
                                    // Case #1
                                    return slot_a.clone();
                                } else {
                                    // Case #3
                                    let (reg, ast) = compiler.get_type_registry_and_ast();
                                    let joined_type = check_not_null!(slot_a.0.type_)
                                        .get_least_supertype(reg, ast, fn_slot_type.unwrap());
                                    return if Some(joined_type) == slot_a.0.type_ {
                                        slot_a.clone()
                                    } else {
                                        OverlaySlotRef::new(name, Some(joined_type))
                                    };
                                }
                            } else if slot_a.is_none_or(|slot_a| slot_a.0.type_.is_none()) {
                                let slot_b = check_not_null!(slot_b);
                                let fn_slot = typed_scope_a
                                    .and_then(|scope| scope.get_slot(compiler, name.clone()));
                                let fn_slot_type =
                                    fn_slot.and_then(|fn_slot| fn_slot.get_type(compiler));
                                if fn_slot_type.is_none() || fn_slot_type == slot_b.0.type_ {
                                    // Case #2
                                    return slot_b.clone();
                                } else {
                                    // Case #4
                                    let (reg, ast) = compiler.get_type_registry_and_ast();
                                    let joined_type = check_not_null!(slot_b.0.type_)
                                        .get_least_supertype(reg, ast, fn_slot_type.unwrap());
                                    return if Some(joined_type) == slot_b.0.type_ {
                                        slot_b.clone()
                                    } else {
                                        OverlaySlotRef::new(name, Some(joined_type))
                                    };
                                }
                            }
                            let slot_a = slot_a.unwrap();
                            let slot_b = slot_b.unwrap();
                            // Case #5
                            if slot_a.0.type_ == slot_b.0.type_ {
                                return slot_a.clone();
                            }
                            let (reg, ast) = compiler.get_type_registry_and_ast();
                            let joined_type = slot_a.0.type_.unwrap().get_least_supertype(
                                reg,
                                ast,
                                slot_b.0.type_.unwrap(),
                            );
                            if Some(joined_type) == slot_a.0.type_ {
                                slot_a.clone()
                            } else {
                                OverlaySlotRef::new(name, Some(joined_type))
                            }
                        },
                    ),
                )))
            },
        )
    }
}

impl FlowScope for LinkedFlowScope {
    // port: LinkedFlowScope#inferSlotType
    fn infer_slot_type(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        symbol: &JsString,
        type_: Option<TypeId>,
    ) -> Arc<dyn FlowScope> {
        let scope = check_not_null!(self.get_overlay_scope_for_name(compiler, symbol, true));
        let new_scope = scope.infer(symbol, type_);
        // Aggressively remove empty scopes to maintain a reasonable equivalence.
        let new_scopes = if !new_scope.0.slots.is_empty() {
            self.scopes.plus(scope.0.scope, new_scope)
        } else {
            self.scopes.minus(&scope.0.scope)
        };
        if !new_scopes.ptr_eq(&self.scopes) {
            Arc::new(LinkedFlowScope::new(
                new_scopes,
                self.syntactic_scope,
                self.function_scope,
            ))
        } else {
            self
        }
    }

    // port: LinkedFlowScope#inferQualifiedSlot
    fn infer_qualified_slot(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        symbol: &JsString,
        bottom_type: Option<TypeId>,
        inferred_type: TypeId,
        declared: bool,
    ) -> Arc<dyn FlowScope> {
        if self.function_scope.is_global(compiler) {
            // Do not infer qualified names on the global scope.  Ideally these would be
            // added to the scope by TypedScopeCreator, but if they are not, adding them
            // here causes scaling problems (large projects can have tens of thousands of
            // undeclared qualified names in the global scope) with no real benefit.
            return self;
        }
        let mut v = self.syntactic_scope.get_var(compiler, symbol.clone());
        if v.is_none() && !self.function_scope.is_bottom(compiler) {
            // NOTE(sdh): Qualified names are declared on scopes lazily via this method.
            // The difficulty is that it's not always clear which scope they need to be
            // defined on.  In particular, syntacticScope is wrong because it is often a
            // nested block scope that is ignored when branches are joined; functionScope
            // is also wrong because it could lead to ambiguity if the same root name is
            // declared in multiple different blocks.  Instead, the qualified name is declared
            // on the scope that owns the root, when possible. When the root is undeclared, the
            // qualified name is declared in the global scope, as only global variables can be
            // undeclared.
            let root_var = self
                .syntactic_scope
                .get_var(compiler, Self::get_root_of_qualified_name(symbol));
            let root_scope = match root_var {
                Some(root_var) => root_var.get_scope(compiler),
                None => self.syntactic_scope.get_global_scope(compiler),
            };
            let input = NodeUtil::get_input_id(compiler, node)
                .and_then(|input_id| compiler.get_input(&input_id).cloned());
            v = Some(root_scope.declare(
                compiler,
                symbol.clone(),
                Some(node),
                bottom_type,
                input,
                !declared,
            ));
        }

        let declared_type = v.and_then(|v| v.get_type(compiler));
        if let Some(v) = v {
            if !v.is_type_inferred(compiler) {
                // Use the inferred type over the declared type only if the
                // inferred type is a strict subtype of the declared type.
                let (reg, ast) = compiler.get_type_registry_and_ast();
                if declared_type.is_none()
                    || !inferred_type.is_subtype_of(reg, ast, declared_type.unwrap())
                    || declared_type
                        .unwrap()
                        .is_subtype_of(reg, ast, inferred_type)
                    || inferred_type.equals(reg, ast, declared_type)
                {
                    return self;
                }
            } else if let Some(declared_type) = declared_type {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                if !inferred_type.is_subtype_of(reg, ast, declared_type) {
                    // If this inferred type is incompatible with another type previously
                    // inferred and stored on the scope, then update the scope.
                    let joined = declared_type.get_least_supertype(reg, ast, inferred_type);
                    v.set_type(compiler, Some(joined));
                }
            }
        }
        self.infer_slot_type(compiler, symbol, Some(inferred_type))
    }

    // port: LinkedFlowScope#getTypeOfThis
    fn get_type_of_this(&self, compiler: &mut AbstractCompiler) -> Option<TypeId> {
        self.syntactic_scope.get_type_of_this(compiler)
    }

    // port: LinkedFlowScope#getRootNode
    fn get_root_node(&self, compiler: &AbstractCompiler) -> NodeId {
        self.syntactic_scope.get_root_node(compiler)
    }

    // port: LinkedFlowScope#getParentScope
    fn get_parent_scope(&self, _compiler: &AbstractCompiler) -> Option<TypedScope> {
        panic!("UnsupportedOperationException")
    }

    // port: LinkedFlowScope#getSlot
    /// Get the slot for the given symbol.
    fn get_slot(&self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<FlowSlot> {
        let var = self.syntactic_scope.get_var(compiler, name.clone());
        let scope = match var {
            None => self.get_overlay_scope_for_name(compiler, name, false),
            Some(var) => {
                let var_scope = var.get_scope(compiler);
                self.get_overlay_scope_for_scope(var_scope, false)
            }
        };
        match scope {
            Some(scope) => scope.get_slot(compiler, name),
            None => var.map(FlowSlot::Var),
        }
    }

    // port: LinkedFlowScope#getOwnSlot
    fn get_own_slot(&self, _compiler: &mut AbstractCompiler, _name: &JsString) -> Option<FlowSlot> {
        panic!("UnsupportedOperationException")
    }

    // port: LinkedFlowScope#withSyntacticScope
    fn with_syntactic_scope(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
    ) -> Arc<dyn FlowScope> {
        let typed_scope = scope;
        if scope != self.syntactic_scope {
            Arc::new(LinkedFlowScope::new(
                self.trim_scopes(compiler, typed_scope),
                typed_scope,
                self.function_scope,
            ))
        } else {
            self
        }
    }

    // port: LinkedFlowScope#getDeclarationScope
    fn get_declaration_scope(&self, _compiler: &AbstractCompiler) -> TypedScope {
        self.syntactic_scope
    }

    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

impl Annotation for LinkedFlowScope {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    // port: LinkedFlowScope#hashCode
    fn hash_code(&self) -> i32 {
        panic!("UnsupportedOperationException")
    }
    /// Java's Object#toString; the identity hash is JVM-dependent, so the port prints 0.
    fn to_string(&self) -> String {
        format!("com.google.javascript.jscomp.LinkedFlowScope@{:x}", 0)
    }
}
impl LatticeElement for LinkedFlowScope {}

/// Join the two FlowScopes.
#[derive(Default)]
pub struct FlowScopeJoinOp {
    result: Option<Arc<LinkedFlowScope>>,
}

impl FlowScopeJoinOp {
    // port: LinkedFlowScope.FlowScopeJoinOp#FlowScopeJoinOp
    pub fn new() -> Self {
        Self { result: None }
    }

    // port: LinkedFlowScope.FlowScopeJoinOp#joinFlow
    // NOTE(sdh): When joining flow scopes with different syntactic scopes,
    // we do not attempt to recover the correct syntactic scope.  This is
    // okay because joins only occur in two situations: (1) performed by
    // the DataFlowAnalysis class automatically between CFG nodes, and (2)
    // requested manually while traversing a single expression within a CFG
    // node.  The syntactic scope is always set at the beginning of flowing
    // through a CFG node.  In the case of (1), the join result's syntactic
    // scope is immediately replaced with the correct one when we flow through
    // the next node.  In the case of (2), both inputs will always have the
    // same syntactic scope.  So simply propagating either input's scope is
    // perfectly fine.
    pub fn join_flow(&mut self, compiler: &mut AbstractCompiler, input: Arc<dyn FlowScope>) {
        // To join the two scopes, we have to
        let linked_input: Arc<LinkedFlowScope> = input
            .as_any_arc()
            .downcast::<LinkedFlowScope>()
            .unwrap_or_else(|_| panic!("ClassCastException"));
        let Some(result) = self.result.clone() else {
            self.result = Some(linked_input);
            return;
        };
        if result.scopes.ptr_eq(&linked_input.scopes)
            && result.function_scope == linked_input.function_scope
        {
            return;
        }
        // NOTE: it would be nice to put 'null' as the syntactic scope if they're not
        // equal, but this is not currently feasible.  For joins that occur within a
        // single CFG node's flow, it's irrelevant, but for joins between separate
        // CFG nodes, there is *one* place where the syntactic scope is actually used:
        // when joining more than two scopes, the first two scopes are joined, and
        // then the join result is joined with the third.  When joining, we look up
        // the types (and existence) of vars in one scope in the other; so when a var
        // from the third scope (say, a local) is missing from the join result, it
        // looks through the syntactic scope before realizing  this.  A quick fix
        // might be to just check that the scope is non-null before trying to join;
        // a better long-term fix would be to improve how we do joins to avoid
        // excessive map entry creation: find a common ancestor, etc.  One
        // interesting consequence of the current approach is that we may end up
        // adding irrelevant block-local variables to the joined scope unnecessarily.
        let common =
            LinkedFlowScope::get_common_parent_declaration_scope(compiler, &result, &linked_input);
        let function_scope = if result.flows_from_bottom(compiler) {
            linked_input.function_scope
        } else {
            result.function_scope
        };
        self.result = Some(Arc::new(LinkedFlowScope::new(
            LinkedFlowScope::join(compiler, &result, &linked_input, common),
            common,
            function_scope,
        )));
    }

    // port: LinkedFlowScope.FlowScopeJoinOp#finish
    pub fn finish(self) -> Option<Arc<dyn FlowScope>> {
        self.result.map(|result| result as Arc<dyn FlowScope>)
    }
}

#[derive(Debug)]
struct OverlayScope {
    scope: TypedScope,
    slots: HamtPMap<JsString, OverlaySlotRef>,
}

/// Java object identity for OverlayScope values (HamtPMap compares values with `equals`, which
/// OverlayScope does not override).
#[derive(Debug, Clone)]
struct OverlayScopeRef(Arc<OverlayScope>);
impl PartialEq for OverlayScopeRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl OverlayScope {
    // port: LinkedFlowScope.OverlayScope#OverlayScope(TypedScope)
    fn new(scope: TypedScope) -> Self {
        Self {
            scope,
            slots: HamtPMap::empty(),
        }
    }

    // port: LinkedFlowScope.OverlayScope#OverlayScope(TypedScope, PMap)
    fn new_with_slots(scope: TypedScope, slots: HamtPMap<JsString, OverlaySlotRef>) -> Self {
        Self { scope, slots }
    }
}

impl OverlayScopeRef {
    // port: LinkedFlowScope.OverlayScope#infer
    fn infer(&self, name: &JsString, type_: Option<TypeId>) -> OverlayScopeRef {
        // TODO(sdh): variants that do or don't clobber properties (i.e. look up and modify instead)
        let slot = self.0.slots.get(name);
        if slot.is_some_and(|slot| type_ == slot.0.type_) {
            return self.clone();
        }
        OverlayScopeRef(Arc::new(OverlayScope::new_with_slots(
            self.0.scope,
            self.0
                .slots
                .plus(name.clone(), OverlaySlotRef::new(name.clone(), type_)),
        )))
    }

    // port: LinkedFlowScope.OverlayScope#getSlot
    fn get_slot(&self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<FlowSlot> {
        match self.0.slots.get(name) {
            Some(slot) => Some(FlowSlot::Overlay(slot.0.clone())),
            None => self
                .0
                .scope
                .get_slot(compiler, name.clone())
                .map(FlowSlot::Var),
        }
    }
}

#[derive(Debug)]
pub struct OverlaySlot {
    // TODO(sdh): add a final PMap<String, OverlaySlot> for properties
    name: JsString,
    type_: Option<TypeId>,
}

/// Java object identity for OverlaySlot values.
#[derive(Debug, Clone)]
struct OverlaySlotRef(Arc<OverlaySlot>);
impl PartialEq for OverlaySlotRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl OverlaySlotRef {
    // port: LinkedFlowScope.OverlaySlot#OverlaySlot
    fn new(name: JsString, type_: Option<TypeId>) -> Self {
        Self(Arc::new(OverlaySlot { name, type_ }))
    }
}

impl OverlaySlot {
    // port: LinkedFlowScope.OverlaySlot#getName
    pub fn get_name(&self) -> JsString {
        self.name.clone()
    }

    // port: LinkedFlowScope.OverlaySlot#getType
    pub fn get_type(&self) -> Option<TypeId> {
        self.type_
    }

    // port: LinkedFlowScope.OverlaySlot#isTypeInferred
    pub fn is_type_inferred(&self) -> bool {
        true
    }

    // port: LinkedFlowScope.OverlaySlot#getDeclaration
    pub fn get_declaration(&self) -> Option<crate::typed_var::TypedVar> {
        None
    }

    // port: LinkedFlowScope.OverlaySlot#getJSDocInfo
    pub fn get_jsdoc_info(&self) -> Option<Arc<JSDocInfo>> {
        None
    }

    // port: LinkedFlowScope.OverlaySlot#getScope
    pub fn get_scope(&self) -> TypedScope {
        panic!("UnsupportedOperationException")
    }
}
