/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AstAnalyzer.java.

//! Port of `AstAnalyzer.java`: logic for answering questions about portions of the AST.
//!
//! **What kind of methods should go here?**
//!
//! Methods that answer questions about some portion of the AST and that may require global
//! information about the compilation, generally taking at least one `Node` as an argument. For
//! example: does a node have side effects? Can we statically determine the value of a node?
//!
//! **What kind of logic should not go here?**
//!
//! Really simple logic that requires no global information, like finding the parameter list node
//! of a function, should be in `NodeUtil`. Logic that creates new Nodes or modifies the AST
//! should go in `AstFactory`.
//!
//! Rust note: Java's `AstAnalyzer` keeps the `JSTypeRegistry` in a field. In Rust the compiler owns
//! the registry next to its `Ast`, and `JSType#equals` needs `&mut JSTypeRegistry` together with
//! the `Ast`, so the queries take an [`AstAnalyzerContext`] (the arena plus the registry the Java
//! analyzer was constructed with) instead of a bare `&Ast`.

use crate::{AbstractCompiler, node_util::NodeUtil};
use crate::{accessor_summary::AccessorSummary, accessor_summary::PropertyAccessKind};
use closure_jstype::{JSTypeRegistry, js_type::JSType, js_type_native::JSTypeNative};
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    jscomp_colors::standard_colors,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::Arc;

/// Rust-only: the arena the analyzer reads, and the type registry Java's `AstAnalyzer` holds in its
/// `typeRegistry` field (`None` where Java passes `null`).
pub trait AstAnalyzerContext {
    fn get_ast(&self) -> &Ast;
    fn get_type_registry_and_ast(&mut self) -> (Option<&mut JSTypeRegistry>, &Ast);
}

impl AstAnalyzerContext for Ast {
    fn get_ast(&self) -> &Ast {
        self
    }
    fn get_type_registry_and_ast(&mut self) -> (Option<&mut JSTypeRegistry>, &Ast) {
        (None, self)
    }
}

impl AstAnalyzerContext for AbstractCompiler {
    fn get_ast(&self) -> &Ast {
        self
    }
    fn get_type_registry_and_ast(&mut self) -> (Option<&mut JSTypeRegistry>, &Ast) {
        self.get_type_registry_field_and_ast()
    }
}

/// Rust-only: an `Ast` paired with the registry a test or caller hands to `new AstAnalyzer(..)`.
pub struct TypedAstAnalyzerContext<'a> {
    pub ast: &'a Ast,
    pub type_registry: Option<&'a mut JSTypeRegistry>,
}

impl AstAnalyzerContext for TypedAstAnalyzerContext<'_> {
    fn get_ast(&self) -> &Ast {
        self.ast
    }
    fn get_type_registry_and_ast(&mut self) -> (Option<&mut JSTypeRegistry>, &Ast) {
        (self.type_registry.as_deref_mut(), self.ast)
    }
}

/// The set of builtin constructors that don't have side effects.
///
/// TODO(bradfordcsmith): If all of these are annotated `sideefectfree`, can we drop this list?
const CONSTRUCTORS_WITHOUT_SIDE_EFFECTS: [&str; 6] = [
    "Array",
    "Date",
    "Error",
    "Object",
    "RegExp",
    "XMLHttpRequest",
];

// A list of built-in object creation or primitive type cast functions that
// can also be called as constructors but lack side-effects.
// TODO(johnlenz): consider adding an extern annotation for this.
const BUILTIN_FUNCTIONS_WITHOUT_SIDEEFFECTS: [&str; 9] = [
    "Object", "Array", "String", "Number", "BigInt", "Boolean", "RegExp", "Error", "Symbol",
];
const REGEXP_METHODS: [&str; 2] = ["test", "exec"];
const STRING_REGEXP_METHODS: [&str; 4] = ["match", "replace", "search", "split"];

fn contains(set: &[&str], name: &JsString) -> bool {
    set.iter().any(|s| name == *s)
}

/// Logic for answering questions about portions of the AST.
#[derive(Clone)]
pub struct AstAnalyzer {
    accessor_summary: Option<Arc<AccessorSummary>>,
    use_types_for_local_optimization: bool,
    assume_getters_are_pure: bool,
    assume_known_builtins_are_pure: bool,
    has_regexp_global_references: bool,
}

// port: AstAnalyzer.Options
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub use_types_for_local_optimization: bool,
    pub assume_getters_are_pure: bool,
    pub has_regexp_global_references: bool,
    pub assume_known_builtins_are_pure: bool,
}

impl Options {
    // port: AstAnalyzer.Options#builder
    pub fn builder() -> Builder {
        Builder(Options::default())
    }
}

// port: AstAnalyzer.Options.Builder
pub struct Builder(Options);

impl Builder {
    // port: AstAnalyzer.Options.Builder#setUseTypesForLocalOptimization
    pub fn set_use_types_for_local_optimization(mut self, value: bool) -> Self {
        self.0.use_types_for_local_optimization = value;
        self
    }
    // port: AstAnalyzer.Options.Builder#setAssumeGettersArePure
    pub fn set_assume_getters_are_pure(mut self, value: bool) -> Self {
        self.0.assume_getters_are_pure = value;
        self
    }
    // port: AstAnalyzer.Options.Builder#setHasRegexpGlobalReferences
    pub fn set_has_regexp_global_references(mut self, value: bool) -> Self {
        self.0.has_regexp_global_references = value;
        self
    }
    // port: AstAnalyzer.Options.Builder#setAssumeKnownBuiltinsArePure
    pub fn set_assume_known_builtins_are_pure(mut self, value: bool) -> Self {
        self.0.assume_known_builtins_are_pure = value;
        self
    }
    // port: AstAnalyzer.Options.Builder#build
    pub fn build(self) -> Options {
        self.0
    }
}

impl AstAnalyzer {
    // port: AstAnalyzer#AstAnalyzer
    pub fn new(options: Options, accessor_summary: Option<Arc<AccessorSummary>>) -> Self {
        check_argument!(
            options.assume_getters_are_pure || accessor_summary.is_some(),
            "accessorSummary must be provided if assumeGettersArePure is false"
        );
        Self {
            accessor_summary,
            use_types_for_local_optimization: options.use_types_for_local_optimization,
            assume_getters_are_pure: options.assume_getters_are_pure,
            has_regexp_global_references: options.has_regexp_global_references,
            assume_known_builtins_are_pure: options.assume_known_builtins_are_pure,
        }
    }

    /// Returns true if the node may create new mutable state, or change existing state.
    ///
    /// See <http://www.xkcd.com/326/>.
    // port: AstAnalyzer#mayEffectMutableState
    pub fn may_effect_mutable_state<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        n: NodeId,
    ) -> bool {
        self.check_for_state_change_helper(cx, n, /* check_for_new_objects= */ true)
    }

    /// Returns true if the node which may have side effects when executed. This version default to
    /// the "safe" assumptions when the compiler object is not provided (RegExp have side-effects,
    /// etc).
    // port: AstAnalyzer#mayHaveSideEffects
    pub fn may_have_side_effects<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        n: NodeId,
    ) -> bool {
        self.check_for_state_change_helper(cx, n, /* check_for_new_objects= */ false)
    }

    /// Returns true if this function call may have side effects.
    ///
    /// This method is guaranteed to return true all calls that have side-effects, but may also
    /// return true for calls that have none.
    // port: AstAnalyzer#functionCallHasSideEffects
    pub fn function_call_has_side_effects<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        call_node: NodeId,
    ) -> bool {
        let ast = cx.get_ast();
        check_state!(
            call_node.is_call(ast)
                || call_node.is_tagged_template_lit(ast)
                || call_node.is_opt_chain_call(ast),
            "%s",
            call_node.to_string(ast)
        );

        if call_node.is_no_side_effects_call(ast) {
            return false;
        }

        if call_node.is_only_modifies_arguments_call(ast)
            && NodeUtil::all_args_unescaped_local(ast, call_node)
        {
            return false;
        }

        let callee = call_node.get_first_child(ast).unwrap();

        // Built-in functions with no side effects.
        if callee.is_name(ast) {
            let name = callee.get_string(ast);
            if self.assume_known_builtins_are_pure
                && contains(&BUILTIN_FUNCTIONS_WITHOUT_SIDEEFFECTS, &name)
            {
                return false;
            }
        } else if callee.is_get_prop(ast) || callee.is_opt_chain_get_prop(ast) {
            let method = callee.get_string(ast);
            if self.assume_known_builtins_are_pure
                && (if method == "valueOf" {
                    call_node.has_one_child(ast)
                } else {
                    method == "toString"
                        && (call_node.has_one_child(ast) || call_node.has_two_children(ast))
                })
            {
                return false;
            }

            if call_node.is_only_modifies_this_call(ast)
                && NodeUtil::evaluates_to_local_value(ast, callee.get_first_child(ast).unwrap())
            {
                return false;
            }

            // Many common Math functions have no side-effects.
            // TODO(nicksantos): This is a terrible terrible hack, until
            // I create a definitionProvider that understands namespacing.
            if self.assume_known_builtins_are_pure
                && callee.get_first_child(ast).unwrap().is_name(ast)
                && callee.is_qualified_name(ast)
                && callee.get_first_child(ast).unwrap().get_string_ref(ast) == "Math"
            {
                match method.to_string_lossy().as_str() {
                    "abs" | "acos" | "acosh" | "asin" | "asinh" | "atan" | "atanh" | "atan2"
                    | "cbrt" | "ceil" | "cos" | "cosh" | "exp" | "expm1" | "floor" | "hypot"
                    | "log" | "log10" | "log1p" | "log2" | "max" | "min" | "pow" | "round"
                    | "sign" | "sin" | "sinh" | "sqrt" | "tan" | "tanh" | "trunc" => {
                        return false;
                    }
                    "random" => {
                        // no parameters
                        return !call_node.has_one_child(ast);
                    }
                    _ => {
                        // Unknown Math.* function, so fall out of this switch statement.
                    }
                }
            }

            if !self.has_regexp_global_references && self.assume_known_builtins_are_pure {
                let receiver = callee.get_first_child(ast).unwrap();
                if receiver.is_reg_exp(ast) && contains(&REGEXP_METHODS, &method) {
                    return false;
                } else if self.is_typed_as_string(cx, receiver) {
                    let ast = cx.get_ast();
                    // Unlike regexs, string methods don't need to be hosted on a string literal
                    // to avoid leaking mutating global state changes, it is just necessary that
                    // the regex object can't be referenced.
                    let param = callee.get_next(ast);
                    if let Some(param) = param {
                        if param.is_string_lit(ast) {
                            if contains(&STRING_REGEXP_METHODS, &method) {
                                return false;
                            }
                        } else if param.is_reg_exp(ast) {
                            if method == "replace" {
                                // Assume anything but a string constant has side-effects
                                return !param.get_next(ast).unwrap().is_string_lit(ast);
                            } else if contains(&STRING_REGEXP_METHODS, &method) {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        true
    }

    // port: AstAnalyzer#isTypedAsString
    fn is_typed_as_string<C: AstAnalyzerContext + ?Sized>(&self, cx: &mut C, n: NodeId) -> bool {
        let ast = cx.get_ast();
        if n.is_string_lit(ast) {
            return true;
        }

        if self.use_types_for_local_optimization {
            let color = n.get_color(ast);
            if let Some(color) = color {
                return color == *standard_colors::STRING;
            }
            let type_ = n.get_jstype(ast);
            if let Some(type_) = type_ {
                let (type_registry, ast) = cx.get_type_registry_and_ast();
                // Java dereferences the (nullable) typeRegistry field here.
                let type_registry = type_registry.expect("NullPointerException: typeRegistry");
                let native_string_type = type_registry.get_native_type(JSTypeNative::STRING_TYPE);
                if type_.equals(type_registry, ast, native_string_type) {
                    return true;
                }
            }
        }

        false
    }

    /// Returns true if some node in n's subtree changes application state. If
    /// `check_for_new_objects` is true, we assume that newly created mutable objects (like object
    /// literals) change state. Otherwise, we assume that they have no side effects.
    // port: AstAnalyzer#checkForStateChangeHelper
    fn check_for_state_change_helper<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        n: NodeId,
        check_for_new_objects: bool,
    ) -> bool {
        let ast = cx.get_ast();
        let parent = n.get_parent(ast);
        // Rather than id which ops may have side effects, id the ones
        // that we know to be safe
        match n.get_token(ast) {
            // Throw is a side-effect by definition.
            Token::THROW
            // Context switches can conceal side-effects.
            | Token::YIELD
            | Token::AWAIT
            | Token::FOR_AWAIT_OF
            // Enhanced for loops are almost always side-effectful; it's not worth checking them
            // further. Particularly, they represent a kind of assignment op.
            | Token::FOR_OF
            | Token::FOR_IN
            // Variable declarations are side-effects.
            | Token::VAR
            | Token::LET
            | Token::CONST
            | Token::EXPORT
            // import() expressions have side effects
            | Token::DYNAMIC_IMPORT => true,

            // The super keyword is a noop on its own.
            Token::SUPER => false,

            Token::OBJECTLIT | Token::ARRAYLIT | Token::REGEXP => {
                check_for_new_objects
                    || self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }
            Token::OBJECT_REST | Token::OBJECT_SPREAD => {
                // Object-rest and object-spread may trigger a getter.
                !self.assume_getters_are_pure
                    || self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }
            Token::ITER_REST | Token::ITER_SPREAD => {
                NodeUtil::iterates_impure_iterable(ast, n)
                    || self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }

            Token::NAME => {
                // NAMEs have children if the left side of a var/let/const
                // TODO(b/129564961): Consider EXPORT declarations.
                n.has_children(ast)
            }
            Token::FUNCTION => {
                // Function expressions don't have side-effects, but function
                // declarations change the namespace. Either way, we don't need to
                // check the children, since they aren't executed at declaration time.
                check_for_new_objects || NodeUtil::is_function_declaration(ast, n)
            }

            Token::GETTER_DEF | Token::SETTER_DEF | Token::MEMBER_FUNCTION_DEF => {
                // simply defining a member function, getter, or setter has no side effects
                false
            }

            Token::COMPUTED_PROP => {
                let parent = parent.unwrap();
                match parent.get_token(ast) {
                    Token::CLASS_MEMBERS => {
                        // This is a computed method.
                        let first = n.get_first_child(ast).unwrap();
                        self.check_for_state_change_helper(cx, first, check_for_new_objects)
                    }

                    Token::OBJECT_PATTERN => {
                        // Due to language syntax, only the last child can be an OBJECT_REST.
                        // `({ ['thisKey']: target, ...rest} = something())`
                        // The presence of `thisKey` affects what properties get put into `rest`.
                        parent.get_last_child(ast).unwrap().is_object_rest(ast)
                            || self.check_for_change_state_in_children(
                                cx,
                                n,
                                check_for_new_objects,
                            )
                    }

                    Token::OBJECTLIT => {
                        // Assume that COMPUTED_PROP keys in OBJECTLIT never trigger getters.
                        self.check_for_change_state_in_children(cx, n, check_for_new_objects)
                    }
                    _ => panic!("Illegal COMPUTED_PROP parent {}", parent.to_string(ast)),
                }
            }
            Token::MEMBER_FIELD_DEF => {
                n.is_static_member(ast) && n.has_children(ast) && {
                    let first = n.get_first_child(ast).unwrap();
                    self.check_for_state_change_helper(cx, first, check_for_new_objects)
                }
            }
            Token::COMPUTED_FIELD_DEF => {
                let first = n.get_first_child(ast).unwrap();
                let second = n.get_second_child(ast);
                let is_static_member = n.is_static_member(ast);
                self.check_for_state_change_helper(cx, first, check_for_new_objects)
                    || (is_static_member
                        && second.is_some()
                        && self.check_for_state_change_helper(
                            cx,
                            second.unwrap(),
                            check_for_new_objects,
                        ))
            }
            Token::CLASS => {
                let second = n.get_second_child(ast).unwrap();
                let last = n.get_last_child(ast).unwrap();
                check_for_new_objects
                    || NodeUtil::is_class_declaration(ast, n)
                    // Check the extends clause for side effects.
                    || self.check_for_state_change_helper(cx, second, check_for_new_objects)
                    // Check for class members that are computed properties with side effects.
                    || self.check_for_state_change_helper(cx, last, check_for_new_objects)
            }

            Token::NEW => {
                check_for_new_objects
                    || self.constructor_call_has_side_effects(cx.get_ast(), n)
                    || self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }
            Token::CALL | Token::OPTCHAIN_CALL | Token::TAGGED_TEMPLATELIT => {
                // calls to functions that have no side effects have the no
                // side effect property set.
                self.function_call_has_side_effects(cx, n)
                    || self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }

            Token::CAST
            | Token::AND
            | Token::BLOCK
            | Token::ROOT
            | Token::EXPR_RESULT
            | Token::HOOK
            | Token::IF
            | Token::CLASS_MEMBERS
            | Token::PARAM_LIST
            // Any context that supports DEFAULT_VALUE is already an assignment. The possiblity of
            // a default doesn't itself create a side-effect. Therefore, we prefer to defer the
            // decision.
            | Token::DEFAULT_VALUE
            | Token::NUMBER
            | Token::BIGINT
            | Token::OR
            | Token::COALESCE
            | Token::THIS
            | Token::TRUE
            | Token::FALSE
            | Token::NULL
            | Token::STRINGLIT
            | Token::SWITCH
            | Token::TEMPLATELIT_SUB
            | Token::TRY
            | Token::EMPTY
            | Token::TEMPLATELIT
            | Token::TEMPLATELIT_STRING => {
                self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }

            Token::STRING_KEY => {
                let parent = parent.unwrap();
                if parent.is_object_pattern(ast) {
                    // This STRING_KEY names a property being read from.
                    // Assumption: GETELEM (via a COMPUTED_PROP) never triggers a getter or setter.
                    if self.get_property_kind(&n.get_string(ast)).has_getter() {
                        return true;
                    } else if parent.get_last_child(ast).unwrap().is_object_rest(ast) {
                        // Due to language syntax, only the last child can be an OBJECT_REST.
                        // `({ thisKey: target, ...rest} = something())`
                        // The presence of `thisKey` affects what properties get put into `rest`.
                        return true;
                    }
                }
                self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }

            // Since we can't see what property is accessed we cannot tell whether
            // obj[someProp]/obj?.[someProp] will
            // trigger a getter or setter, and thus could have side effects.
            // We will assume it does not. This introduces some risk of code breakage, but the code
            // size cost of assuming all GETELEM/OPTCHAIN_GETELEM nodes have side effects is
            // completely unacceptable.
            Token::GETELEM | Token::OPTCHAIN_GETELEM => {
                self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }

            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                // TODO(b/135640150): Use the parent nodes to determine whether this is a get or set.
                self.get_property_kind(&n.get_string(ast)).has_getter_or_setter()
                    || self.check_for_change_state_in_children(cx, n, check_for_new_objects)
            }
            _ => {
                if NodeUtil::is_simple_operator(ast, n) {
                    return self.check_for_change_state_in_children(cx, n, check_for_new_objects);
                }
                if NodeUtil::is_assignment_op(ast, n) {
                    return self.check_assignment_for_change_state(cx, n, check_for_new_objects);
                }
                true
            }
        }
    }

    // port: AstAnalyzer#checkForChangeStateInChildren
    fn check_for_change_state_in_children<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        n: NodeId,
        check_for_new_objects: bool,
    ) -> bool {
        let mut c = n.get_first_child(cx.get_ast());
        while let Some(cur) = c {
            if self.check_for_state_change_helper(cx, cur, check_for_new_objects) {
                return true;
            }
            c = cur.get_next(cx.get_ast());
        }

        false
    }

    // port: AstAnalyzer#checkAssignmentForChangeState
    fn check_assignment_for_change_state<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        assign_op: NodeId,
        check_for_new_objects: bool,
    ) -> bool {
        let assign_target = assign_op.get_first_child(cx.get_ast()).unwrap();

        if assign_target.is_name(cx.get_ast()) {
            return true;
        }

        let assign_value = assign_op.get_last_child(cx.get_ast()).unwrap();
        // Assignments will have side effects if
        // a) The RHS has side effects, or
        // b) The LHS has side effects, or
        // c) A name on the LHS will exist beyond the life of this statement.
        if self.check_for_state_change_helper(cx, assign_target, check_for_new_objects)
            || self.check_for_state_change_helper(cx, assign_value, check_for_new_objects)
        {
            return true;
        }

        let ast = cx.get_ast();
        if !NodeUtil::is_normal_get(ast, assign_target) {
            // TODO(johnlenz): remove this code and make this an exception. This
            // is here only for legacy reasons, the AST is not valid but
            // preserve existing behavior.
            return !NodeUtil::is_literal_value(ast, assign_target, true);
        }
        // If the object being assigned to is a local object, don't
        // consider this a side-effect as it can't be referenced
        // elsewhere.  Don't do this recursively as the property might
        // be an alias of another object, unlike a literal below.
        let mut current = assign_target.get_first_child(ast).unwrap();
        if NodeUtil::evaluates_to_local_value(ast, current) {
            return false;
        }

        // A literal value as defined by "isLiteralValue" is guaranteed
        // not to be an alias, or any components which are aliases of
        // other objects.
        // If the root object is a literal don't consider this a
        // side-effect.
        while NodeUtil::is_normal_get(ast, current) {
            current = current.get_first_child(ast).unwrap();
        }

        !NodeUtil::is_literal_value(ast, current, true)
    }

    /// Do calls to this constructor have side effects?
    // port: AstAnalyzer#constructorCallHasSideEffects
    pub fn constructor_call_has_side_effects(&self, ast: &Ast, new_node: NodeId) -> bool {
        check_argument!(
            new_node.is_new(ast),
            "Expected NEW node, got %s",
            new_node.get_token(ast)
        );

        if new_node.is_no_side_effects_call(ast) {
            return false;
        }

        // allArgsUnescapedLocal() is actually confirming that all of the arguments are literals or
        // values created at the point they are passed in to the call and are not saved anywhere in
        // the calling scope.
        // TODO(bradfordcsmith): It would be good to rename allArgsUnescapedLocal() to something
        // that makes this clearer.
        if new_node.is_only_modifies_arguments_call(ast)
            && NodeUtil::all_args_unescaped_local(ast, new_node)
        {
            return false;
        }

        let name_node = new_node.get_first_child(ast).unwrap();
        !name_node.is_name(ast)
            || !self.assume_known_builtins_are_pure
            || !contains(
                &CONSTRUCTORS_WITHOUT_SIDE_EFFECTS,
                &name_node.get_string(ast),
            )
    }

    /// Returns true if the current node's type implies side effects.
    ///
    /// This is a non-recursive version of the may have side effects check; used to check wherever
    /// the current node's type is one of the reasons why a subtree has side effects.
    // port: AstAnalyzer#nodeTypeMayHaveSideEffects
    pub fn node_type_may_have_side_effects<C: AstAnalyzerContext + ?Sized>(
        &self,
        cx: &mut C,
        n: NodeId,
    ) -> bool {
        let ast = cx.get_ast();
        if NodeUtil::is_assignment_op(ast, n) {
            return true;
        }

        match n.get_token(ast) {
            Token::DELPROP
            | Token::DEC
            | Token::INC
            | Token::YIELD
            | Token::THROW
            | Token::AWAIT
            | Token::DYNAMIC_IMPORT => true,
            Token::FOR_IN // assigns to a loop LHS
            | Token::FOR_OF // assigns to a loop LHS, runs an iterator
            | Token::FOR_AWAIT_OF // assigns to a loop LHS, runs an iterator, async operations
            => true,
            Token::OPTCHAIN_CALL | Token::CALL | Token::TAGGED_TEMPLATELIT => {
                self.function_call_has_side_effects(cx, n)
            }
            Token::NEW => self.constructor_call_has_side_effects(ast, n),

            // A variable definition that assigns a value.
            // TODO(b/129564961): Consider EXPORT declarations.
            Token::NAME => n.has_children(ast),
            // A destructuring declaration statement or assignment. Technically these might contain
            // no lvalues but that case is rare enough to be ignored.
            Token::DESTRUCTURING_LHS => true,

            // Object-rest and object-spread may trigger a getter.
            Token::OBJECT_REST | Token::OBJECT_SPREAD => !self.assume_getters_are_pure,
            Token::ITER_REST | Token::ITER_SPREAD => NodeUtil::iterates_impure_iterable(ast, n),
            Token::STRING_KEY => {
                if n.get_parent(ast).unwrap().is_object_pattern(ast) {
                    return self.get_property_kind(&n.get_string(ast)).has_getter();
                }
                false
            }
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                self.get_property_kind(&n.get_string(ast)).has_getter_or_setter()
            }
            _ => false,
        }
    }

    // port: AstAnalyzer#getPropertyKind
    fn get_property_kind(&self, name: &JsString) -> PropertyAccessKind {
        if self.assume_getters_are_pure {
            PropertyAccessKind::NORMAL
        } else {
            self.accessor_summary
                .as_ref()
                .expect("NullPointerException: accessorSummary")
                .get_kind(name)
        }
    }
}
