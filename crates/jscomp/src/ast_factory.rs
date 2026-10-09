/*
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
//   src/com/google/javascript/jscomp/AstFactory.java.

//! Port of `AstFactory.java`: creates AST nodes and subtrees.
//!
//! This class supports creating nodes either with or without type information.
//!
//! The idea is that client code can create the trees of nodes it needs without having to contain
//! logic for deciding whether type information should be added or not, and only minimal logic for
//! determining which types to add when they are necessary. Most methods in this class are able to
//! determine the correct type information from already existing AST nodes and the current scope.
//!
//! AstFactory supports both Closure types (see `JSType`) and optimization-only types (see
//! [`Color`]s). Colors contain less information than JSTypes which puts some restrictions on the
//! amount of inference this class can do. For example, there's no way to ask "What is the color of
//! property 'x' on receiver color 'obj'". This is why many methods accept a StaticScope instead of a
//! Scope: you may pass in a `GlobalNamespace` or similar object which contains fully qualified
//! names, to look up colors for an entire property chain.
//!
//! IMPORTANT: The methods in this class should never set source reference information. It is the
//! responsibility of the client code to set the correct source reference information. If we were to
//! make guesses here, that would lead to client code that sometimes does and sometimes doesn't set
//! the source reference and force the reader of that code to determine whether the default guess we
//! have here is really correct or not. It's better to have the decision made explicitly in the
//! client code.
//!
//! Rust-only: Java's factory holds the `JSTypeRegistry` it was created with; here the registry is
//! owned by the Compiler (or a test), so the methods take an [`AstFactoryContext`] that hands out
//! the arena and that registry together. Methods taking a `StaticScope` take an
//! [`AstFactoryStaticScope`], which reads the context (jscomp's syntactic scopes are ids into the
//! Compiler's scope arena).
//!
//! TODO(b/193800507): delete the methods in this class that only work for JSTypes but not colors.

use crate::{
    abstract_compiler::LifeCycleStage,
    compiler::Compiler,
    js::runtime_js_lib_manager::{JsLibField, RuntimeJsLibManager},
    node_util::NodeUtil,
    scope::ScopeId,
    typed_scope::TypedScope,
};
use closure_jstype::{
    JSTypeRegistry, TypeId, function_type::FunctionType, js_type::JSType,
    js_type_native::JSTypeNative, object_type, object_type::ObjectType,
    template_type_replacer::TemplateTypeReplacer,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jscomp_colors::{Color, ColorId, color_registry::ColorRegistry, standard_colors},
    node::{Ast, NodeId, Prop},
    static_scope::StaticScope,
    token::Token,
};
use std::sync::{Arc, LazyLock, Mutex};

// port: AstFactory#bigintNumberStringColor
static BIGINT_NUMBER_STRING_COLOR: LazyLock<Color> = LazyLock::new(|| {
    Color::create_union(&IndexSet::<_>::from_iter([
        standard_colors::BIGINT.clone(),
        standard_colors::STRING.clone(),
        standard_colors::NUMBER.clone(),
    ]))
});

/// Rust-only: the arena the factory builds in, with the `JSTypeRegistry` Java's factory was
/// created with (`None` where the context has none; only a JSTYPE-mode factory reads it).
pub trait AstFactoryContext {
    fn get_type_registry_and_ast_mut(&mut self) -> (Option<&mut JSTypeRegistry>, &mut Ast);
}

impl AstFactoryContext for Ast {
    fn get_type_registry_and_ast_mut(&mut self) -> (Option<&mut JSTypeRegistry>, &mut Ast) {
        (None, self)
    }
}

impl AstFactoryContext for Compiler {
    fn get_type_registry_and_ast_mut(&mut self) -> (Option<&mut JSTypeRegistry>, &mut Ast) {
        self.get_type_registry_field_and_ast_mut()
    }
}

/// Rust-only: an `Ast` paired with the registry a test or caller hands to
/// `AstFactory.createFactoryWithTypes(..)`.
pub struct TypedAstFactoryContext<'a> {
    pub ast: &'a mut Ast,
    pub type_registry: &'a mut JSTypeRegistry,
}

impl AstFactoryContext for TypedAstFactoryContext<'_> {
    fn get_type_registry_and_ast_mut(&mut self) -> (Option<&mut JSTypeRegistry>, &mut Ast) {
        (Some(&mut *self.type_registry), &mut *self.ast)
    }
}

/// Rust-only: what AstFactory asks of a `StaticScope`: `scope.getSlot(name)`, then the slot's
/// `getDeclaration().getNode()`.
pub trait AstFactoryStaticScope<C: ?Sized> {
    /// `None` when `getSlot(name)` is null; otherwise the slot's declaration node (`None` when the
    /// slot has no declaration or the declaration no node).
    fn get_slot_declaration_node(&self, cx: &mut C, name: &JsString) -> Option<Option<NodeId>>;
}

impl<C: ?Sized, S: StaticScope + ?Sized> AstFactoryStaticScope<C> for S {
    fn get_slot_declaration_node(&self, _cx: &mut C, name: &JsString) -> Option<Option<NodeId>> {
        self.get_slot(name).map(|slot| {
            slot.get_declaration()
                .map(|declaration| declaration.get_node())
        })
    }
}

impl AstFactoryStaticScope<Compiler> for ScopeId {
    fn get_slot_declaration_node(
        &self,
        cx: &mut Compiler,
        name: &JsString,
    ) -> Option<Option<NodeId>> {
        let var = self.get_slot(cx, name)?;
        Some(
            var.get_declaration(cx)
                .and_then(|declaration| declaration.get_node(cx)),
        )
    }
}

// port: AstFactory.TypeMode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum TypeMode {
    JSTYPE,
    COLOR,
    NONE,
}

pub struct AstFactory {
    color_registry: Option<Arc<ColorRegistry>>,
    // We need the unknown type so frequently, it's worth caching it.
    unknown_type: Option<TypeId>,
    type_mode: TypeMode,
    life_cycle_stage: LifeCycleStage,
    runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
}

/// Typed borrowing view for java.lang.reflect.Field replay; the instance fields remain private.
pub struct AstFactoryReplayFields<'a> {
    pub color_registry: &'a Option<Arc<ColorRegistry>>,
    pub unknown_type: &'a Option<TypeId>,
    pub type_mode: &'a TypeMode,
    pub life_cycle_stage: &'a LifeCycleStage,
    pub runtime_js_lib_manager: &'a Option<Arc<Mutex<RuntimeJsLibManager>>>,
}

impl AstFactory {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> AstFactoryReplayFields<'_> {
        AstFactoryReplayFields {
            color_registry: &self.color_registry,
            unknown_type: &self.unknown_type,
            type_mode: &self.type_mode,
            life_cycle_stage: &self.life_cycle_stage,
            runtime_js_lib_manager: &self.runtime_js_lib_manager,
        }
    }
}

impl AstFactory {
    // port: AstFactory#AstFactory(LifeCycleStage, JSTypeRegistry, RuntimeJsLibManager)
    fn new_with_types(
        life_cycle_stage: LifeCycleStage,
        registry: &JSTypeRegistry,
        runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
    ) -> Self {
        Self {
            life_cycle_stage,
            runtime_js_lib_manager,
            color_registry: None,
            unknown_type: Some(Self::get_native_type(
                Some(registry),
                JSTypeNative::UNKNOWN_TYPE,
            )),
            type_mode: TypeMode::JSTYPE,
        }
    }

    // port: AstFactory#AstFactory(LifeCycleStage, RuntimeJsLibManager)
    fn new(
        life_cycle_stage: LifeCycleStage,
        runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
    ) -> Self {
        Self {
            life_cycle_stage,
            runtime_js_lib_manager,
            color_registry: None,
            unknown_type: None,
            type_mode: TypeMode::NONE,
        }
    }

    // port: AstFactory#AstFactory(LifeCycleStage, ColorRegistry, RuntimeJsLibManager)
    fn new_with_colors(
        life_cycle_stage: LifeCycleStage,
        color_registry: Arc<ColorRegistry>,
        runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
    ) -> Self {
        Self {
            life_cycle_stage,
            runtime_js_lib_manager,
            color_registry: Some(color_registry),
            unknown_type: None,
            type_mode: TypeMode::COLOR,
        }
    }

    // port: AstFactory#createFactoryWithoutTypes
    pub fn create_factory_without_types(
        life_cycle_stage: LifeCycleStage,
        runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
    ) -> Self {
        Self::new(life_cycle_stage, runtime_js_lib_manager)
    }

    // port: AstFactory#createFactoryWithTypes
    pub fn create_factory_with_types(
        life_cycle_stage: LifeCycleStage,
        registry: &JSTypeRegistry,
        runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
    ) -> Self {
        Self::new_with_types(life_cycle_stage, registry, runtime_js_lib_manager)
    }

    // port: AstFactory#createFactoryWithColors
    pub fn create_factory_with_colors(
        life_cycle_stage: LifeCycleStage,
        color_registry: Arc<ColorRegistry>,
        runtime_js_lib_manager: Option<Arc<Mutex<RuntimeJsLibManager>>>,
    ) -> Self {
        Self::new_with_colors(life_cycle_stage, color_registry, runtime_js_lib_manager)
    }

    /// Does this class instance add types to the nodes it creates?
    // port: AstFactory#isAddingTypes
    pub fn is_adding_types(&self) -> bool {
        TypeMode::JSTYPE == self.type_mode
    }

    /// Does this class instance add optimization colors to the nodes it creates?
    // port: AstFactory#isAddingColors
    pub fn is_adding_colors(&self) -> bool {
        TypeMode::COLOR == self.type_mode
    }

    // port: AstFactory#assertNotAddingColors
    fn assert_not_adding_colors(&self) {
        check_state!(!self.is_adding_colors(), "method not supported for colors");
    }

    /// Returns a new EXPR_RESULT node.
    ///
    /// Statements have no type information, so this is functionally the same as calling
    /// `IR.exprResult(expr)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#exprResult
    pub fn expr_result<C: AstFactoryContext + ?Sized>(&self, cx: &mut C, expr: NodeId) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        // TODO(bradfordcsmith): This method should not be calling .srcref()
        IR::expr_result(ast, expr).srcref(ast, expr)
    }

    /// Returns a new EMPTY node.
    ///
    /// EMPTY Nodes have no type information, so this is functionally the same as calling
    /// `IR.empty()`. It exists so that a pass can be consistent about always using `AstFactory`
    /// to create new nodes.
    // port: AstFactory#createEmpty
    pub fn create_empty<C: AstFactoryContext + ?Sized>(&self, cx: &mut C) -> NodeId {
        IR::empty(cx.get_type_registry_and_ast_mut().1)
    }

    /// Returns a new BLOCK node.
    ///
    /// Blocks have no type information, so this is functionally the same as calling
    /// `IR.block(statements)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createBlock
    pub fn create_block<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        statements: &[NodeId],
    ) -> NodeId {
        IR::block_with_children(cx.get_type_registry_and_ast_mut().1, statements)
    }

    /// Returns a new IF node.
    ///
    /// Blocks have no type information, so this is functionally the same as calling
    /// `IR.ifNode(cond, then)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createIf(Node, Node)
    pub fn create_if<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        cond: NodeId,
        then: NodeId,
    ) -> NodeId {
        IR::if_node(cx.get_type_registry_and_ast_mut().1, cond, then)
    }

    /// Returns a new IF node.
    ///
    /// Blocks have no type information, so this is functionally the same as calling
    /// `IR.ifNode(cond, then, elseNode)`. It exists so that a pass can be consistent about always
    /// using `AstFactory` to create new nodes.
    // port: AstFactory#createIf(Node, Node, Node)
    pub fn create_if_with_else<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        cond: NodeId,
        then: NodeId,
        else_node: NodeId,
    ) -> NodeId {
        IR::if_node_with_else(cx.get_type_registry_and_ast_mut().1, cond, then, else_node)
    }

    /// Returns a new FOR node.
    ///
    /// Blocks have no type information, so this is functionally the same as calling
    /// `IR.forNode(init, cond, incr, body)`. It exists so that a pass can be consistent about
    /// always using `AstFactory` to create new nodes.
    // port: AstFactory#createFor
    pub fn create_for<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        init: NodeId,
        cond: NodeId,
        incr: NodeId,
        body: NodeId,
    ) -> NodeId {
        IR::for_node(cx.get_type_registry_and_ast_mut().1, init, cond, incr, body)
    }

    /// Returns a new BREAK node.
    ///
    /// Breaks have no type information, so this is functionally the same as calling
    /// `IR.breakNode()`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createBreak
    pub fn create_break<C: AstFactoryContext + ?Sized>(&self, cx: &mut C) -> NodeId {
        IR::break_node(cx.get_type_registry_and_ast_mut().1)
    }

    /// Returns a new LABEL node.
    ///
    /// Labels have no type information, so this is functionally the same as calling
    /// `IR.label(label, stmt)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createLabel
    pub fn create_label<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        label: NodeId,
        stmt: NodeId,
    ) -> NodeId {
        IR::label(cx.get_type_registry_and_ast_mut().1, label, stmt)
    }

    /// Returns a new CATCH node.
    ///
    /// Catch nodes have no type information, so this is functionally the same as calling
    /// `IR.catchNode(error, block)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createCatch
    pub fn create_catch<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        error: NodeId,
        block: NodeId,
    ) -> NodeId {
        IR::catch_node(cx.get_type_registry_and_ast_mut().1, error, block)
    }

    /// Returns a new TRY node.
    ///
    /// Try nodes have no type information, so this is functionally the same as calling
    /// `IR.tryFinally(tryBlock, finallyBlock)`. It exists so that a pass can be consistent about
    /// always using `AstFactory` to create new nodes.
    // port: AstFactory#createTryFinally
    pub fn create_try_finally<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        try_block: NodeId,
        finally_block: NodeId,
    ) -> NodeId {
        IR::try_finally(
            cx.get_type_registry_and_ast_mut().1,
            try_block,
            finally_block,
        )
    }

    /// Returns a new TRY node.
    ///
    /// Try nodes have no type information, so this is functionally the same as calling
    /// `IR.tryCatchFinally(tryBlock, catchNode, finallyBlock)`. It exists so that a pass can be
    /// consistent about always using `AstFactory` to create new nodes.
    // port: AstFactory#createTryCatchFinally
    pub fn create_try_catch_finally<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        try_block: NodeId,
        catch_node: NodeId,
        finally_block: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        check_state!(try_block.is_block(ast));
        check_state!(catch_node.is_catch(ast));
        check_state!(finally_block.is_block(ast));
        IR::try_catch_finally(ast, try_block, catch_node, finally_block)
    }

    /// Returns a new THROW node.
    ///
    /// Throw nodes have no type information, so this is functionally the same as calling
    /// `IR.throwNode(value)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createThrow
    pub fn create_throw<C: AstFactoryContext + ?Sized>(&self, cx: &mut C, expr: NodeId) -> NodeId {
        IR::throw_node(cx.get_type_registry_and_ast_mut().1, expr)
    }

    /// Returns a new `return` statement.
    ///
    /// Return statements have no type information, so this is functionally the same as calling
    /// `IR.return(value)`. It exists so that a pass can be consistent about always using
    /// `AstFactory` to create new nodes.
    // port: AstFactory#createReturn
    pub fn create_return<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        value: NodeId,
    ) -> NodeId {
        IR::return_node_with_expression(cx.get_type_registry_and_ast_mut().1, value)
    }

    /// Returns a new `yield` expression.
    ///
    /// - `type`: Type we expect to get back after the yield
    /// - `value`: value to yield
    // port: AstFactory#createYield
    pub fn create_yield<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        type_: Type,
        value: NodeId,
    ) -> NodeId {
        let result = IR::yield_node(cx.get_type_registry_and_ast_mut().1, value);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Returns a new `await` expression.
    ///
    /// - `type`: Type we expect to get back after the await
    /// - `value`: value to await
    // port: AstFactory#createAwait
    pub fn create_await<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        type_: Type,
        value: NodeId,
    ) -> NodeId {
        let result = IR::r#await(cx.get_type_registry_and_ast_mut().1, value);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createString
    pub fn create_string<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        value: impl Into<JsString>,
    ) -> NodeId {
        let result = IR::string(cx.get_type_registry_and_ast_mut().1, value);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(JSTypeNative::STRING_TYPE, standard_colors::STRING.clone()),
            result,
        );
        result
    }

    // port: AstFactory#createNumber
    pub fn create_number<C: AstFactoryContext + ?Sized>(&self, cx: &mut C, value: f64) -> NodeId {
        let result = IR::number(cx.get_type_registry_and_ast_mut().1, value);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(JSTypeNative::NUMBER_TYPE, standard_colors::NUMBER.clone()),
            result,
        );
        result
    }

    // port: AstFactory#createBoolean
    pub fn create_boolean<C: AstFactoryContext + ?Sized>(&self, cx: &mut C, value: bool) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        let result = if value {
            IR::true_node(ast)
        } else {
            IR::false_node(ast)
        };
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createNull
    pub fn create_null<C: AstFactoryContext + ?Sized>(&self, cx: &mut C) -> NodeId {
        let result = IR::null_node(cx.get_type_registry_and_ast_mut().1);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::NULL_TYPE,
                standard_colors::NULL_OR_VOID.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createVoid
    pub fn create_void<C: AstFactoryContext + ?Sized>(&self, cx: &mut C, child: NodeId) -> NodeId {
        let result = IR::void_node(cx.get_type_registry_and_ast_mut().1, child);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::VOID_TYPE,
                standard_colors::NULL_OR_VOID.clone(),
            ),
            result,
        );
        result
    }

    /// Returns a new node representing the undefined value.
    // port: AstFactory#createUndefinedValue
    pub fn create_undefined_value<C: AstFactoryContext + ?Sized>(&self, cx: &mut C) -> NodeId {
        // We prefer `void 0` as being shorter than `undefined`.
        // Also, it's technically possible for malicious code to assign a value to `undefined`.
        let zero = self.create_number(cx, 0.0);
        self.create_void(cx, zero)
    }

    // port: AstFactory#createNot
    pub fn create_not<C: AstFactoryContext + ?Sized>(&self, cx: &mut C, child: NodeId) -> NodeId {
        let result = IR::not(cx.get_type_registry_and_ast_mut().1, child);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    /// Creates a THIS node with the correct type for the given function node.
    // port: AstFactory#createThis
    pub fn create_this<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        this_type: Type,
    ) -> NodeId {
        let result = IR::this_node(cx.get_type_registry_and_ast_mut().1);
        self.set_js_type_or_color(cx, this_type, result);
        result
    }

    /// Creates a SUPER node with the correct type for the given function node.
    // port: AstFactory#createSuper
    pub fn create_super<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        super_type: Type,
    ) -> NodeId {
        let result = IR::super_node(cx.get_type_registry_and_ast_mut().1);
        self.set_js_type_or_color(cx, super_type, result);
        result
    }

    /// Creates a NEW_TARGET node with the given type.
    // port: AstFactory#createNewTarget
    pub fn create_new_target<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        new_target_type: Type,
    ) -> NodeId {
        let result = IR::new_target(cx.get_type_registry_and_ast_mut().1);
        self.set_js_type_or_color(cx, new_target_type, result);
        result
    }

    /// Creates a THIS node with the correct type for the given ES6 class node.
    ///
    /// With the optimization colors type system, we can support inferring the type of this for
    /// constructors but not generic functions annotated @this
    // port: AstFactory#createThisForEs6Class
    pub fn create_this_for_es6_class<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        function_node: NodeId,
    ) -> NodeId {
        {
            let ast = cx.get_type_registry_and_ast_mut().1;
            check_state!(
                function_node.is_class(ast),
                "%s",
                function_node.to_string(ast)
            );
        }
        let result = IR::this_node(cx.get_type_registry_and_ast_mut().1);
        let type_ = self.get_type_of_this_for_es6_class(cx, function_node);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Creates a THIS node with the correct type for the given ES6 class member function node.
    // port: AstFactory#createThisForEs6ClassMember
    pub fn create_this_for_es6_class_member<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        member_node: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        check_argument!(member_node.get_parent(ast).unwrap().is_class_members(ast));
        check_argument!(
            member_node.is_member_function_def(ast)
                || member_node.is_member_field_def(ast)
                || member_node.is_computed_field_def(ast)
        );

        let class_node = member_node.get_grandparent(ast).unwrap();
        if member_node.is_static_member(ast) {
            let result = IR::this_node(ast);
            self.set_js_type_or_color(cx, Self::type_node(class_node), result);
            result
        } else {
            self.create_this_for_es6_class(cx, class_node)
        }
    }

    // port: AstFactory#getTypeOfThisForFunctionNode
    fn get_type_of_this_for_function_node<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        function_node: NodeId,
    ) -> Option<TypeId> {
        self.assert_not_adding_colors();
        if self.is_adding_types() {
            let function_type = self.get_function_type(cx, function_node);
            let registry = cx.get_type_registry_and_ast_mut().0;
            let registry = registry.expect("registry is null");
            Some(
                ObjectType::get_type_of_this(function_type, registry)
                    .expect("NullPointerException: typeOfThis"),
            )
        } else {
            None // not adding type information
        }
    }

    // port: AstFactory#getTypeOfThisForEs6Class
    fn get_type_of_this_for_es6_class<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        function_node: NodeId,
    ) -> Type {
        {
            let ast = cx.get_type_registry_and_ast_mut().1;
            check_argument!(
                function_node.is_class(ast),
                "%s",
                function_node.to_string(ast)
            );
        }
        match self.type_mode {
            TypeMode::JSTYPE => {
                Self::type_jstype(self.get_type_of_this_for_function_node(cx, function_node))
            }
            TypeMode::COLOR => {
                let color = function_node.get_color(cx.get_type_registry_and_ast_mut().1);
                Self::type_(self.get_instance_of_color(color))
            }
            TypeMode::NONE => Self::no_type_information(),
        }
    }

    // port: AstFactory#getFunctionType
    fn get_function_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        function_node: NodeId,
    ) -> TypeId {
        {
            let ast = cx.get_type_registry_and_ast_mut().1;
            check_state!(
                function_node.is_function(ast) || function_node.is_class(ast),
                "not a function or class: %s",
                function_node.to_string(ast)
            );
        }
        self.assert_not_adding_colors();
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let registry = registry.expect("registry is null");
        // If the function declaration was cast to a different type, we want the original type
        // from before the cast.
        let type_before_cast = function_node.get_jstype_before_cast(ast);
        if let Some(type_before_cast) = type_before_cast {
            type_before_cast.assert_function_type(registry, ast)
        } else {
            function_node
                .get_jstype_required(ast)
                .assert_function_type(registry, ast)
        }
    }

    /// Creates a NAME node having the type of "this" appropriate for the given function node.
    // port: AstFactory#createThisAliasReferenceForEs6Class
    pub fn create_this_alias_reference_for_es6_class<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        alias_name: impl Into<JsString>,
        function_node: NodeId,
    ) -> NodeId {
        let type_ = self.get_type_of_this_for_es6_class(cx, function_node);
        self.create_name(cx, alias_name, type_)
    }

    /// Creates a statement declaring a const alias for "this".
    ///
    /// Returns a declaration statement such as `let name = value;` or `var name = value;` or
    /// `const name = value;`.
    // port: AstFactory#createSingleNameDeclaration
    pub fn create_single_name_declaration<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        token_type: Token,
        name: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        match token_type {
            Token::LET => self.create_single_let_name_declaration_with_value(cx, name, value),
            Token::VAR => self.create_single_var_name_declaration_with_value(cx, name, value),
            Token::CONST => self.create_single_const_name_declaration(cx, name, value),
            _ => panic!("UnsupportedOperationException: Unexpeted token type: {token_type:?}"),
        }
    }

    /// Creates a new `let` declaration for a single variable name with a void type and no JSDoc.
    ///
    /// e.g. `let variableName`
    // port: AstFactory#createSingleLetNameDeclaration(String)
    pub fn create_single_let_name_declaration<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        variable_name: impl Into<JsString>,
    ) -> NodeId {
        let name = self.create_name(
            cx,
            variable_name,
            Self::type_native_and_color(
                JSTypeNative::VOID_TYPE,
                standard_colors::NULL_OR_VOID.clone(),
            ),
        );
        IR::r#let(cx.get_type_registry_and_ast_mut().1, name)
    }

    /// Creates a new `let` declaration for a single variable name with type and value from the
    /// given value node.
    ///
    /// e.g. `let variableName = value;`
    // port: AstFactory#createSingleLetNameDeclaration(String, Node)
    pub fn create_single_let_name_declaration_with_value<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        variable_name: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        let name = self.create_name(cx, variable_name, Self::type_node(value));
        IR::let_with_value(cx.get_type_registry_and_ast_mut().1, name, value)
    }

    /// Creates a new `var` declaration statement for a single variable name with void type and no
    /// JSDoc.
    ///
    /// e.g. `var variableName`
    // port: AstFactory#createSingleVarNameDeclaration(String)
    pub fn create_single_var_name_declaration<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        variable_name: impl Into<JsString>,
    ) -> NodeId {
        let name = self.create_name(
            cx,
            variable_name,
            Self::type_native_and_color(
                JSTypeNative::VOID_TYPE,
                standard_colors::NULL_OR_VOID.clone(),
            ),
        );
        IR::var(cx.get_type_registry_and_ast_mut().1, name)
    }

    /// Creates a new `var` declaration statement for a single variable name with the given value
    /// and no JSDoc.
    ///
    /// e.g. `var variableName = value;`
    // port: AstFactory#createSingleVarNameDeclaration(String, Node)
    pub fn create_single_var_name_declaration_with_value<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        variable_name: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        let name = self.create_name(cx, variable_name, Self::type_node(value));
        IR::var_with_value(cx.get_type_registry_and_ast_mut().1, name, value)
    }

    /// Creates a new `const` declaration statement for a single variable name.
    ///
    /// Takes the type for the variable name from the value node.
    ///
    /// e.g. `const variableName = value;`
    // port: AstFactory#createSingleConstNameDeclaration
    pub fn create_single_const_name_declaration<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        variable_name: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        let type_ = {
            let ast = cx.get_type_registry_and_ast_mut().1;
            Self::type_jstype_and_color(value.get_jstype(ast), value.get_color(ast))
        };
        let name_node = self.create_constant_name(cx, variable_name, type_);
        IR::const_node(cx.get_type_registry_and_ast_mut().1, name_node, value)
    }

    /// Creates a new `const` declaration statement for an object pattern.
    ///
    /// e.g. `const {Foo} = value;`
    // port: AstFactory#createSingleConstObjectPatternDeclaration
    pub fn create_single_const_object_pattern_declaration<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        object_pattern: NodeId,
        value: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        check_state!(
            object_pattern.is_object_pattern(ast),
            "not an object pattern: %s",
            object_pattern.to_string(ast)
        );
        IR::const_node(ast, object_pattern, value)
    }

    /// Creates a reference to "arguments" with the type specified in externs, or unknown if the
    /// externs for it weren't included.
    // port: AstFactory#createArgumentsReference
    pub fn create_arguments_reference<C: AstFactoryContext + ?Sized>(&self, cx: &mut C) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::name(ast, "arguments");
        match self.type_mode {
            TypeMode::JSTYPE => {
                let arguments_type = registry
                    .expect("NullPointerException: registry")
                    .get_native_type(JSTypeNative::ARGUMENTS_TYPE);
                result.set_jstype(ast, Some(arguments_type));
            }
            TypeMode::COLOR => {
                let color = self
                    .color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::ARGUMENTS_ID);
                result.set_color(ast, Some(color));
            }
            TypeMode::NONE => {}
        }
        result
    }

    /// Creates a statement declaring a const alias for "arguments".
    ///
    /// Returns a statement like `const argsAlias = arguments;`.
    // port: AstFactory#createArgumentsAliasDeclaration
    pub fn create_arguments_alias_declaration<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        alias_name: impl Into<JsString>,
    ) -> NodeId {
        let arguments_reference = self.create_arguments_reference(cx);
        self.create_single_const_name_declaration(cx, alias_name, arguments_reference)
    }

    // port: AstFactory#createName(String, Type)
    pub fn create_name<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        type_: Type,
    ) -> NodeId {
        let name = name.into();
        check_argument!(
            name != "$jscomp",
            "Use createQName(RuntimeJsLibManager.Field) to reference $jscomp.* methods. Found: %s",
            name
        );
        let result = IR::name(cx.get_type_registry_and_ast_mut().1, name.clone());
        self.set_js_type_or_color(cx, type_, result);
        if self.life_cycle_stage.is_normalized() && name.starts_with("$jscomp") {
            // $jscomp will always be a constant and needs to be marked that way to satisfy
            // the normalization invariants.
            // TODO: b/322009741 - Stop depending on lifeCycleStage.isNormalized() and "$jscomp"
            // prefix to decide constness. The callers must explicitly use `createConstantName` is
            // they need a NAME node that's set with IS_CONSTANT_NAME prop.
            result.put_boolean_prop(
                cx.get_type_registry_and_ast_mut().1,
                Prop::IS_CONSTANT_NAME,
                true,
            );
        }
        result
    }

    // port: AstFactory#createConstantName
    pub fn create_constant_name<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        type_: Type,
    ) -> NodeId {
        let name = name.into();
        check_argument!(
            name != "$jscomp",
            "Use createQName(RuntimeJsLibManager.Field) to reference $jscomp.* methods. Found: %s",
            name
        );
        let result = IR::name(cx.get_type_registry_and_ast_mut().1, name);
        self.set_js_type_or_color(cx, type_, result);
        if self.life_cycle_stage.is_normalized() {
            // TODO: b/322009741 - Stop depending on lifeCycleStage.isNormalized() to decide
            // constness
            result.put_boolean_prop(
                cx.get_type_registry_and_ast_mut().1,
                Prop::IS_CONSTANT_NAME,
                true,
            );
        }
        result
    }

    /// Creates a new NAME node, setting its type from the definition of the name in the given
    /// scope (if the factory is adding types or colors).
    ///
    /// - `scope`: The scope in which the name is declared. Must not be `None` if the factory is
    ///   adding types or colors, or if the AST is normalized.
    // port: AstFactory#createName(StaticScope, String)
    pub fn create_name_in_scope<C, S>(
        &self,
        cx: &mut C,
        scope: Option<&S>,
        name: impl Into<JsString>,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let name = name.into();
        check_argument!(
            name != "$jscomp",
            "Use createQName(RuntimeJsLibManager.Field) to reference $jscomp.* methods. Found: %s",
            name
        );
        let result = IR::name(cx.get_type_registry_and_ast_mut().1, name.clone());

        if self.life_cycle_stage.is_normalized() || self.type_mode != TypeMode::NONE {
            // We need a scope to maintain normalization and / or propagate type information
            let scope = check_not_null!(
                scope,
                "A scope is required [lifeCycleStage: %s, typeMode: %s]",
                format!("{:?}", self.life_cycle_stage),
                format!("{:?}", self.type_mode)
            );
            let var = scope.get_slot_declaration_node(cx, &name);
            match var {
                None => {
                    // TODO(bradfordcsmith): Why is this special exception needed?
                    // There are a few cases where `$jscomp` isn't found in the code (implying that
                    // runtime library injection somehow didn't happen), but we do perform
                    // transpilations which require it to exist. Can we fix that?
                    // This only happens when type checking is not being done.
                    check_state!(
                        self.type_mode == TypeMode::NONE,
                        "Missing var %s in scope",
                        name
                    );
                }
                Some(declaration_node) => {
                    let var_definition_node = declaration_node
                        .expect("Cannot find type for var with missing declaration");
                    let ast = cx.get_type_registry_and_ast_mut().1;
                    // Normalization requires that all references to a constant variable have this
                    // property.
                    if var_definition_node.get_boolean_prop(ast, Prop::IS_CONSTANT_NAME) {
                        result.put_boolean_prop(ast, Prop::IS_CONSTANT_NAME, true);
                    }
                    match self.type_mode {
                        TypeMode::JSTYPE => {
                            let definition_type = var_definition_node.get_jstype(ast);
                            // TODO(b/149843534): crash instead of defaulting to unknown
                            result.set_jstype(
                                ast,
                                if definition_type.is_some() {
                                    definition_type
                                } else {
                                    self.unknown_type
                                },
                            );
                        }
                        TypeMode::COLOR => {
                            let definition_color = var_definition_node.get_color(ast);
                            // TODO(b/149843534): crash instead of defaulting to unknown
                            result.set_color(
                                ast,
                                Some(
                                    definition_color
                                        .unwrap_or_else(|| standard_colors::UNKNOWN.clone()),
                                ),
                            );
                        }
                        TypeMode::NONE => {}
                    }
                }
            }
        }
        result
    }

    // port: AstFactory#createNameWithUnknownType
    pub fn create_name_with_unknown_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
    ) -> NodeId {
        self.create_name(
            cx,
            name,
            Self::type_jstype_and_color(self.unknown_type, Some(standard_colors::UNKNOWN.clone())),
        )
    }

    /// Looks up the type of a name from a `TypedScope` created from typechecking, using the `JSType`
    /// API. Will crash if is called on an AstFactory that is created after JSType -> color
    /// conversion.
    ///
    /// Prefer `createQName(StaticScope, String)` if running after JSType -> color conversion.
    ///
    /// `global_typed_scope` must be the top, global scope.
    // port: AstFactory#createQNameUsingJSTypeInfo
    pub fn create_qname_using_js_type_info(
        &self,
        compiler: &mut Compiler,
        global_typed_scope: Option<TypedScope>,
        qname: &str,
    ) -> NodeId {
        check_argument!(
            global_typed_scope.is_none_or(|scope| scope.is_global(compiler)),
            "%s",
            global_typed_scope.map_or_else(|| "null".to_owned(), |scope| scope.to_string(compiler))
        );
        self.assert_not_adding_colors();
        // DOT_SPLITTER.splitToList(qname)
        let name_parts: Vec<&str> = qname.split('.').collect();
        check_state!(!name_parts.is_empty());

        let receiver_part = name_parts[0];
        let receiver = IR::name(compiler, receiver_part);
        if self.is_adding_types() {
            // globalTypedScope.getVar(receiverPart): a null scope is a NullPointerException.
            let global_typed_scope = global_typed_scope.expect("NullPointerException");
            let var = check_not_null!(
                global_typed_scope.get_var(compiler, receiver_part),
                receiver_part
            );
            let var_type = var.get_type(compiler);
            let var_type = var_type.unwrap_or_else(|| panic!("{}", var.to_string(compiler)));
            receiver.set_jstype(compiler, Some(var_type));
        }

        let other_parts = &name_parts[1..];
        self.create_get_props_without_colors(compiler, receiver, other_parts)
    }

    /// Creates a qualified name in the given scope.
    ///
    /// Only works if `isAddingColors()` is false.
    // port: AstFactory#createQName(StaticScope, String)
    pub fn create_qname<C, S>(&self, cx: &mut C, scope: &S, qname: &str) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        // DOT_SPLITTER.split(qname)
        let names: Vec<&str> = qname.split('.').collect();
        self.create_qname_from_names(cx, scope, &names)
    }

    /// Creates a qualified name in the given scope.
    ///
    /// Only works if `isAddingColors()` is false.
    // port: AstFactory#createQName(StaticScope, Iterable)
    pub fn create_qname_from_names<C, S>(&self, cx: &mut C, scope: &S, names: &[&str]) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let base_name = *check_not_null!(names.first());
        let property_names = &names[1..];
        self.create_qname_with_base_name(cx, scope, base_name, property_names)
    }

    /// Creates a qualified name in the given scope.
    ///
    /// Only works if `isAddingColors()` is false.
    // port: AstFactory#createQName(StaticScope, String, String...)
    // port: AstFactory#createQName(StaticScope, String, Iterable)
    pub fn create_qname_with_base_name<C, S>(
        &self,
        cx: &mut C,
        scope: &S,
        base_name: &str,
        property_names: &[&str],
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let base_name_node = self.create_name_in_scope(cx, Some(scope), base_name);
        self.create_qname_from_base_name_node(cx, scope, base_name, base_name_node, property_names)
    }

    // port: AstFactory#createQName(StaticScope, String, Node, Iterable)
    fn create_qname_from_base_name_node<C, S>(
        &self,
        cx: &mut C,
        scope: &S,
        base_name: &str,
        base_name_node: NodeId,
        property_names: &[&str],
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let mut qname = base_name_node;
        let mut name = base_name.to_string();
        for property_name in property_names {
            name += ".";
            name += property_name;
            // Java's `Type type = null` reaches setJSTypeOrColor only in NONE mode, where
            // noTypeInformation() is equally unused.
            let mut type_ = Self::no_type_information();
            if self.is_adding_types() || self.is_adding_colors() {
                let def = scope
                    .get_slot_declaration_node(cx, &JsString::from(name.as_str()))
                    .unwrap_or_else(|| panic!("Cannot find name {name} in StaticScope."))
                    .expect("NullPointerException: getDeclaration().getNode()");
                type_ = Self::type_node(def);
            }
            qname = self.create_get_prop(cx, qname, *property_name, type_);
        }
        qname
    }

    /// Creates a qualified name for a runtime library field, which must be injected.
    // port: AstFactory#createQName(StaticScope, JsLibField)
    pub fn create_qname_for_field<C, S>(
        &self,
        cx: &mut C,
        scope: &S,
        field: &dyn JsLibField,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let qname = field.assert_injected().qualified_name().to_string();
        let parts: Vec<&str> = qname.split('.').collect();
        let base_name = *check_not_null!(parts.first());
        check_state!(
            base_name.starts_with("$jscomp"),
            "Unexpected Field name %s",
            base_name
        );
        let base_name_node = if base_name == "$jscomp" {
            self.create_jscomp(cx)
        } else {
            self.create_name(
                cx,
                base_name,
                Self::type_jstype_and_color(
                    self.unknown_type,
                    Some(standard_colors::UNKNOWN.clone()),
                ),
            )
        };
        if self.life_cycle_stage.is_normalized() {
            base_name_node.put_boolean_prop(
                cx.get_type_registry_and_ast_mut().1,
                Prop::IS_CONSTANT_NAME,
                true,
            );
        }
        let property_names = &parts[1..];
        self.create_qname_from_base_name_node(cx, scope, base_name, base_name_node, property_names)
    }

    /// Creates a qualified name with unknown type.
    // port: AstFactory#createQNameWithUnknownType(String)
    pub fn create_qname_with_unknown_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        qname: &str,
    ) -> NodeId {
        let names: Vec<&str> = qname.split('.').collect();
        self.create_qname_with_unknown_type_from_names(cx, &names)
    }

    /// Creates a qualified name for a runtime library field with unknown type.
    // port: AstFactory#createQNameWithUnknownType(JsLibField)
    pub fn create_qname_with_unknown_type_for_field<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        field: &dyn JsLibField,
    ) -> NodeId {
        let qname = field.assert_injected().qualified_name().to_string();
        let parts: Vec<&str> = qname.split('.').collect();
        let base_name = *check_not_null!(parts.first());
        check_state!(
            base_name.starts_with("$jscomp"),
            "Unexpected Field name %s",
            base_name
        );
        let base_name_node = if base_name == "$jscomp" {
            self.create_jscomp(cx)
        } else {
            self.create_name(
                cx,
                base_name,
                Self::type_jstype_and_color(
                    self.unknown_type,
                    Some(standard_colors::UNKNOWN.clone()),
                ),
            )
        };
        if self.life_cycle_stage.is_normalized() {
            base_name_node.put_boolean_prop(
                cx.get_type_registry_and_ast_mut().1,
                Prop::IS_CONSTANT_NAME,
                true,
            );
        }
        let property_names = &parts[1..];
        self.create_get_props_with_unknown_type(cx, base_name_node, property_names)
    }

    // port: AstFactory#createQNameWithUnknownType(Iterable)
    fn create_qname_with_unknown_type_from_names<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        names: &[&str],
    ) -> NodeId {
        let base_name = *check_not_null!(names.first());
        let property_names = &names[1..];
        self.create_qname_with_unknown_type_with_base_name(cx, base_name, property_names)
    }

    // port: AstFactory#createQNameWithUnknownType(String, Iterable)
    pub fn create_qname_with_unknown_type_with_base_name<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        base_name: &str,
        property_names: &[&str],
    ) -> NodeId {
        let base_name_node = self.create_name_with_unknown_type(cx, base_name);
        self.create_get_props_with_unknown_type(cx, base_name_node, property_names)
    }

    // port: AstFactory#createJscomp
    fn create_jscomp<C: AstFactoryContext + ?Sized>(&self, cx: &mut C) -> NodeId {
        let jscomp = IR::name(cx.get_type_registry_and_ast_mut().1, "$jscomp");
        self.set_js_type_or_color(
            cx,
            Self::type_jstype_and_color(self.unknown_type, Some(standard_colors::UNKNOWN.clone())),
            jscomp,
        );
        if self.life_cycle_stage.is_normalized() {
            jscomp.put_boolean_prop(
                cx.get_type_registry_and_ast_mut().1,
                Prop::IS_CONSTANT_NAME,
                true,
            );
        }
        jscomp
    }

    /// Creates an access of the `prototype` property of `receiver`.
    // port: AstFactory#createPrototypeAccess
    pub fn create_prototype_access<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
    ) -> NodeId {
        let result = IR::getprop(cx.get_type_registry_and_ast_mut().1, receiver, "prototype");
        match self.type_mode {
            TypeMode::JSTYPE => {
                let type_ =
                    self.get_js_type_for_property(cx, receiver, &JsString::from("prototype"));
                result.set_jstype(cx.get_type_registry_and_ast_mut().1, Some(type_));
            }
            TypeMode::COLOR => {
                let ast = cx.get_type_registry_and_ast_mut().1;
                let receiver_color = check_not_null!(
                    receiver.get_color(ast),
                    "Missing color on %s",
                    receiver.to_string(ast)
                );
                let possible_prototypes = receiver_color.get_prototypes();
                result.set_color(
                    ast,
                    Some(if possible_prototypes.is_empty() {
                        standard_colors::UNKNOWN.clone()
                    } else {
                        Color::create_union(possible_prototypes)
                    }),
                );
            }
            TypeMode::NONE => {}
        }
        result
    }

    /// Creates an access of the given `qname` on `$jscomp.global`
    ///
    /// For example, given "Object.defineProperties", returns an AST representation of
    /// "$jscomp.global.Object.defineProperties".
    ///
    /// This may be useful if adding synthetic code to a local scope, which can shadow the global
    /// like Object you're trying to access. The `$jscomp.global` field must have been injected.
    // port: AstFactory#createJSCompDotGlobalAccess
    pub fn create_jscomp_dot_global_access<C, S>(
        &self,
        cx: &mut C,
        scope: &S,
        qname: &str,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let global = self
            .runtime_js_lib_manager
            .as_ref()
            .expect("NullPointerException: runtimeJsLibManager")
            .lock()
            .unwrap()
            .get_js_lib_field("$jscomp.global");
        let jscomp_dot_global = self.create_qname_for_field(cx, scope, global.as_ref());
        let result = self.create_qname(cx, scope, qname);
        // Move the fully qualified qname onto the $jscomp.global getprop
        let (qname_root, qname_root_string) = {
            let ast = cx.get_type_registry_and_ast_mut().1;
            let qname_root = NodeUtil::get_root_of_qualified_name(ast, result);
            (qname_root, qname_root.get_string(ast))
        };
        let getprop = self.create_get_prop(
            cx,
            jscomp_dot_global,
            qname_root_string,
            Self::type_node(qname_root),
        );
        qname_root.replace_with(cx.get_type_registry_and_ast_mut().1, getprop);
        result
    }

    /// Creates a GETPROP node, setting its type from the receiver's JSType property.
    ///
    /// Deprecated: use `createGetProp(Node, String, Type)` instead; this method does not support
    /// colors.
    // port: AstFactory#createGetPropWithoutColor
    pub fn create_get_prop_without_color<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_name: impl Into<JsString>,
    ) -> NodeId {
        self.assert_not_adding_colors();
        let property_name = property_name.into();
        let result = IR::getprop(
            cx.get_type_registry_and_ast_mut().1,
            receiver,
            property_name.clone(),
        );
        if self.is_adding_types() {
            let type_ = self.get_js_type_for_property(cx, receiver, &property_name);
            result.set_jstype(cx.get_type_registry_and_ast_mut().1, Some(type_));
        }
        result
    }

    /// Creates a GETPROP node with the given type.
    // port: AstFactory#createGetProp
    pub fn create_get_prop<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_name: impl Into<JsString>,
        type_: Type,
    ) -> NodeId {
        let result = IR::getprop(
            cx.get_type_registry_and_ast_mut().1,
            receiver,
            property_name,
        );
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Creates a tree of nodes representing `receiver.name1.name2.etc`.
    ///
    /// Deprecated: does not support colors.
    // port: AstFactory#createGetPropsWithoutColors
    pub fn create_get_props_without_colors<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_names: &[&str],
    ) -> NodeId {
        self.assert_not_adding_colors();
        let mut result = receiver;
        for property_name in property_names {
            result = self.create_get_prop_without_color(cx, result, *property_name);
        }
        result
    }

    // port: AstFactory#createGetPropsWithUnknownType
    pub fn create_get_props_with_unknown_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_names: &[&str],
    ) -> NodeId {
        let mut result = receiver;
        for property_name in property_names {
            result = self.create_get_prop_with_unknown_type(cx, result, *property_name);
        }
        result
    }

    // port: AstFactory#createGetPropWithUnknownType
    pub fn create_get_prop_with_unknown_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_name: impl Into<JsString>,
    ) -> NodeId {
        let result = IR::getprop(
            cx.get_type_registry_and_ast_mut().1,
            receiver,
            property_name,
        );
        self.set_js_type_or_color(
            cx,
            Self::type_jstype_and_color(self.unknown_type, Some(standard_colors::UNKNOWN.clone())),
            result,
        );
        result
    }

    // port: AstFactory#createStartOptChainGetprop
    pub fn create_start_opt_chain_getprop<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_name: impl Into<JsString>,
        type_: Type,
    ) -> NodeId {
        let result = IR::start_opt_chain_getprop(
            cx.get_type_registry_and_ast_mut().1,
            receiver,
            property_name,
        );
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createContinueOptChainGetprop
    pub fn create_continue_opt_chain_getprop<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_name: impl Into<JsString>,
        type_: Type,
    ) -> NodeId {
        let result = IR::continue_opt_chain_getprop(
            cx.get_type_registry_and_ast_mut().1,
            receiver,
            property_name,
        );
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createStartOptChainGetelem
    pub fn create_start_opt_chain_getelem<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        elem: NodeId,
        type_: Type,
    ) -> NodeId {
        let result =
            IR::start_opt_chain_getelem(cx.get_type_registry_and_ast_mut().1, receiver, elem);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createContinueOptChainGetelem
    pub fn create_continue_opt_chain_getelem<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        elem: NodeId,
        type_: Type,
    ) -> NodeId {
        let result =
            IR::continue_opt_chain_getelem(cx.get_type_registry_and_ast_mut().1, receiver, elem);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createStartOptChainCall
    pub fn create_start_opt_chain_call<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        type_: Type,
        args: &[NodeId],
    ) -> NodeId {
        let result = IR::start_opt_chain_call(cx.get_type_registry_and_ast_mut().1, receiver, args);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createContinueOptChainCall
    pub fn create_continue_opt_chain_call<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        type_: Type,
        args: &[NodeId],
    ) -> NodeId {
        let result =
            IR::continue_opt_chain_call(cx.get_type_registry_and_ast_mut().1, receiver, args);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Creates a GETELEM node.
    // port: AstFactory#createGetElem
    pub fn create_get_elem<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        key: NodeId,
    ) -> NodeId {
        let result = IR::getelem(cx.get_type_registry_and_ast_mut().1, receiver, key);
        // TODO(bradfordcsmith): When receiver is an Array<T> or an Object<K, V>, use the template
        // type here.
        self.set_js_type_or_color(
            cx,
            Self::type_jstype_and_color(self.unknown_type, Some(standard_colors::UNKNOWN.clone())),
            result,
        );
        result
    }

    // port: AstFactory#createDelProp
    pub fn create_del_prop<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        target: NodeId,
    ) -> NodeId {
        let result = IR::delprop(cx.get_type_registry_and_ast_mut().1, target);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createStringKey
    pub fn create_string_key<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        key: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        let result = IR::string_key_with_value(ast, key, value);
        let type_ = Self::type_jstype_and_color(value.get_jstype(ast), value.get_color(ast));
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createComputedProperty
    pub fn create_computed_property<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        key: NodeId,
        value: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        let result = IR::computed_prop(ast, key, value);
        let type_ = Self::type_jstype_and_color(value.get_jstype(ast), value.get_color(ast));
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Creates a GETTER_DEF whose function returns `value`.
    // port: AstFactory#createGetterDef
    pub fn create_getter_def<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        let return_type = value.get_jstype(cx.get_type_registry_and_ast_mut().1);
        // Name is stored on the GETTER_DEF node. The function has no name.
        let return_node = self.create_return(cx, value);
        let body = IR::block_with_child(cx.get_type_registry_and_ast_mut().1, return_node);
        let function_node =
            self.create_zero_arg_function(cx, /* name= */ "", body, return_type);
        let ast = cx.get_type_registry_and_ast_mut().1;
        let getter_node = ast.new_string_with_token(Token::GETTER_DEF, name);
        getter_node.add_child_to_front(ast, function_node);
        getter_node
    }

    // port: AstFactory#createIn
    pub fn create_in<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::r#in(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createComma
    pub fn create_comma<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        let result = IR::comma(ast, left, right);
        let type_ = Self::type_jstype_and_color(right.get_jstype(ast), right.get_color(ast));
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createCommas
    pub fn create_commas<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        first: NodeId,
        second: NodeId,
        rest: &[NodeId],
    ) -> NodeId {
        let mut result = self.create_comma(cx, first, second);
        for next in rest {
            result = self.create_comma(cx, result, *next);
        }
        result
    }

    /// Creates a logical and expression.
    // port: AstFactory#createAnd
    pub fn create_and<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::and(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_union_of_operands(cx, result, left, right);
        result
    }

    /// Creates a logical or expression.
    // port: AstFactory#createOr
    pub fn create_or<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::or(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_union_of_operands(cx, result, left, right);
        result
    }

    /// Rust-only: the type switch `createAnd` and `createOr` share (Java repeats it inline).
    // port: AstFactory#createAnd (type switch)
    // port: AstFactory#createOr (type switch)
    fn set_union_of_operands<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        result: NodeId,
        left: NodeId,
        right: NodeId,
    ) {
        match self.type_mode {
            TypeMode::JSTYPE => {
                let (registry, ast) = cx.get_type_registry_and_ast_mut();
                let registry = registry.expect("NullPointerException: registry");
                let left_type = check_not_null!(left.get_jstype(ast), "%s", left.to_string(ast));
                let right_type = check_not_null!(right.get_jstype(ast), "%s", right.to_string(ast));
                let union = registry.create_union_type(ast, &[left_type, right_type]);
                result.set_jstype(ast, Some(union));
            }
            TypeMode::COLOR => {
                let ast = cx.get_type_registry_and_ast_mut().1;
                let left_color = check_not_null!(left.get_color(ast), "%s", left.to_string(ast));
                let right_color = check_not_null!(right.get_color(ast), "%s", right.to_string(ast));
                result.set_color(
                    ast,
                    Some(Color::create_union(&IndexSet::<_>::from_iter([
                        left_color,
                        right_color,
                    ]))),
                );
            }
            TypeMode::NONE => {}
        }
    }

    // port: AstFactory#createAdd
    pub fn create_add<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::add(ast, left, right);
        // Note: this result type could be made tighter if it proves useful for optimizations later
        // on like setting the string type if both operands are strings.
        match self.type_mode {
            TypeMode::JSTYPE => {
                let type_ =
                    Self::get_native_type(registry.as_deref(), JSTypeNative::BIGINT_NUMBER_STRING);
                result.set_jstype(ast, Some(type_));
            }
            TypeMode::COLOR => {
                result.set_color(ast, Some(BIGINT_NUMBER_STRING_COLOR.clone()));
            }
            TypeMode::NONE => {}
        }
        result
    }

    // port: AstFactory#createSub
    pub fn create_sub<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::sub(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(JSTypeNative::NUMBER_TYPE, standard_colors::NUMBER.clone()),
            result,
        );
        result
    }

    // port: AstFactory#createInc
    pub fn create_inc<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        operand: NodeId,
        is_post: bool,
    ) -> NodeId {
        let result = IR::inc(cx.get_type_registry_and_ast_mut().1, operand, is_post);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(JSTypeNative::NUMBER_TYPE, standard_colors::NUMBER.clone()),
            result,
        );
        result
    }

    // port: AstFactory#createLessThan
    pub fn create_less_than<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::lt(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createBitwiseAnd
    pub fn create_bitwise_and<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::bitwise_and(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(JSTypeNative::NUMBER_TYPE, standard_colors::NUMBER.clone()),
            result,
        );
        result
    }

    // port: AstFactory#createRightShift
    pub fn create_right_shift<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = IR::right_shift(cx.get_type_registry_and_ast_mut().1, left, right);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(JSTypeNative::NUMBER_TYPE, standard_colors::NUMBER.clone()),
            result,
        );
        result
    }

    // port: AstFactory#createCall
    pub fn create_call<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        callee: NodeId,
        result_type: Type,
        args: &[NodeId],
    ) -> NodeId {
        let result = NodeUtil::new_call_node(cx.get_type_registry_and_ast_mut().1, callee, args);
        self.set_js_type_or_color(cx, result_type, result);
        result
    }

    // port: AstFactory#createCallWithUnknownType
    pub fn create_call_with_unknown_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        callee: NodeId,
        args: &[NodeId],
    ) -> NodeId {
        self.create_call(
            cx,
            callee,
            Self::type_jstype_and_color(self.unknown_type, Some(standard_colors::UNKNOWN.clone())),
            args,
        )
    }

    /// Creates a call to Object.assign that returns the specified type.
    ///
    /// Object.assign returns !Object in the externs, which can lose type information if the actual
    /// type is known.
    // port: AstFactory#createObjectDotAssignCall
    pub fn create_object_dot_assign_call<C, S>(
        &self,
        cx: &mut C,
        scope: &S,
        return_type: Type,
        args: &[NodeId],
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let obj_assign = self.create_qname_with_base_name(cx, scope, "Object", &["assign"]);
        let result = self.create_call(cx, obj_assign, return_type.clone(), args);

        match self.type_mode {
            TypeMode::JSTYPE => {
                let (registry, ast) = cx.get_type_registry_and_ast_mut();
                let registry = registry.expect("NullPointerException: registry");
                // Make a unique function type that returns the exact type we've inferred it to be.
                // Object.assign in the externs just returns !Object, which loses type information.
                let return_js_type = return_type.get_jstype(ast, registry);
                let object_type = registry.get_native_type(JSTypeNative::OBJECT_TYPE);
                let object_or_null = registry.create_union_type_from_native(
                    ast,
                    &[JSTypeNative::OBJECT_TYPE, JSTypeNative::NULL_TYPE],
                );
                let obj_assign_type = registry.create_function_type_with_var_args(
                    ast,
                    return_js_type,
                    &[object_type, object_or_null],
                );
                obj_assign.set_jstype(ast, Some(obj_assign_type));
            }
            TypeMode::COLOR | TypeMode::NONE => {}
        }

        result
    }

    // port: AstFactory#createNewNode
    pub fn create_new_node<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        target: NodeId,
        args: &[NodeId],
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::new_node(ast, target, args);
        match self.type_mode {
            TypeMode::JSTYPE => {
                let registry = registry.expect("NullPointerException: registry");
                let mut instance_type = target
                    .get_jstype(ast)
                    .expect("NullPointerException: getJSType()");
                if instance_type.is_function_type(registry) {
                    instance_type = instance_type
                        .to_maybe_function_type(registry)
                        .unwrap()
                        .get_instance_type(registry)
                        .expect("NullPointerException: getInstanceType()");
                } else {
                    instance_type =
                        Self::get_native_type(Some(registry), JSTypeNative::UNKNOWN_TYPE);
                }
                result.set_jstype(ast, Some(instance_type));
            }
            TypeMode::COLOR => {
                let color = self.get_instance_of_color(target.get_color(ast));
                result.set_color(ast, Some(color));
            }
            TypeMode::NONE => {}
        }
        result
    }

    /// Create a call that returns an instance of the given class type.
    ///
    /// This method is intended for use in special cases, such as calling `super()` in a
    /// constructor.
    // port: AstFactory#createConstructorCall
    pub fn create_constructor_call<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        class_type: Type,
        callee: NodeId,
        args: &[NodeId],
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = NodeUtil::new_call_node(ast, callee, args);
        match self.type_mode {
            TypeMode::JSTYPE => {
                let registry = registry.expect("NullPointerException: registry");
                let class_js_type = class_type.get_jstype(ast, registry);
                let constructor_type =
                    check_not_null!(class_js_type.to_maybe_function_type(registry));
                let instance_type = check_not_null!(constructor_type.get_instance_type(registry));
                result.set_jstype(ast, Some(instance_type));
            }
            TypeMode::COLOR => {
                let color = class_type.get_color(ast, self.color_registry.as_deref());
                result.set_color(ast, Some(self.get_instance_of_color(Some(color))));
            }
            TypeMode::NONE => {}
        }
        result
    }

    // port: AstFactory#createAssignStatement
    pub fn create_assign_statement<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        lhs: NodeId,
        rhs: NodeId,
    ) -> NodeId {
        let assign = self.create_assign(cx, lhs, rhs);
        self.expr_result(cx, assign)
    }

    /// Creates an assignment expression `lhs = rhs`
    // port: AstFactory#createAssign(Node, Node)
    pub fn create_assign<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        lhs: NodeId,
        rhs: NodeId,
    ) -> NodeId {
        let result = IR::assign(cx.get_type_registry_and_ast_mut().1, lhs, rhs);
        self.set_js_type_or_color(cx, Self::type_node(rhs), result);
        result
    }

    /// Creates an assignment expression `lhs = rhs`
    // port: AstFactory#createAssign(String, Node)
    pub fn create_assign_to_name<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        lhs_name: impl Into<JsString>,
        rhs: NodeId,
    ) -> NodeId {
        let name = self.create_name(cx, lhs_name, Self::type_node(rhs));
        self.create_assign(cx, name, rhs)
    }

    /// Creates an object-literal with zero or more elements, `{}`.
    ///
    /// The type of the literal, if assigned, may be a supertype of the known properties.
    // port: AstFactory#createObjectLit(Node...)
    pub fn create_object_lit<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        elements: &[NodeId],
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::objectlit(ast, elements);
        match self.type_mode {
            TypeMode::JSTYPE => {
                let type_ = registry
                    .expect("NullPointerException: registry")
                    .create_anonymous_object_type(ast, None);
                result.set_jstype(ast, Some(type_));
            }
            TypeMode::COLOR => {
                result.set_color(ast, Some(standard_colors::TOP_OBJECT.clone()));
            }
            TypeMode::NONE => {}
        }
        result
    }

    /// Creates an object-literal with zero or more elements and a specific type.
    ///
    /// The type of the literal, if assigned, may be a supertype of the known properties.
    // port: AstFactory#createObjectLit(Type, Node...)
    pub fn create_object_lit_with_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        type_: Type,
        elements: &[NodeId],
    ) -> NodeId {
        let result = IR::objectlit(cx.get_type_registry_and_ast_mut().1, elements);
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Creates a quoted string key node; type information is not added.
    // port: AstFactory#createQuotedStringKey
    pub fn create_quoted_string_key<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        key: impl Into<JsString>,
        value: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        let result = IR::string_key_with_value(ast, key, value);
        result.set_quoted_string_key(ast);
        result
    }

    /// Creates an empty function `function() {}`
    // port: AstFactory#createEmptyFunction
    pub fn create_empty_function<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        type_: Type,
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = NodeUtil::empty_function(ast);
        if self.is_adding_types() {
            let registry = registry.expect("NullPointerException: registry");
            check_argument!(
                type_.get_jstype(ast, registry).is_function_type(registry),
                "%s",
                format!("{type_:?}")
            );
        }
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    /// Creates an empty generator function `function*() {}`
    // port: AstFactory#createEmptyGeneratorFunction
    pub fn create_empty_generator_function<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        type_: Type,
    ) -> NodeId {
        let result = self.create_empty_function(cx, type_);
        result.set_is_generator_function(cx.get_type_registry_and_ast_mut().1, true);
        result
    }

    /// Creates a function `function name(paramList) { body }`
    ///
    /// - `name`: STRING node - empty string if no name
    /// - `paramList`: PARAM_LIST node
    /// - `body`: BLOCK node
    /// - `type`: type to apply to the function itself
    // port: AstFactory#createFunction
    pub fn create_function<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        param_list: NodeId,
        body: NodeId,
        type_: Type,
    ) -> NodeId {
        let name_node = self.create_name(cx, name, type_.clone());
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::function(ast, name_node, param_list, body);
        if self.is_adding_types() {
            let registry = registry.expect("NullPointerException: registry");
            check_argument!(
                type_.get_jstype(ast, registry).is_function_type(registry),
                "%s",
                format!("{type_:?}")
            );
        }
        self.set_js_type_or_color(cx, type_, result);
        result
    }

    // port: AstFactory#createParamList
    pub fn create_param_list<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        parameter_names: &[&str],
    ) -> NodeId {
        let param_list = IR::param_list(cx.get_type_registry_and_ast_mut().1, &[]);
        for parameter_name in parameter_names {
            let name = self.create_name_with_unknown_type(cx, *parameter_name);
            param_list.add_child_to_back(cx.get_type_registry_and_ast_mut().1, name);
        }
        param_list
    }

    // port: AstFactory#createZeroArgFunction
    pub fn create_zero_arg_function<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        body: NodeId,
        return_type: Option<TypeId>,
    ) -> NodeId {
        let function_type = if self.is_adding_types() {
            let (registry, ast) = cx.get_type_registry_and_ast_mut();
            let registry = registry.expect("NullPointerException: registry");
            // registry.createFunctionType(returnType)
            let parameters = registry.create_parameters(&[]);
            let function_type =
                registry.create_function_type_with_parameters(ast, return_type, parameters);
            function_type.to_maybe_function_type(registry)
        } else {
            None
        };
        let param_list = IR::param_list(cx.get_type_registry_and_ast_mut().1, &[]);
        self.create_function(
            cx,
            name,
            param_list,
            body,
            Self::type_jstype_and_color(function_type, Some(standard_colors::TOP_OBJECT.clone())),
        )
    }

    // port: AstFactory#createZeroArgGeneratorFunction
    pub fn create_zero_arg_generator_function<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        body: NodeId,
        return_type: Option<TypeId>,
    ) -> NodeId {
        let result = self.create_zero_arg_function(cx, name, body, return_type);
        result.set_is_generator_function(cx.get_type_registry_and_ast_mut().1, true);
        result
    }

    // port: AstFactory#createZeroArgArrowFunctionForExpression
    pub fn create_zero_arg_arrow_function_for_expression<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        expression: NodeId,
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let name = IR::name(ast, "");
        let param_list = IR::param_list(ast, &[]);
        let result = IR::arrow_function(ast, name, param_list, expression);
        match self.type_mode {
            TypeMode::JSTYPE => {
                let registry = registry.expect("NullPointerException: registry");
                // It feels like we should be adding type-of-this here, but it should remain
                // unknown, because you're allowed to supply any kind of value of `this` when
                // calling an arrow function. It will just be ignored in favor of the `this` in the
                // scope where the arrow was defined.
                let function_type = closure_jstype::function_type::builder()
                    .with_return_type(expression.get_jstype_required(ast))
                    .with_parameters(Vec::new())
                    .build_and_resolve(registry, ast);
                result.set_jstype(ast, Some(function_type));
            }
            TypeMode::COLOR => {
                result.set_color(ast, Some(standard_colors::TOP_OBJECT.clone()));
            }
            TypeMode::NONE => {}
        }
        result
    }

    // port: AstFactory#createMemberFunctionDef
    pub fn create_member_function_def<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        name: impl Into<JsString>,
        function: NodeId,
    ) -> NodeId {
        let ast = cx.get_type_registry_and_ast_mut().1;
        // A function used for a member function definition must have an empty name,
        // because the name string goes on the MEMBER_FUNCTION_DEF node.
        check_argument!(
            function
                .get_first_child(ast)
                .unwrap()
                .get_string(ast)
                .is_empty(),
            "%s",
            function.to_string(ast)
        );
        let result = IR::member_function_def(ast, name, function);
        self.set_js_type_or_color(cx, Self::type_node(function), result);
        result
    }

    // port: AstFactory#createSheq
    pub fn create_sheq<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        expr1: NodeId,
        expr2: NodeId,
    ) -> NodeId {
        let result = IR::sheq(cx.get_type_registry_and_ast_mut().1, expr1, expr2);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createEq
    pub fn create_eq<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        expr1: NodeId,
        expr2: NodeId,
    ) -> NodeId {
        let result = IR::eq(cx.get_type_registry_and_ast_mut().1, expr1, expr2);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createNe
    pub fn create_ne<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        expr1: NodeId,
        expr2: NodeId,
    ) -> NodeId {
        let result = IR::ne(cx.get_type_registry_and_ast_mut().1, expr1, expr2);
        self.set_js_type_or_color(
            cx,
            Self::type_native_and_color(
                JSTypeNative::BOOLEAN_TYPE,
                standard_colors::BOOLEAN.clone(),
            ),
            result,
        );
        result
    }

    // port: AstFactory#createHook
    pub fn create_hook<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        condition: NodeId,
        expr1: NodeId,
        expr2: NodeId,
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::hook(ast, condition, expr1, expr2);
        match self.type_mode {
            TypeMode::JSTYPE => {
                let registry = registry.expect("NullPointerException: registry");
                let variants = [
                    expr1.get_jstype(ast).expect("NullPointerException"),
                    expr2.get_jstype(ast).expect("NullPointerException"),
                ];
                let union = registry.create_union_type(ast, &variants);
                result.set_jstype(ast, Some(union));
            }
            TypeMode::COLOR => {
                // ImmutableSet.of rejects null elements
                let colors = IndexSet::<_>::from_iter([
                    expr1.get_color(ast).expect("NullPointerException"),
                    expr2.get_color(ast).expect("NullPointerException"),
                ]);
                result.set_color(ast, Some(Color::create_union(&colors)));
            }
            TypeMode::NONE => {}
        }
        result
    }

    // port: AstFactory#createOptionalChainShortCircuit
    pub fn create_optional_chain_short_circuit<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        result_expr: NodeId,
    ) -> NodeId {
        let null = self.create_null(cx);
        let cond = self.create_eq(cx, receiver, null);
        let undefined = self.create_undefined_value(cx);
        self.create_hook(cx, cond, undefined, result_expr)
    }

    // port: AstFactory#createArraylit(Node...)
    // port: AstFactory#createArraylit(Iterable)
    pub fn create_arraylit<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        elements: &[NodeId],
    ) -> NodeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let result = IR::arraylit(ast, elements);
        match self.type_mode {
            TypeMode::JSTYPE => {
                let registry = registry.expect("NullPointerException: registry");
                let array_type = registry.get_native_object_type(JSTypeNative::ARRAY_TYPE);
                // TODO(nickreid): Use a reasonable template type. Remeber to consider SPREAD.
                let unknown = Self::get_native_type(Some(registry), JSTypeNative::UNKNOWN_TYPE);
                let type_ = registry.create_templatized_type(ast, array_type, &[unknown]);
                result.set_jstype(ast, Some(type_));
            }
            TypeMode::COLOR => {
                let color = self
                    .color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::ARRAY_ID);
                result.set_color(ast, Some(color));
            }
            TypeMode::NONE => {}
        }
        result
    }

    /// Rust-only: `runtimeJsLibManager.getJsLibField(name)`.
    fn get_js_lib_field(&self, field_name: &str) -> Arc<dyn JsLibField> {
        self.runtime_js_lib_manager
            .as_ref()
            .expect("NullPointerException: runtimeJsLibManager")
            .lock()
            .unwrap()
            .get_js_lib_field(field_name)
    }

    /// Rust-only: the JSTYPE branch shared by the `$jscomp` iterator helpers: fills the template
    /// type of `callee`'s function type with `template_type` and returns its return type.
    // port: AstFactory#createJSCompMakeIteratorCall (JSTYPE branch)
    fn replace_callee_template_and_get_return_type<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        callee: NodeId,
        template_type: TypeId,
    ) -> Type {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let registry = registry.expect("NullPointerException: registry");
        let callee_type = callee
            .get_jstype(ast)
            .expect("NullPointerException: getJSType()");
        let replaced = Self::replace_template(registry, ast, callee_type, &[template_type]);
        callee.set_jstype(ast, Some(replaced));
        let return_type = replaced
            .assert_function_type(registry, ast)
            .get_return_type(registry);
        Self::type_jstype(Some(return_type))
    }

    /// Given an iterable like `rhs` in
    ///
    /// ```js
    /// for await (lhs of rhs) { block(); }
    /// ```
    ///
    /// returns a call node for the `rhs` wrapped in a `$jscomp.makeIterator` call.
    // port: AstFactory#createJSCompMakeIteratorCall
    pub fn create_jscomp_make_iterator_call<C, S>(
        &self,
        cx: &mut C,
        iterable: NodeId,
        scope: &S,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let make_iterator = self.get_js_lib_field("$jscomp.makeIterator");
        let make_iterator_name = self.create_qname_for_field(cx, scope, make_iterator.as_ref());
        let type_ = match self.type_mode {
            TypeMode::JSTYPE => {
                // Since createCall (currently) doesn't handle templated functions, fill in the
                // template types
                // of makeIteratorName manually.
                // e.g get `number` from `Iterable<number>`
                let iterable_type = {
                    let (registry, ast) = cx.get_type_registry_and_ast_mut();
                    let registry = registry.expect("NullPointerException: registry");
                    let key = registry.get_iterable_value_template();
                    iterable
                        .get_jstype(ast)
                        .expect("NullPointerException: getJSType()")
                        .get_template_type_map(registry)
                        .get_resolved_template_type(registry, ast, key)
                };
                // e.g. replace
                //   function(Iterable<T>): Iterator<T>
                // with
                //   function(Iterable<number>): Iterator<number>
                self.replace_callee_template_and_get_return_type(
                    cx,
                    make_iterator_name,
                    iterable_type,
                )
            }
            TypeMode::COLOR => Self::type_(
                self.color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::ITERATOR_ID),
            ),
            TypeMode::NONE => Self::no_type_information(),
        };
        let call = self.create_call(cx, make_iterator_name, type_, &[iterable]);
        call.put_boolean_prop(cx.get_type_registry_and_ast_mut().1, Prop::FREE_CALL, true);
        call
    }

    /// Given an iterator like `rhs` in
    ///
    /// ```js
    /// [...rhs]
    /// ```
    ///
    /// returns a call node for the `rhs` wrapped in a `$jscomp.arrayFromIterator(rhs)` call.
    // port: AstFactory#createJscompArrayFromIteratorCall
    pub fn create_jscomp_array_from_iterator_call<C, S>(
        &self,
        cx: &mut C,
        iterator: NodeId,
        scope: &S,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let array_from_iterator = self.get_js_lib_field("$jscomp.arrayFromIterator");
        let make_iterator_name =
            self.create_qname_for_field(cx, scope, array_from_iterator.as_ref());
        let result_type = match self.type_mode {
            TypeMode::JSTYPE => {
                // Since createCall (currently) doesn't handle templated functions, fill in the
                // template types of makeIteratorName manually.
                let iterable_type = {
                    let (registry, ast) = cx.get_type_registry_and_ast_mut();
                    let registry = registry.expect("NullPointerException: registry");
                    let key = registry.get_iterator_value_template();
                    iterator
                        .get_jstype(ast)
                        .expect("NullPointerException: getJSType()")
                        .get_template_type_map(registry)
                        .get_resolved_template_type(registry, ast, key)
                };
                // e.g. replace
                //   function(Iterator<T>): Array<T>
                // with
                //   function(Iterator<number>): Array<number>
                self.replace_callee_template_and_get_return_type(
                    cx,
                    make_iterator_name,
                    iterable_type,
                )
            }
            // colors don't include generics, so just set the return type to Array.
            TypeMode::COLOR => Self::type_(
                self.color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::ARRAY_ID),
            ),
            TypeMode::NONE => Self::no_type_information(),
        };
        let call = self.create_call(cx, make_iterator_name, result_type, &[iterator]);
        call.put_boolean_prop(cx.get_type_registry_and_ast_mut().1, Prop::FREE_CALL, true);
        call
    }

    /// Given an iterable like `rhs` in
    ///
    /// ```js
    /// [...rhs]
    /// ```
    ///
    /// returns a call node for the `rhs` wrapped in a `$jscomp.arrayFromIterable(rhs)` call.
    // port: AstFactory#createJscompArrayFromIterableCall
    pub fn create_jscomp_array_from_iterable_call<C, S>(
        &self,
        cx: &mut C,
        iterable: NodeId,
        scope: &S,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let array_from_iterable = self.get_js_lib_field("$jscomp.arrayFromIterable");
        let make_iterable_name =
            self.create_qname_for_field(cx, scope, array_from_iterable.as_ref());
        let result_type = match self.type_mode {
            TypeMode::JSTYPE => {
                // Since createCall (currently) doesn't handle templated functions, fill in the
                // template types of makeIteratorName manually.
                let iterable_type = {
                    let (registry, ast) = cx.get_type_registry_and_ast_mut();
                    let registry = registry.expect("NullPointerException: registry");
                    let key = registry.get_iterable_value_template();
                    iterable
                        .get_jstype(ast)
                        .expect("NullPointerException: getJSType()")
                        .get_template_type_map(registry)
                        .get_resolved_template_type(registry, ast, key)
                };
                // e.g. replace
                //   function(Iterable<T>): Array<T>
                // with
                //   function(Iterable<number>): Array<number>
                self.replace_callee_template_and_get_return_type(
                    cx,
                    make_iterable_name,
                    iterable_type,
                )
            }
            // colors don't include generics, so just set the return type to Array.
            TypeMode::COLOR => Self::type_(
                self.color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::ARRAY_ID),
            ),
            TypeMode::NONE => Self::no_type_information(),
        };
        let call = self.create_call(cx, make_iterable_name, result_type, &[iterable]);
        call.put_boolean_prop(cx.get_type_registry_and_ast_mut().1, Prop::FREE_CALL, true);
        call
    }

    /// Given an iterable like `rhs` in
    ///
    /// ```js
    /// for await (lhs of rhs) { block(); }
    /// ```
    ///
    /// returns a call node for the `rhs` wrapped in a `$jscomp.makeAsyncIterator` call.
    // port: AstFactory#createJSCompMakeAsyncIteratorCall
    pub fn create_jscomp_make_async_iterator_call<C, S>(
        &self,
        cx: &mut C,
        iterable: NodeId,
        scope: &S,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let make_async_iterator = self.get_js_lib_field("$jscomp.makeAsyncIterator");
        let make_iterator_async_name =
            self.create_qname_for_field(cx, scope, make_async_iterator.as_ref());
        let result_type = match self.type_mode {
            TypeMode::JSTYPE => {
                // Since createCall (currently) doesn't handle templated functions, fill in the
                // template types of makeIteratorName manually.
                // e.g get `number` from `AsyncIterable<number>`
                let async_iterable_type = {
                    let (registry, ast) = cx.get_type_registry_and_ast_mut();
                    let registry = registry.expect("NullPointerException: registry");
                    let iterable_type = iterable
                        .get_jstype(ast)
                        .expect("NullPointerException: getJSType()");
                    crate::js_iterables::JsIterables::maybe_box_iterable_or_async_iterable(
                        iterable_type,
                        registry,
                        ast,
                    )
                    .or_else(
                        self.unknown_type
                            .expect("NullPointerException: unknownType"),
                    )
                };
                // e.g. replace
                //   function(AsyncIterable<T>): AsyncIterator<T>
                // with
                //   function(AsyncIterable<number>): AsyncIterator<number>
                self.replace_callee_template_and_get_return_type(
                    cx,
                    make_iterator_async_name,
                    async_iterable_type,
                )
            }
            TypeMode::COLOR => Self::type_(
                self.color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::ASYNC_ITERATOR_ITERABLE_ID),
            ),
            TypeMode::NONE => Self::no_type_information(),
        };
        let call = self.create_call(cx, make_iterator_async_name, result_type, &[iterable]);
        call.put_boolean_prop(cx.get_type_registry_and_ast_mut().1, Prop::FREE_CALL, true);
        call
    }

    // port: AstFactory#replaceTemplate
    fn replace_template(
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        templated_type: TypeId,
        template_types: &[TypeId],
    ) -> TypeId {
        let template_keys = templated_type
            .get_template_type_map(registry)
            .get_template_keys()
            .to_vec();
        let type_map = registry.get_empty_template_type_map().copy_with_extension(
            registry,
            ast,
            &template_keys,
            template_types,
        );
        let mut replacer = TemplateTypeReplacer::for_partial_replacement(type_map);
        templated_type.visit(registry, ast, &mut replacer)
    }

    /// Creates an empty generator function with the correct return type to be an argument to
    /// `$jscomp.AsyncGeneratorWrapper`.
    ///
    /// - `asyncGeneratorWrapperType`: the instantiated type of
    ///   `$jscomp.AsyncGeneratorWrapper`, e.g. `$jscomp.AsyncGeneratorWrapper<number>`. Only used
    ///   when adding JSTypes.
    // port: AstFactory#createEmptyAsyncGeneratorWrapperArgument
    pub fn create_empty_async_generator_wrapper_argument<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        async_generator_wrapper_type: Option<TypeId>,
    ) -> NodeId {
        let mut generator_type = Self::no_type_information();

        if self.is_adding_types() {
            let (registry, ast) = cx.get_type_registry_and_ast_mut();
            let registry = registry.expect("NullPointerException: registry");
            let async_generator_wrapper_type =
                async_generator_wrapper_type.expect("NullPointerException");
            if async_generator_wrapper_type.is_unknown_type(registry, ast) {
                // Not injecting libraries?
                let generator = Self::get_native_type(Some(registry), JSTypeNative::GENERATOR_TYPE);
                let unknown_type = self
                    .unknown_type
                    .expect("NullPointerException: unknownType");
                let return_type = Self::replace_template(registry, ast, generator, &[unknown_type]);
                generator_type =
                    Self::type_jstype(Some(registry.create_function_type(ast, return_type, &[])));
            } else {
                // Generator<$jscomp.AsyncGeneratorWrapper$ActionRecord<number>>
                let parameters = async_generator_wrapper_type
                    .to_maybe_function_type(registry)
                    .expect("NullPointerException: toMaybeFunctionType()")
                    .get_parameters(registry);
                // Iterables.getOnlyElement
                check_argument!(
                    parameters.len() == 1,
                    "expected one element but was: %s",
                    parameters.len()
                );
                let inner_function_return_type = parameters[0].get_jstype();
                generator_type = Self::type_jstype(Some(registry.create_function_type(
                    ast,
                    inner_function_return_type,
                    &[],
                )));
            }
        } else if self.is_adding_colors() {
            // colors don't model function types, so it's fine to fallback to the top object.
            generator_type = Self::type_(standard_colors::TOP_OBJECT.clone());
        }

        self.create_empty_generator_function(cx, generator_type)
    }

    /// Creates a call to `$jscomp.asyncExecutePromiseGeneratorFunction` with the given generator
    /// function.
    // port: AstFactory#createJscompAsyncExecutePromiseGeneratorFunctionCall
    pub fn create_jscomp_async_execute_promise_generator_function_call<C, S>(
        &self,
        cx: &mut C,
        scope: &S,
        generator_function: NodeId,
    ) -> NodeId
    where
        C: AstFactoryContext + ?Sized,
        S: AstFactoryStaticScope<C> + ?Sized,
    {
        let method = self.get_js_lib_field("$jscomp.asyncExecutePromiseGeneratorFunction");
        let jscomp_dot_async_execute_promise_generator_function =
            self.create_qname_for_field(cx, scope, method.as_ref());
        let result_type = match self.type_mode {
            TypeMode::JSTYPE => {
                // TODO(bradfordcsmith): Maybe update the type to be more specific
                // Currently this method expects `function(): !Generator<?>` and returns
                // `Promise<?>`. Since we propagate type information only if type checking has
                // already run, these unknowns probably don't matter, but we should be able to be
                // more specific with the return type at least.
                let (registry, ast) = cx.get_type_registry_and_ast_mut();
                let registry = registry.expect("NullPointerException: registry");
                let return_type = jscomp_dot_async_execute_promise_generator_function
                    .get_jstype(ast)
                    .expect("NullPointerException: getJSType()")
                    .assert_function_type(registry, ast)
                    .get_return_type(registry);
                Self::type_jstype(Some(return_type))
            }
            TypeMode::COLOR => Self::type_(
                self.color_registry
                    .as_ref()
                    .expect("NullPointerException: colorRegistry")
                    .get(standard_colors::PROMISE_ID),
            ),
            TypeMode::NONE => Self::type_jstype(self.unknown_type),
        };
        let call = self.create_call(
            cx,
            jscomp_dot_async_execute_promise_generator_function,
            result_type,
            &[generator_function],
        );
        call.put_boolean_prop(cx.get_type_registry_and_ast_mut().1, Prop::FREE_CALL, true);
        call
    }

    // port: AstFactory#getInstanceOfColor
    fn get_instance_of_color(&self, color: Option<Color>) -> Color {
        let color = color.expect("NullPointerException: getColor()");
        let instance_colors = color.get_instance_colors();
        if instance_colors.is_empty() {
            standard_colors::UNKNOWN.clone()
        } else {
            Color::create_union(instance_colors)
        }
    }

    // port: AstFactory#getNativeType
    fn get_native_type(registry: Option<&JSTypeRegistry>, native_type: JSTypeNative) -> TypeId {
        let registry = check_not_null!(registry, "registry is null");
        registry.get_native_type(native_type)
    }

    // port: AstFactory#getJsTypeForProperty
    fn get_js_type_for_property<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        receiver: NodeId,
        property_name: &JsString,
    ) -> TypeId {
        let (registry, ast) = cx.get_type_registry_and_ast_mut();
        let registry = registry.expect("registry is null");
        let ast = &*ast;
        // NOTE: we use both findPropertyType and getPropertyType because they are subtly
        // different: findPropertyType works on JSType, autoboxing scalars and joining unions,
        // but it returns null if the type is not found and does not handle dynamic types of
        // Function.prototype.call and .apply; whereas getPropertyType does not autobox nor
        // iterate over unions, but it does synthesize the function properties correctly, and
        // it returns unknown instead of null if the property is missing.
        let mut getprop_type = None;
        let receiver_js_type = receiver.get_jstype(ast);
        if let Some(receiver_js_type) = receiver_js_type {
            getprop_type =
                receiver_js_type.find_property_type(registry, ast, property_name.clone());
            if getprop_type.is_none() {
                let autoboxed = receiver_js_type.autobox(registry, ast);
                let receiver_object_type = object_type::cast(registry, Some(autoboxed));
                getprop_type = match receiver_object_type {
                    None => self.unknown_type,
                    Some(receiver_object_type) => Some(ObjectType::get_property_type(
                        receiver_object_type,
                        registry,
                        ast,
                        property_name.clone(),
                    )),
                };
            }
        }
        let mut getprop_type = getprop_type
            .or(self.unknown_type)
            .expect("NullPointerException: unknownType");
        // TODO(bradfordcsmith): Special case $jscomp.global until we annotate its type correctly.
        if getprop_type.is_unknown_type(registry, ast)
            && *property_name == "global"
            && receiver.matches_name(ast, "$jscomp")
        {
            getprop_type = Self::get_native_type(Some(registry), JSTypeNative::GLOBAL_THIS);
        }
        getprop_type
    }

    // port: AstFactory#setJSTypeOrColor
    fn set_js_type_or_color<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        type_: Type,
        result: NodeId,
    ) {
        match self.type_mode {
            TypeMode::JSTYPE => {
                let (registry, ast) = cx.get_type_registry_and_ast_mut();
                let jstype = type_.get_jstype(ast, registry.expect("registry is null"));
                result.set_jstype(ast, Some(jstype));
            }
            TypeMode::COLOR => {
                let ast = cx.get_type_registry_and_ast_mut().1;
                let color = type_.get_color(ast, self.color_registry.as_deref());
                result.set_color(ast, Some(color));
            }
            TypeMode::NONE => {}
        }
    }

    /// Uses the JSType or Color of the given node as a template for adding type information
    // port: AstFactory#type(Node)
    pub fn type_node(node: NodeId) -> Type {
        Type::TypeOnNode(node)
    }

    // port: AstFactory#type(JSType)
    pub fn type_jstype(type_: Option<TypeId>) -> Type {
        Type::JSTypeOrColor {
            jstype: type_,
            jstype_native: None,
            color: None,
            color_id: None,
        }
    }

    // port: AstFactory#type(JSTypeNative)
    pub fn type_native(type_: JSTypeNative) -> Type {
        Type::JSTypeOrColor {
            jstype: None,
            jstype_native: Some(type_),
            color: None,
            color_id: None,
        }
    }

    // port: AstFactory#type(Color)
    pub fn type_(type_: Color) -> Type {
        Type::JSTypeOrColor {
            jstype: None,
            jstype_native: None,
            color: Some(type_),
            color_id: None,
        }
    }

    // port: AstFactory#type(ColorId)
    pub fn type_color_id(type_: ColorId) -> Type {
        Type::JSTypeOrColor {
            jstype: None,
            jstype_native: None,
            color: None,
            color_id: Some(type_),
        }
    }

    // port: AstFactory#type(JSType, Color)
    pub fn type_jstype_and_color(type_: Option<TypeId>, color: Option<Color>) -> Type {
        Type::JSTypeOrColor {
            jstype: type_,
            jstype_native: None,
            color,
            color_id: None,
        }
    }

    // port: AstFactory#type(JSTypeNative, Color)
    pub fn type_native_and_color(type_: JSTypeNative, color: Color) -> Type {
        Type::JSTypeOrColor {
            jstype: None,
            jstype_native: Some(type_),
            color: Some(color),
            color_id: None,
        }
    }

    // port: AstFactory#noTypeInformation
    fn no_type_information() -> Type {
        Type::JSTypeOrColor {
            jstype: None,
            jstype_native: None,
            color: None,
            color_id: None,
        }
    }
}

/// Represents a type to be set on a new node: a JSType, a Color, or both, depending on the
/// factory's mode.
// port: AstFactory.Type
#[derive(Debug, Clone)]
pub enum Type {
    // port: AstFactory.TypeOnNode
    TypeOnNode(NodeId),
    // port: AstFactory.JSTypeOrColor
    JSTypeOrColor {
        jstype: Option<TypeId>,
        jstype_native: Option<JSTypeNative>,
        color: Option<Color>,
        color_id: Option<ColorId>,
    },
}

impl Type {
    // port: AstFactory.TypeOnNode#getJSType
    // port: AstFactory.JSTypeOrColor#getJSType
    fn get_jstype(&self, ast: &Ast, registry: &JSTypeRegistry) -> TypeId {
        match self {
            Type::TypeOnNode(n) => {
                let jstype = n.get_jstype(ast);
                // TODO(b/149843534): crash instead of defaulting to unknown
                jstype.unwrap_or_else(|| registry.get_native_type(JSTypeNative::UNKNOWN_TYPE))
            }
            Type::JSTypeOrColor {
                jstype,
                jstype_native,
                ..
            } => jstype.unwrap_or_else(|| {
                registry.get_native_type(jstype_native.expect("NullPointerException: jstypeNative"))
            }),
        }
    }

    // port: AstFactory.TypeOnNode#getColor
    // port: AstFactory.JSTypeOrColor#getColor
    fn get_color(&self, ast: &Ast, registry: Option<&ColorRegistry>) -> Color {
        match self {
            Type::TypeOnNode(n) => {
                let color = n.get_color(ast);
                // TODO(b/149843534): crash instead of defaulting to unknown
                color.unwrap_or_else(|| standard_colors::UNKNOWN.clone())
            }
            Type::JSTypeOrColor {
                color, color_id, ..
            } => match color {
                Some(color) => color.clone(),
                None => registry
                    .expect("NullPointerException: colorRegistry")
                    .get(check_not_null!(*color_id)),
            },
        }
    }
}
