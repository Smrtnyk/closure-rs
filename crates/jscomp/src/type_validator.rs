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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/TypeValidator.java.

//! A central reporter for all type violations (TypeValidator.java).
use crate::{
    abstract_compiler::AbstractCompiler,
    check_level::CheckLevel,
    compiler_input::CompilerInput,
    diagnostic_group::DiagnosticGroup,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    js_iterables::JsIterables,
    node_traversal::NodeTraversal,
    platform::Platform,
    rhino_error_reporter::BOUNDED_GENERIC_TYPE_ERROR,
    type_mismatch::{Accumulator, TypeMismatch},
    typed_var::TypedVar,
};
use closure_jstype::{
    JSTypeNative::{
        self, ARRAY_TYPE, ASYNC_GENERATOR_TYPE, BIGINT_NUMBER, BIGINT_NUMBER_OBJECT,
        BIGINT_NUMBER_STRING, BIGINT_NUMBER_STRING_OBJECT, BOOLEAN_TYPE, GENERATOR_TYPE,
        I_TEMPLATE_ARRAY_TYPE, ITERABLE_TYPE, NO_OBJECT_TYPE, NULL_TYPE, NUMBER_STRING_SYMBOL,
        NUMBER_SYMBOL, NUMBER_TYPE, OBJECT_TYPE, STRING_SYMBOL, STRING_TYPE, UNKNOWN_TYPE,
        VOID_TYPE,
    },
    JSTypeRegistry, TypeId,
    enum_element_type::EnumElementType,
    enum_type::EnumType,
    function_type::FunctionType,
    js_type::{JSType, Nullability, SubtypingMode},
    named_type::NamedType,
    object_type::ObjectType,
    property::{Property, PropertyKey},
    template_type::TemplateType,
    template_type_replacer::TemplateTypeReplacer,
    templatized_type::TemplatizedType,
    union_type::UnionType,
    visitor::WithDefaultCase,
};
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::{
    collections::BTreeSet,
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

// port: TypeValidator#INVALID_CAST
pub static INVALID_CAST: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_CAST",
    "invalid cast - must be a subtype or supertype\nfrom: {0}\nto  : {1}",
);

// port: TypeValidator#TYPE_MISMATCH_WARNING
pub static TYPE_MISMATCH_WARNING: DiagnosticType =
    DiagnosticType::warning("JSC_TYPE_MISMATCH", "{0}");

// port: TypeValidator#INVALID_ASYNC_RETURN_TYPE
pub static INVALID_ASYNC_RETURN_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_ASYNC_RETURN_TYPE",
    "The return type of an async function must be a supertype of Promise\nfound: {0}",
);

// port: TypeValidator#INVALID_OPERAND_TYPE
pub static INVALID_OPERAND_TYPE: DiagnosticType =
    DiagnosticType::disabled("JSC_INVALID_OPERAND_TYPE", "{0}");

// port: TypeValidator#MISSING_EXTENDS_TAG_WARNING
pub static MISSING_EXTENDS_TAG_WARNING: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_EXTENDS_TAG",
    "Missing @extends tag on type {0}",
);

// port: TypeValidator#DUP_VAR_DECLARATION
pub static DUP_VAR_DECLARATION: DiagnosticType = DiagnosticType::warning(
    "JSC_DUP_VAR_DECLARATION",
    "variable {0} redefined, original definition at {1}:{2}",
);

// port: TypeValidator#DUP_VAR_DECLARATION_TYPE_MISMATCH
pub static DUP_VAR_DECLARATION_TYPE_MISMATCH: DiagnosticType = DiagnosticType::warning(
    "JSC_DUP_VAR_DECLARATION_TYPE_MISMATCH",
    "variable {0} redefined with type {1}, original definition at {2}:{3} with type {4}",
);

// port: TypeValidator#INTERFACE_METHOD_NOT_IMPLEMENTED
pub static INTERFACE_METHOD_NOT_IMPLEMENTED: DiagnosticType = DiagnosticType::warning(
    "JSC_INTERFACE_METHOD_NOT_IMPLEMENTED",
    "property {0} on interface {1} is not implemented by type {2}",
);

// port: TypeValidator#HIDDEN_INTERFACE_PROPERTY_MISMATCH
pub static HIDDEN_INTERFACE_PROPERTY_MISMATCH: DiagnosticType = DiagnosticType::warning(
    "JSC_HIDDEN_INTERFACE_PROPERTY_MISMATCH",
    "mismatch of the {0} property on type {4} and the type of the property it overrides from interface {1}\noriginal: {2}\noverride: {3}",
);

// port: TypeValidator#HIDDEN_SUPERCLASS_PROPERTY_MISMATCH
pub static HIDDEN_SUPERCLASS_PROPERTY_MISMATCH: DiagnosticType = DiagnosticType::warning(
    "JSC_HIDDEN_SUPERCLASS_PROPERTY_MISMATCH",
    "mismatch of the {0} property type and the type of the property it overrides from superclass {1}\noriginal: {2}\noverride: {3}",
);

// port: TypeValidator#ABSTRACT_METHOD_NOT_IMPLEMENTED
pub static ABSTRACT_METHOD_NOT_IMPLEMENTED: DiagnosticType = DiagnosticType::warning(
    "JSC_ABSTRACT_METHOD_NOT_IMPLEMENTED",
    "property {0} on abstract class {1} is not implemented by type {2}",
);

// port: TypeValidator#UNKNOWN_TYPEOF_VALUE
pub static UNKNOWN_TYPEOF_VALUE: DiagnosticType =
    DiagnosticType::warning("JSC_UNKNOWN_TYPEOF_VALUE", "unknown type: {0}");

// port: TypeValidator#ILLEGAL_PROPERTY_ACCESS
pub static ILLEGAL_PROPERTY_ACCESS: DiagnosticType = DiagnosticType::warning(
    "JSC_ILLEGAL_PROPERTY_ACCESS",
    "Cannot do {0} access on a {1}",
);

// port: TypeValidator#ALL_DIAGNOSTICS
pub static ALL_DIAGNOSTICS: LazyLock<Arc<DiagnosticGroup>> = LazyLock::new(|| {
    Arc::new(DiagnosticGroup::new(&[
        &ABSTRACT_METHOD_NOT_IMPLEMENTED,
        &DUP_VAR_DECLARATION,
        &DUP_VAR_DECLARATION_TYPE_MISMATCH,
        &HIDDEN_INTERFACE_PROPERTY_MISMATCH,
        &ILLEGAL_PROPERTY_ACCESS,
        &INTERFACE_METHOD_NOT_IMPLEMENTED,
        &INVALID_ASYNC_RETURN_TYPE,
        &INVALID_CAST,
        &MISSING_EXTENDS_TAG_WARNING,
        &TYPE_MISMATCH_WARNING,
        &UNKNOWN_TYPEOF_VALUE,
    ]))
});

// port: TypeValidator#FOUND_REQUIRED
const FOUND_REQUIRED: &str = "{0}\nfound   : {1}\nrequired: {2}";

// port: TypeValidator#FOUND_REQUIRED_MISSING
const FOUND_REQUIRED_MISSING: &str =
    "{0}\nfound   : {1}\nrequired: {2}\nmissing : [{3}]\nmismatch: [{4}]";

/// A central reporter for all type violations: places where the programmer has annotated a
/// variable (or property) with one type, but has assigned another type to it.
///
/// Also doubles as a central repository for all type violations, so that type-based optimizations
/// (like AmbiguateProperties) can be fault-tolerant.
///
/// Rust: Java's passes keep the compiler's one validator in a field (`compiler.getTypeValidator()`)
/// and call it while they also use the compiler, so the compiler hands out an `Arc<TypeValidator>`
/// and every method takes the compiler (which owns the registry, DESIGN §8). The validator's two
/// mutable fields (`subtypingMode`, `mismatches`) sit behind mutexes, never held across a call
/// into the compiler.
#[derive(Debug)]
pub struct TypeValidator {
    all_bitwisable_value_types: TypeId,
    null_or_undefined: TypeId,
    promise_of_unknown_type: TypeId,
    iterable_or_async_iterable: TypeId,

    // In TypeCheck, when we are analyzing a file with .java.js suffix, we set
    // this field to IGNORE_NULL_UNDEFINED
    subtyping_mode: Mutex<SubtypingMode>,

    mismatches: Mutex<Accumulator>,
}

/// Rust-only: Java's `Supplier<String> typeNameSupplier`.
pub type TypeNameSupplier<'a> = &'a dyn Fn(&mut AbstractCompiler) -> String;

impl TypeValidator {
    // port: TypeValidator#TypeValidator
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let (type_registry, ast) = compiler.get_type_registry_and_ast();
        let ast: &Ast = ast;
        let all_bitwisable_value_types = type_registry.create_union_type_from_native(
            ast,
            &[STRING_TYPE, NUMBER_TYPE, BOOLEAN_TYPE, NULL_TYPE, VOID_TYPE],
        );
        let null_or_undefined = type_registry.get_native_type(JSTypeNative::NULL_VOID);
        let promise = type_registry.get_native_object_type(JSTypeNative::PROMISE_TYPE);
        let unknown = type_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let promise_of_unknown_type =
            type_registry.create_templatized_type(ast, promise, &[unknown]);
        let iterator = type_registry.get_native_object_type(JSTypeNative::ITERATOR_TYPE);
        let async_iterator =
            type_registry.get_native_object_type(JSTypeNative::ASYNC_ITERATOR_TYPE);
        let iterable_or_async_iterable =
            type_registry.create_union_type(ast, &[iterator, async_iterator]);
        Self {
            all_bitwisable_value_types,
            null_or_undefined,
            promise_of_unknown_type,
            iterable_or_async_iterable,
            subtyping_mode: Mutex::new(SubtypingMode::NORMAL),
            mismatches: Mutex::new(Accumulator::new()),
        }
    }

    /// Utility function that attempts to get an instance type from a potential constructor type
    // port: TypeValidator#getInstanceOfCtor
    pub fn get_instance_of_ctor(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        t: Option<TypeId>,
    ) -> Option<TypeId> {
        let t = t?;
        let ctor = t
            .dereference(reg, ast)
            .and_then(|d| d.to_maybe_function_type(reg));
        if let Some(ctor) = ctor
            && ctor.is_constructor(reg)
        {
            return FunctionType::get_instance_type(ctor, reg);
        }
        None
    }

    /// Gets a list of type violations.
    ///
    /// For each violation, one element is the expected type and the other is the type that is
    /// actually found. Order is not significant.
    ///
    /// NOTE(dimvar): Even though TypeMismatch is a pair, the passes that call this method never
    /// use it as a pair; they just add both its elements to a set of invalidating types. Consider
    /// just maintaining a set of types here instead of a set of type pairs.
    // port: TypeValidator#getMismatches
    pub fn get_mismatches(&self) -> Vec<TypeMismatch> {
        self.lock_mismatches().get_mismatches()
    }

    // port: TypeValidator#setSubtypingMode
    pub fn set_subtyping_mode(&self, mode: SubtypingMode) {
        *self
            .subtyping_mode
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = mode;
    }

    /// Rust-only: reads `subtypingMode`.
    fn subtyping_mode(&self) -> SubtypingMode {
        *self
            .subtyping_mode
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Rust-only: the `mismatches` accumulator.
    fn lock_mismatches(&self) -> MutexGuard<'_, Accumulator> {
        self.mismatches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    // All non-private methods should have the form:
    // expectCondition(Node n, ...);
    // If there is a mismatch, the {@code expect} method should issue
    // a warning and attempt to correct the mismatch, when possible.

    // port: TypeValidator#expectValidTypeofName
    pub fn expect_valid_typeof_name(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        found: &str,
    ) {
        let error = JSError::make(&compiler.ast, n, &UNKNOWN_TYPEOF_VALUE, &[found]);
        compiler.report(error);
    }

    /// Expect the type to be an object, or a type convertible to object. If the expectation is
    /// not met, issue a warning at the provided node's source code position.
    ///
    /// Returns true if there was no warning, false if there was a mismatch.
    // port: TypeValidator#expectObject
    pub fn expect_object(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_object_context(reg, ast) {
            self.mismatch_native(compiler, n, msg, type_, OBJECT_TYPE);
            return false;
        }
        true
    }

    /// Expect the type to be an object. Unlike expectObject, a type convertible to object is not
    /// acceptable.
    // port: TypeValidator#expectActualObject
    pub fn expect_actual_object(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_object(reg, ast) {
            self.mismatch_native(compiler, n, msg, type_, OBJECT_TYPE);
        }
    }

    /// Expect the type to contain an object sometimes. If the expectation is not met, issue a
    /// warning at the provided node's source code position.
    // port: TypeValidator#expectAnyObject
    pub fn expect_any_object(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let any_object_type = Self::get_native_type(compiler, NO_OBJECT_TYPE);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !any_object_type.is_subtype_of(reg, ast, type_) && !type_.is_empty_type(reg) {
            self.mismatch(compiler, n, msg, type_, any_object_type);
        }
    }

    /// Expect the type to autobox to be an Iterable.
    ///
    /// Returns true if there was no warning, false if there was a mismatch.
    // port: TypeValidator#expectAutoboxesToIterable
    pub fn expect_autoboxes_to_iterable(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) -> bool {
        // Note: we don't just use JSType.autobox() here because that removes null and undefined.
        // We want to keep null and undefined around.
        let iterable = Self::get_native_type(compiler, ITERABLE_TYPE);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if type_.is_union_type(reg) {
            let alternates = type_
                .to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast);
            for alt in alternates.iter().copied() {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let alt = if alt.is_boxable_scalar(reg) {
                    alt.autoboxes_to(reg).unwrap()
                } else {
                    alt
                };
                if !alt.is_subtype_of(reg, ast, iterable) {
                    self.mismatch_native(compiler, n, msg, type_, ITERABLE_TYPE);
                    return false;
                }
            }
        } else {
            let autoboxed_type = if type_.is_boxable_scalar(reg) {
                type_.autoboxes_to(reg).unwrap()
            } else {
                type_
            };
            if !autoboxed_type.is_subtype_of(reg, ast, iterable) {
                self.mismatch_native(compiler, n, msg, type_, ITERABLE_TYPE);
                return false;
            }
        }
        true
    }

    /// Expect the type to autobox to be an Iterable or AsyncIterable.
    ///
    /// Returns the unwrapped variants of the iterable(s), or empty if not iterable.
    // port: TypeValidator#expectAutoboxesToIterableOrAsyncIterable
    pub fn expect_autoboxes_to_iterable_or_async_iterable(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) -> Option<TypeId> {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let maybe_boxed = JsIterables::maybe_box_iterable_or_async_iterable(type_, reg, ast);

        if maybe_boxed.is_match() {
            return Some(maybe_boxed.get_templated_type());
        }

        self.mismatch(compiler, n, msg, type_, self.iterable_or_async_iterable);

        None
    }

    /// Expect the type to be a Generator or supertype of Generator.
    // port: TypeValidator#expectGeneratorSupertype
    pub fn expect_generator_supertype(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        if !self.native_is_subtype_of(compiler, GENERATOR_TYPE, type_) {
            self.mismatch_native(compiler, n, msg, type_, GENERATOR_TYPE);
        }
    }

    /// Expect the type to be a AsyncGenerator or supertype of AsyncGenerator.
    // port: TypeValidator#expectAsyncGeneratorSupertype
    pub fn expect_async_generator_supertype(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        if !self.native_is_subtype_of(compiler, ASYNC_GENERATOR_TYPE, type_) {
            self.mismatch_native(compiler, n, msg, type_, ASYNC_GENERATOR_TYPE);
        }
    }

    /// Expect the type to be a supertype of `Promise`.
    ///
    /// `Promise` is the *lower* bound of the declared return type, since that's what async
    /// functions always return; the user can't return an instance of a more specific type.
    // port: TypeValidator#expectValidAsyncReturnType
    pub fn expect_valid_async_return_type(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if self.promise_of_unknown_type.is_subtype_of(reg, ast, type_) {
            return;
        }

        let type_string = JSType::to_string(type_, reg, ast);
        let err = JSError::make(ast, n, &INVALID_ASYNC_RETURN_TYPE, &[&type_string]);
        self.register_mismatch_and_report(compiler, type_, self.promise_of_unknown_type, err);
    }

    /// Expect the type to be an ITemplateArray or supertype of ITemplateArray.
    // port: TypeValidator#expectITemplateArraySupertype
    pub fn expect_i_template_array_supertype(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        if !self.native_is_subtype_of(compiler, I_TEMPLATE_ARRAY_TYPE, type_) {
            self.mismatch_native(compiler, n, msg, type_, I_TEMPLATE_ARRAY_TYPE);
        }
    }

    /// Expect the type to be a string, or a type convertible to string. If the expectation is not
    /// met, issue a warning at the provided node's source code position.
    // port: TypeValidator#expectString
    pub fn expect_string(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_string_context(reg, ast) {
            self.mismatch_native(compiler, n, msg, type_, STRING_TYPE);
        }
    }

    /// Expect the type to be a number, or a type convertible to number. If the expectation is not
    /// met, issue a warning at the provided node's source code position.
    // port: TypeValidator#expectNumber
    pub fn expect_number(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_number_context(reg, ast) {
            self.mismatch_native(compiler, n, msg, type_, NUMBER_TYPE);
        } else {
            self.expect_number_strict(compiler, n, type_, msg);
        }
    }

    /// Expect the type to be a number or a subtype.
    // port: TypeValidator#expectNumberStrict
    pub fn expect_number_strict(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let number = Self::get_native_type(compiler, NUMBER_TYPE);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_subtype_of(reg, ast, number) {
            self.register_mismatch_and_report_diagnostic(
                compiler,
                n,
                &INVALID_OPERAND_TYPE,
                msg,
                type_,
                number,
                None,
                None,
            );
        }
    }

    // port: TypeValidator#expectMatchingTypesStrict
    pub fn expect_matching_types_strict(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: TypeId,
        right: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !left.is_subtype_of(reg, ast, right) && !right.is_subtype_of(reg, ast, left) {
            self.register_mismatch_and_report_diagnostic(
                compiler,
                n,
                &INVALID_OPERAND_TYPE,
                msg,
                right,
                left,
                None,
                None,
            );
        }
    }

    /// Expect the type to be a valid operand to a bitwise operator. This includes numbers, any
    /// type convertible to a number, or any other primitive type
    /// (undefined|null|boolean|string).
    // port: TypeValidator#expectBitwiseable
    pub fn expect_bitwiseable(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_number_context(reg, ast)
            && !type_.is_subtype_of(reg, ast, self.all_bitwisable_value_types)
        {
            self.mismatch(compiler, n, msg, type_, self.all_bitwisable_value_types);
        } else {
            self.expect_number_strict(compiler, n, type_, msg);
        }
    }

    /// Expect the type to be a number or string, or a type convertible to a number or symbol. If
    /// the expectation is not met, issue a warning at the provided node's source code position.
    // port: TypeValidator#expectNumberOrSymbol
    pub fn expect_number_or_symbol(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_number_context(reg, ast) && !type_.matches_symbol_context(reg, ast) {
            self.mismatch_native(compiler, n, msg, type_, NUMBER_SYMBOL);
        }
    }

    /// Expect the type to be a string or symbol, or a type convertible to a string. If the
    /// expectation is not met, issue a warning at the provided node's source code position.
    // port: TypeValidator#expectStringOrSymbol
    pub fn expect_string_or_symbol(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_string_context(reg, ast) && !type_.matches_symbol_context(reg, ast) {
            self.mismatch_native(compiler, n, msg, type_, STRING_SYMBOL);
        }
    }

    /// Expect the type to be unknown or a comparable type (bigint, number, or string)
    // port: TypeValidator#expectUnknownOrComparable
    pub fn expect_unknown_or_comparable(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let bigint_number_string = Self::get_native_type(compiler, BIGINT_NUMBER_STRING);
        let bigint_number_string_object =
            Self::get_native_type(compiler, BIGINT_NUMBER_STRING_OBJECT);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_subtype_of(reg, ast, bigint_number_string)
            && !type_.is_subtype_of(reg, ast, bigint_number_string_object)
        {
            if type_.matches_number_context(reg, ast) || type_.matches_string_context(reg, ast) {
                // INVALID_OPERAND_TYPE is suppressed unless strict type checking is enabled
                self.register_mismatch_and_report_diagnostic(
                    compiler,
                    n,
                    &INVALID_OPERAND_TYPE,
                    msg,
                    type_,
                    bigint_number_string,
                    None,
                    None,
                );
            } else {
                self.mismatch_native(compiler, n, msg, type_, BIGINT_NUMBER_STRING);
            }
        }
    }

    /// Expect the type to be a number or string or symbol, or a type convertible to a number or
    /// string. If the expectation is not met, issue a warning at the provided node's source code
    /// position.
    // port: TypeValidator#expectStringOrNumberOrSymbol
    pub fn expect_string_or_number_or_symbol(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.matches_number_context(reg, ast)
            && !type_.matches_string_context(reg, ast)
            && !type_.matches_symbol_context(reg, ast)
        {
            self.mismatch_native(compiler, n, msg, type_, NUMBER_STRING_SYMBOL);
        } else {
            self.expect_string_or_number_or_symbol_strict(compiler, n, type_, msg);
        }
    }

    // port: TypeValidator#expectStringOrNumberOrSymbolStrict
    pub fn expect_string_or_number_or_symbol_strict(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let number_string_symbol = Self::get_native_type(compiler, NUMBER_STRING_SYMBOL);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_subtype_of(reg, ast, number_string_symbol) {
            self.register_mismatch_and_report_diagnostic(
                compiler,
                n,
                &INVALID_OPERAND_TYPE,
                msg,
                type_,
                number_string_symbol,
                None,
                None,
            );
        }
    }

    /// Expect the type to be a bigint or number, or a type convertible to number. If the
    /// expectation is not met, issue a warning at the provided node's source code position.
    // port: TypeValidator#expectBigIntOrNumber
    pub fn expect_big_int_or_number(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        msg: &str,
    ) {
        let bigint_number = Self::get_native_type(compiler, BIGINT_NUMBER);
        let bigint_number_object = Self::get_native_type(compiler, BIGINT_NUMBER_OBJECT);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_subtype_of(reg, ast, bigint_number)
            && !type_.is_subtype_of(reg, ast, bigint_number_object)
        {
            if type_.matches_number_context(reg, ast) {
                // INVALID_OPERAND_TYPE is suppressed unless strict type checking is enabled
                self.register_mismatch_and_report_diagnostic(
                    compiler,
                    n,
                    &INVALID_OPERAND_TYPE,
                    msg,
                    type_,
                    bigint_number,
                    None,
                    None,
                );
            } else {
                self.mismatch_native(compiler, n, msg, type_, BIGINT_NUMBER);
            }
        }
    }

    /// Expect the type to be anything but the null or void type. If the expectation is not met,
    /// issue a warning at the provided node's source code position. Note that a union type that
    /// includes the void type and at least one other type meets the expectation.
    ///
    /// Returns whether the expectation was met.
    // port: TypeValidator#expectNotNullOrUndefined
    pub fn expect_not_null_or_undefined(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        type_: TypeId,
        msg: &str,
        expected_type: TypeId,
    ) -> bool {
        let in_global_scope = t.in_global_scope();
        let compiler = t.get_compiler();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_no_type(reg)
            && !type_.is_unknown_type(reg, ast)
            && type_.is_subtype_of(reg, ast, self.null_or_undefined)
            && !Self::contains_forward_declared_unresolved_name(reg, ast, type_)
        {
            // There's one edge case right now that we don't handle well, and
            // that we don't want to warn about.
            // if (this.x == null) {
            //   this.initializeX();
            //   this.x.foo();
            // }
            // In this case, we incorrectly type x because of how we
            // infer properties locally. See issue 109.
            // http://blickly.github.io/closure-compiler-issues/#109
            //
            // We do not do this inference globally.
            if n.is_get_prop(ast) && !in_global_scope && type_.is_null_type(reg) {
                return true;
            }

            self.mismatch(compiler, n, msg, type_, expected_type);
            return false;
        }
        true
    }

    // port: TypeValidator#containsForwardDeclaredUnresolvedName
    fn contains_forward_declared_unresolved_name(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> bool {
        if type_.is_union_type(reg) {
            let alternates = type_
                .to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast);
            for alt in alternates.iter().copied() {
                if Self::contains_forward_declared_unresolved_name(reg, ast, alt) {
                    return true;
                }
            }
        }
        type_.is_no_resolved_type(reg)
    }

    /// Expect that the type of a switch condition matches the type of its case condition.
    // port: TypeValidator#expectSwitchMatchesCase
    pub fn expect_switch_matches_case(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        switch_type: TypeId,
        case_type: TypeId,
    ) {
        // ECMA-262, page 68, step 3 of evaluation of CaseBlock
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !switch_type.can_test_for_shallow_equality_with(reg, ast, case_type) {
            let first_child = n.get_first_child(ast).unwrap();
            self.mismatch(
                compiler,
                first_child,
                "case expression doesn't match switch",
                case_type,
                switch_type,
            );
        }
    }

    /// Expect that the first type can be addressed with GETELEM syntax and that the second type
    /// is the right type for an index into the first type.
    ///
    /// `n`: The GETELEM or COMPUTED_PROP node to issue warnings on. `obj_type`: The type we're
    /// indexing into (the left side of the GETELEM). `index_type`: The type inside the brackets
    /// of the GETELEM/COMPUTED_PROP.
    // port: TypeValidator#expectIndexMatch
    pub fn expect_index_match(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        obj_type: TypeId,
        index_type: TypeId,
    ) {
        let ast = &compiler.ast;
        if !(n.is_get_elem(ast)
            || n.is_opt_chain_get_elem(ast)
            || n.is_computed_prop(ast)
            || n.is_computed_field_def(ast))
        {
            panic!("{n:?}");
        }
        let index_node = if n.is_get_elem(ast) {
            n.get_last_child(ast).unwrap()
        } else {
            n.get_first_child(ast).unwrap()
        };
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if index_type.is_symbol_value_type(reg) {
            // For now, allow symbols definitions/access on any type. In the future only allow
            // them on the subtypes for which they are defined.
            return;
        }
        if obj_type.is_unknown_type(reg, ast) {
            self.expect_string_or_number_or_symbol(
                compiler,
                index_node,
                index_type,
                "property access",
            );
            return;
        }
        let dereferenced = obj_type.dereference(reg, ast);
        let object_index_key = reg.get_object_index_key();
        if let Some(dereferenced) = dereferenced
            && dereferenced
                .get_template_type_map(reg)
                .has_template_key(object_index_key)
        {
            let template_type = dereferenced
                .get_template_type_map(reg)
                .get_resolved_template_type(reg, ast, object_index_key);
            self.expect_can_assign_to(
                compiler,
                index_node,
                index_type,
                template_type,
                "restricted index type",
            );
        } else if dereferenced.is_some_and(|d| d.is_array_type(reg)) {
            self.expect_number_or_symbol(compiler, index_node, index_type, "array access");
        } else if obj_type.is_struct(reg, ast) {
            let error = JSError::make(
                ast,
                index_node,
                &ILLEGAL_PROPERTY_ACCESS,
                &["'[]'", "struct"],
            );
            compiler.report(error);
        } else if obj_type.matches_object_context(reg, ast) {
            self.expect_string_or_symbol(compiler, index_node, index_type, "property access");
        } else {
            let required = reg.create_union_type_from_native(ast, &[ARRAY_TYPE, OBJECT_TYPE]);
            self.mismatch(
                compiler,
                n,
                "only arrays or objects can be accessed",
                obj_type,
                required,
            );
        }
    }

    /// Expect that the first type can be assigned to a symbol of the second type.
    ///
    /// `n`: The node to issue warnings on. `right_type`: The type on the RHS of the assign.
    /// `left_type`: The type of the symbol on the LHS of the assign. `owner`: The owner of the
    /// property being assigned to. `prop_name`: The name of the property being assigned to.
    /// Returns true if the types matched, false otherwise.
    // port: TypeValidator#expectCanAssignToPropertyOf(Node,JSType,JSType,Node,String)
    pub fn expect_can_assign_to_property_of(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        right_type: TypeId,
        left_type: TypeId,
        owner: NodeId,
        prop_name: &str,
    ) -> bool {
        let type_name_supplier: Box<dyn Fn(&mut AbstractCompiler) -> String> =
            if n.is_member_field_def(&compiler.ast) {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let class_type = n
                    .get_grandparent(ast)
                    .unwrap()
                    .get_jstype(ast)
                    .unwrap()
                    .assert_function_type(reg, ast);
                Box::new(move |compiler: &mut AbstractCompiler| {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let instance = FunctionType::get_instance_type(class_type, reg).unwrap();
                    JSType::to_string(instance, reg, ast)
                })
            } else {
                Box::new(move |compiler: &mut AbstractCompiler| {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    reg.get_readable_type_name(ast, owner)
                })
            };
        let owner_type = Self::get_js_type(compiler, owner);
        self.expect_can_assign_to_property_of_type(
            compiler,
            n,
            right_type,
            left_type,
            owner_type,
            &*type_name_supplier,
            prop_name,
        )
    }

    /// Expect that the first type can be assigned to a symbol of the second type.
    ///
    /// `n`: The node to issue warnings on. `right_type`: The type on the RHS of the assign.
    /// `left_type`: The type of the symbol on the LHS of the assign. `owner_type`: The owner of
    /// the property being assigned to. `prop_name`: The name of the property being assigned to.
    /// Returns true if the types matched, false otherwise.
    // port: TypeValidator#expectCanAssignToPropertyOf(Node,JSType,JSType,JSType,Supplier,String)
    #[allow(clippy::too_many_arguments)]
    pub fn expect_can_assign_to_property_of_type(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        right_type: TypeId,
        left_type: TypeId,
        owner_type: TypeId,
        type_name_supplier: TypeNameSupplier<'_>,
        prop_name: &str,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if left_type.is_template_type(reg) {
            let left = left_type.to_maybe_template_type(reg).unwrap();
            if right_type.contains_reference_ancestor(reg, ast, left)
                || right_type.is_unknown_type(reg, ast)
                || left.is_unknown_type(reg, ast)
            {
                // The only time we can assign to a variable with a template type is if the value
                // assigned has a type that explicitly has it as a supertype.
                // Otherwise, the template type is existential and it is unknown whether or not it
                // is a proper super type.
                return true;
            } else {
                let msg = format!(
                    "assignment to property {prop_name} of {}",
                    type_name_supplier(compiler)
                );
                self.register_mismatch_and_report_diagnostic(
                    compiler,
                    n,
                    &TYPE_MISMATCH_WARNING,
                    &msg,
                    right_type,
                    left_type,
                    Some(BTreeSet::new()),
                    Some(BTreeSet::new()),
                );
                return false;
            }
        }
        // The NoType check is a hack to make typedefs work OK.
        if !left_type.is_no_type(reg) && !right_type.is_subtype_of(reg, ast, left_type) {
            // Do not type-check interface methods, because we expect that
            // they will have dummy implementations that do not match the type
            // annotations.
            if owner_type.is_function_prototype_type(reg) {
                let owner_fn = owner_type
                    .to_object_type(reg)
                    .unwrap()
                    .get_owner_function(reg)
                    .unwrap();
                if owner_fn.is_interface(reg)
                    && right_type.is_function_type(reg)
                    && left_type.is_function_type(reg)
                {
                    return true;
                }
            }
            let msg = format!(
                "assignment to property {prop_name} of {}",
                type_name_supplier(compiler)
            );
            self.mismatch(compiler, n, &msg, right_type, left_type);
            return false;
        }
        true
    }

    /// Expect that the first type can be assigned to a symbol of the second type.
    ///
    /// `n`: The node to issue warnings on. `right_type`: The type on the RHS of the assign.
    /// `left_type`: The type of the symbol on the LHS of the assign. `msg`: An extra message for
    /// the mismatch warning, if necessary. Returns true if the types matched, false otherwise.
    // port: TypeValidator#expectCanAssignTo
    pub fn expect_can_assign_to(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        right_type: TypeId,
        left_type: TypeId,
        msg: &str,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if left_type.is_template_type(reg) {
            let left = left_type.to_maybe_template_type(reg).unwrap();
            if right_type.contains_reference_ancestor(reg, ast, left)
                || right_type.is_unknown_type(reg, ast)
                || left.is_unknown_type(reg, ast)
            {
                // The only time we can assign to a variable with a template type is if the value
                // assigned has a type that explicitly has it as a supertype.
                // Otherwise, the template type is existential and it is unknown whether or not it
                // is a proper super type.
                return true;
            } else {
                self.register_mismatch_and_report_diagnostic(
                    compiler,
                    n,
                    &TYPE_MISMATCH_WARNING,
                    msg,
                    right_type,
                    left_type,
                    Some(BTreeSet::new()),
                    Some(BTreeSet::new()),
                );
                return false;
            }
        }
        if !right_type.is_subtype_of(reg, ast, left_type) {
            self.mismatch(compiler, n, msg, right_type, left_type);
            return false;
        }
        true
    }

    /// Expect that the type of an argument matches the type of the parameter that it's
    /// fulfilling.
    ///
    /// `n`: The node to issue warnings on. `arg_type`: The type of the argument. `param_type`:
    /// The type of the parameter. `call_node`: The call node, to help with the warning message.
    /// `ordinal`: The argument ordinal, to help with the warning message.
    // port: TypeValidator#expectArgumentMatchesParameter
    pub fn expect_argument_matches_parameter(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        arg_type: TypeId,
        param_type: TypeId,
        call_node: NodeId,
        ordinal: i32,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !arg_type.is_subtype_of(reg, ast, param_type) {
            let callee = call_node.get_first_child(ast).unwrap();
            let msg = format!(
                "actual parameter {ordinal} of {} does not match formal parameter",
                reg.get_readable_type_name_no_deref(ast, callee)
            );
            self.mismatch(compiler, n, &msg, arg_type, param_type);
        }
    }

    /// Expect that the first type is the direct superclass of the second type.
    ///
    /// `n`: The node where warnings should point to. `super_object`: The expected super instance
    /// type. `sub_object`: The sub instance type.
    // port: TypeValidator#expectSuperType
    pub fn expect_super_type(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        super_object: TypeId,
        sub_object: TypeId,
    ) {
        let object_type = Self::get_native_type(compiler, OBJECT_TYPE);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let sub_ctor = ObjectType::get_constructor(sub_object, reg);
        let implicit_proto = sub_object.get_implicit_prototype(reg, ast);
        let mut declared_super = match implicit_proto {
            None => None,
            Some(implicit_proto) => implicit_proto.get_implicit_prototype(reg, ast),
        };
        if let Some(d) = declared_super
            && d.is_templatized_type(reg)
        {
            declared_super = Some(TemplatizedType::get_referenced_type(
                d.to_maybe_templatized_type(reg).unwrap(),
                reg,
            ));
        }
        if let Some(declared_super) = declared_super
            && !Self::is_unknown_type_instance(reg, super_object)
            && !declared_super.equals(reg, ast, super_object)
        {
            if declared_super.equals(reg, ast, object_type) {
                let sub_object_string = JSType::to_string(sub_object, reg, ast);
                let error =
                    JSError::make(ast, n, &MISSING_EXTENDS_TAG_WARNING, &[&sub_object_string]);
                self.register_mismatch_and_report(compiler, super_object, declared_super, error);
            } else {
                self.mismatch(
                    compiler,
                    n,
                    "mismatch in declaration of superclass type",
                    super_object,
                    declared_super,
                );
            }

            // Correct the super type.
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let sub_ctor = sub_ctor.unwrap();
            if !sub_ctor.has_cached_values(reg) {
                sub_ctor.set_prototype_based_on(reg, ast, super_object);
            }
        }
    }

    /// Rust-only: Java's `type instanceof UnknownType`. The registry creates exactly two
    /// UnknownType objects (JSTypeRegistry#initializeBuiltInTypes: UNKNOWN_TYPE and
    /// CHECKED_UNKNOWN_TYPE), so the class test is an identity test against them.
    fn is_unknown_type_instance(reg: &JSTypeRegistry, type_: TypeId) -> bool {
        type_ == reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)
            || type_ == reg.get_native_type(JSTypeNative::CHECKED_UNKNOWN_TYPE)
    }

    /// Expect that an ES6 class's extends clause is actually a supertype of the given class.
    /// Compares the registered supertype, which is taken from the JSDoc if present, otherwise from
    /// the AST, with the type in the extends node of the AST.
    ///
    /// `n`: The node where warnings should point to. `sub_ctor`: The sub constructor type.
    /// `ast_super_ctor`: The expected super constructor from the extends node in the AST.
    // port: TypeValidator#expectExtends
    pub fn expect_extends(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        sub_ctor: TypeId,
        ast_super_ctor: Option<TypeId>,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let Some(ast_super_ctor) =
            ast_super_ctor.filter(|c| c.is_constructor(reg) || c.is_interface(reg))
        else {
            // toMaybeFunctionType failed, or we've got a loose type.  Let it go for now.
            return;
        };
        if ast_super_ctor.is_constructor(reg) != sub_ctor.is_constructor(reg) {
            // Don't bother looking if one is a constructor and the other is an interface.
            // We'll report an error elsewhere.
            return;
        }
        let ast_super_instance = FunctionType::get_instance_type(ast_super_ctor, reg).unwrap();
        if sub_ctor.is_constructor(reg) {
            // There should be exactly one superclass, and it needs to have this constructor.
            // Note: if the registered supertype (from the @extends jsdoc) was unresolved,
            // then getSuperClassConstructor will be null - make sure not to crash.
            let registered_super_ctor = sub_ctor.get_super_class_constructor(reg, ast);
            if let Some(registered_super_ctor) = registered_super_ctor {
                let registered_super_instance =
                    FunctionType::get_instance_type(registered_super_ctor, reg).unwrap();
                if !ast_super_instance.equals(reg, ast, registered_super_instance) {
                    self.mismatch(
                        compiler,
                        n,
                        "mismatch in declaration of superclass type",
                        ast_super_instance,
                        registered_super_instance,
                    );
                }
            }
        } else if sub_ctor.is_interface(reg) {
            // We intentionally skip this check for interfaces because they can extend multiple
            // other interfaces.
        }
    }

    /// Expect that it's valid to assign something to a given type's prototype.
    ///
    /// Most of these checks occur during TypedScopeCreator, so we just handle very basic cases
    /// here
    ///
    /// For example, assuming `Foo` is a constructor, `Foo.prototype = 3;` will warn because `3`
    /// is not an object.
    ///
    /// `owner_type`: The type of the object whose prototype is being changed. (e.g. `Foo` above)
    /// `node`: Node to issue warnings on (e.g. `3` above) `right_type`: the rvalue type being
    /// assigned to the prototype (e.g. `number` above)
    // port: TypeValidator#expectCanAssignToPrototype
    pub fn expect_can_assign_to_prototype(
        &self,
        compiler: &mut AbstractCompiler,
        owner_type: TypeId,
        node: NodeId,
        right_type: TypeId,
    ) {
        let reg = compiler.get_type_registry();
        if owner_type.is_function_type(reg) {
            let function_type = owner_type.to_maybe_function_type(reg).unwrap();
            if function_type.is_constructor(reg) {
                self.expect_object(
                    compiler,
                    node,
                    right_type,
                    "cannot override prototype with non-object",
                );
            }
        }
    }

    /// Expect that the first type can be cast to the second type. The first type must have some
    /// relationship with the second.
    ///
    /// `n`: The node where warnings should point. `target_type`: The type being cast to.
    /// `source_type`: The type being cast from.
    // port: TypeValidator#expectCanCast
    pub fn expect_can_cast(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        target_type: TypeId,
        source_type: TypeId,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !source_type.can_cast_to(reg, ast, target_type) {
            let source_string = JSType::to_string(source_type, reg, ast);
            let target_string = JSType::to_string(target_type, reg, ast);
            let error = JSError::make(ast, n, &INVALID_CAST, &[&source_string, &target_string]);
            self.register_mismatch_and_report(compiler, source_type, target_type, error);
            // The "canCastTo" check is intentionally looser than the subtyping check, but we
            // still want to record potential mismatches for disambiguation safety.
        } else if !source_type.is_subtype_of(reg, ast, target_type) {
            self.lock_mismatches()
                .register_mismatch(reg, ast, n, source_type, target_type);
        }
    }

    /// Expect that the given variable has not been declared with a type.
    ///
    /// `source_name`: The name of the source file we're in. `n`: The node where warnings should
    /// point to. `parent`: The parent of `n`. `var`: The variable that we're checking.
    /// `variable_name`: The name of the variable. `new_type`: The type being applied to the
    /// variable. Mostly just here for the benefit of the warning.
    ///
    /// Returns the variable we end up with. Most of the time, this will just be `var`, but in
    /// some rare cases we will need to declare a new var with new source info.
    // port: TypeValidator#expectUndeclaredVariable
    #[allow(clippy::too_many_arguments)]
    pub fn expect_undeclared_variable(
        &self,
        compiler: &mut AbstractCompiler,
        _source_name: &str,
        input: Option<CompilerInput>,
        n: NodeId,
        parent: NodeId,
        var: TypedVar,
        variable_name: &str,
        new_type: Option<TypeId>,
    ) -> TypedVar {
        let mut new_var = var;
        let var_type = var.get_type(compiler);
        let unknown = Self::get_native_type(compiler, UNKNOWN_TYPE);

        // Only report duplicate declarations that have types. Other duplicates
        // will be reported by the syntactic scope creator later in the
        // compilation process.
        if let (Some(var_type), Some(new_type)) = (var_type, new_type)
            && var_type != unknown
            && new_type != unknown
        {
            // If there are two typed declarations of the same variable, that
            // is an error and the second declaration is ignored, except in the
            // case of native types. A null input type means that the declaration
            // was made in TypedScopeCreator#createInitialScope and is a
            // native type. We should redeclare it at the new input site.
            if var.get_input(compiler).is_none() {
                let s = var.get_scope(compiler);
                s.undeclare(compiler, var);
                new_var = s.declare(
                    compiler,
                    variable_name,
                    Some(n),
                    Some(var_type),
                    input,
                    false,
                );

                let ast = &mut compiler.ast;
                n.set_jstype(ast, Some(var_type));
                if parent.is_var(ast) {
                    if n.has_children(ast) {
                        n.get_first_child(ast)
                            .unwrap()
                            .set_jstype(ast, Some(var_type));
                    }
                } else if parent.is_expr_result(ast) {
                    n.set_jstype(ast, Some(var_type));
                } else {
                    check_state!(parent.is_function(ast) || parent.is_class(ast));
                    parent.set_jstype(ast, Some(var_type));
                }
            } else {
                // Check for @suppress duplicate or similar warnings guard on the previous variable
                // declaration location.
                let var_name_node = var.get_name_node(compiler).unwrap();
                let allow_dupe =
                    Self::has_duplicate_declaration_suppression(compiler, var_name_node);
                // If the previous definition doesn't suppress the warning, emit it here (i.e.
                // always emit on the second of the duplicate definitions). The warning might still
                // be suppressed by an @suppress tag on this declaration.
                if !allow_dupe {
                    let input_name = var.get_input_name(compiler);
                    let lineno = var_name_node.get_lineno(&compiler.ast).to_string();
                    let parent_node = var.get_parent_node(compiler).unwrap();
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    // Report specifically if it is not just a duplicate, but types also don't
                    // mismatch.
                    if !new_type.equals(reg, ast, var_type) {
                        let new_type_string = JSType::to_string(new_type, reg, ast);
                        let var_type_string = JSType::to_string(var_type, reg, ast);
                        let error = JSError::make(
                            ast,
                            n,
                            &DUP_VAR_DECLARATION_TYPE_MISMATCH,
                            &[
                                variable_name,
                                &new_type_string,
                                &input_name,
                                &lineno,
                                &var_type_string,
                            ],
                        );
                        compiler.report(error);
                    } else if !parent_node.is_expr_result(ast) {
                        // If the type matches and the previous declaration was a stub declaration
                        // (isExprResult), then ignore the duplicate, otherwise emit an error.
                        let error = JSError::make(
                            ast,
                            n,
                            &DUP_VAR_DECLARATION,
                            &[variable_name, &input_name, &lineno],
                        );
                        compiler.report(error);
                    }
                }
            }
        }

        new_var
    }

    /// Expect that all properties on interfaces that this type implements are implemented and
    /// correctly typed.
    // port: TypeValidator#expectAllInterfaceProperties
    pub fn expect_all_interface_properties(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let instance = FunctionType::get_instance_type(type_, reg).unwrap();
        for implemented in type_.get_all_implemented_interfaces(reg, ast) {
            self.expect_interface_properties(compiler, n, instance, implemented);
        }
        let reg = compiler.get_type_registry();
        for extended in type_.get_extended_interfaces(reg) {
            self.expect_interface_properties(compiler, n, instance, extended);
        }
    }

    // port: TypeValidator#expectInterfaceProperties
    fn expect_interface_properties(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        instance: TypeId,
        ancestor_interface: TypeId,
    ) {
        // Case: `/** @interface */ class Foo { constructor() { this.prop; } }`
        let reg = compiler.get_type_registry();
        for prop in ancestor_interface.get_own_property_keys(reg) {
            self.expect_interface_property(compiler, n, instance, ancestor_interface, prop);
        }
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if let Some(implicit_prototype) = ancestor_interface.get_implicit_prototype(reg, ast) {
            // Case: `/** @interface */ class Foo { prop() { } }`
            for prop in implicit_prototype.get_own_property_keys(reg) {
                self.expect_interface_property(compiler, n, instance, ancestor_interface, prop);
            }
        }
    }

    /// Expect that the property in an interface that this type implements is implemented and
    /// correctly typed.
    // port: TypeValidator#expectInterfaceProperty
    fn expect_interface_property(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        instance: TypeId,
        implemented_interface: TypeId,
        prop_name: PropertyKey,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let prop_slot = instance.find_closest_definition(reg, ast, prop_name.clone());
        let instance_ctor = ObjectType::get_constructor(instance, reg).unwrap();
        let not_implemented = match prop_slot {
            None => true,
            Some(prop_slot) => {
                !instance_ctor.is_interface(reg) && prop_slot.is_owned_by_interface(reg)
            }
        };
        if not_implemented {
            if FunctionType::is_abstract(instance_ctor, reg) || instance_ctor.is_interface(reg) {
                // Abstract classes and interfaces are not required to implement interface
                // properties.
                return;
            }
            if implemented_interface
                .get_property_type(reg, ast, prop_name.clone())
                .is_voidable(reg, ast)
            {
                // Voidable properties don't require explicit initialization in type
                // constructors.
                return;
            }

            let prop_display = prop_name.human_readable_name(reg).to_string();
            let interface_name =
                java_string(ObjectType::get_reference_name(implemented_interface, reg));
            let instance_string = JSType::to_string(instance, reg, ast);
            let error = JSError::make(
                ast,
                n,
                &INTERFACE_METHOD_NOT_IMPLEMENTED,
                &[&prop_display, &interface_name, &instance_string],
            );
            self.register_mismatch_and_report(compiler, instance, implemented_interface, error);
        } else {
            let prop_slot = prop_slot.unwrap();
            let local = prop_slot
                .get_owner_instance_type(reg)
                .equals(reg, ast, instance);
            if !local && instance_ctor.is_interface(reg) {
                // non-local interface mismatches already handled via interface property conflict
                // checks.
                return;
            }

            let prop = prop_slot.get_value();
            let mut prop_node = prop.get_declaration(reg).and_then(|d| d.get_node(reg));
            // Fall back on the constructor node if we can't find a node for the property.
            if prop_node.is_none() {
                prop_node = Some(n);
            }
            let prop_type = prop.get_type(reg);

            self.check_property_type(
                compiler,
                prop_node.unwrap(),
                instance,
                implemented_interface,
                prop_name,
                prop_type,
            );
        }
    }

    /// Check the property is correctly typed (i.e. subtype of the parent property's type
    /// declaration).
    // port: TypeValidator#checkPropertyType
    pub fn check_property_type(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        instance: TypeId,
        parent: TypeId,
        property_name: PropertyKey,
        found: TypeId,
    ) {
        let subtyping_mode = self.subtyping_mode();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let mut required = parent.get_property_type(reg, ast, property_name.clone());
        let type_map = instance.get_template_type_map(reg);
        if !type_map.is_empty() && required.has_any_template_types(reg, ast) {
            required = required.visit(
                reg,
                ast,
                &mut TemplateTypeReplacer::for_partial_replacement(type_map),
            );
        }

        if found.is_subtype_of_with_mode(reg, ast, required, subtyping_mode) {
            return;
        }

        // Implemented, but not correctly typed
        let parent_ctor = ObjectType::get_constructor(parent, reg).unwrap();
        let diagnostic = if parent_ctor.is_interface(reg) {
            &HIDDEN_INTERFACE_PROPERTY_MISMATCH
        } else {
            &HIDDEN_SUPERCLASS_PROPERTY_MISMATCH
        };
        let property_display = property_name.human_readable_name(reg).to_string();
        let parent_name = java_string(ObjectType::get_reference_name(parent, reg));
        let required_string = JSType::to_string(required, reg, ast);
        let found_string = JSType::to_string(found, reg, ast);
        let instance_string = JSType::to_string(instance, reg, ast);
        let err = JSError::make(
            ast,
            n,
            diagnostic,
            &[
                &property_display,
                &parent_name,
                &required_string,
                &found_string,
                &instance_string,
            ],
        );
        self.register_mismatch_and_report(compiler, found, required, err);
    }

    /// For a concrete class, expect that all abstract methods that haven't been implemented by
    /// any of the super classes on the inheritance chain are implemented.
    // port: TypeValidator#expectAbstractMethodsImplemented
    pub fn expect_abstract_methods_implemented(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        ctor_type: TypeId,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        check_argument!(ctor_type.is_constructor(reg));

        // Java LinkedHashMap<Property.Key, ObjectType>: insertion order, keys compared by equals.
        let mut abstract_method_super_type_map: Vec<(PropertyKey, TypeId)> = Vec::new();
        let mut curr_super_ctor = ctor_type.get_super_class_constructor(reg, ast);
        if curr_super_ctor.is_none_or(|c| !FunctionType::is_abstract(c, reg)) {
            return;
        }

        while let Some(c) = curr_super_ctor
            && FunctionType::is_abstract(c, reg)
        {
            let super_type = FunctionType::get_instance_type(c, reg).unwrap();
            let prototype = FunctionType::get_prototype(c, reg, ast);
            for prop in prototype.get_own_property_keys(reg) {
                let maybe_abstract_method = super_type
                    .find_property_type(reg, ast, prop.clone())
                    .unwrap()
                    .to_maybe_function_type(reg);
                if let Some(maybe_abstract_method) = maybe_abstract_method
                    && FunctionType::is_abstract(maybe_abstract_method, reg)
                    && !abstract_method_super_type_map
                        .iter()
                        .any(|(k, _)| *k == prop)
                {
                    abstract_method_super_type_map.push((prop, super_type));
                }
            }
            curr_super_ctor = c.get_super_class_constructor(reg, ast);
        }

        let instance = FunctionType::get_instance_type(ctor_type, reg).unwrap();
        for (method, super_type) in abstract_method_super_type_map {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let abstract_method = instance
                .find_property_type(reg, ast, method.clone())
                .unwrap()
                .to_maybe_function_type(reg);
            if abstract_method.is_none_or(|m| FunctionType::is_abstract(m, reg)) {
                let method_display = method.human_readable_name(reg).to_string();
                let super_type_string = JSType::to_string(super_type, reg, ast);
                let instance_string = JSType::to_string(instance, reg, ast);
                let error = JSError::make(
                    ast,
                    n,
                    &ABSTRACT_METHOD_NOT_IMPLEMENTED,
                    &[&method_display, &super_type_string, &instance_string],
                );
                self.register_mismatch_and_report(compiler, instance, super_type, error);
            }
        }
    }

    // port: TypeValidator#mismatch(Node,String,JSType,JSTypeNative)
    fn mismatch_native(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        msg: &str,
        found: TypeId,
        required: JSTypeNative,
    ) {
        let required = Self::get_native_type(compiler, required);
        self.mismatch(compiler, n, msg, found, required);
    }

    // port: TypeValidator#mismatch(Node,String,JSType,JSType)
    fn mismatch(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        msg: &str,
        found: TypeId,
        required: TypeId,
    ) {
        let subtyping_mode = self.subtyping_mode();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !found.is_subtype_of_with_mode(reg, ast, required, subtyping_mode) {
            let mut missing: Option<BTreeSet<JsString>> = None;
            let mut mismatch: Option<BTreeSet<JsString>> = None;
            let mut structural_type: Option<TypeId> = None;
            if required.is_structural_type(reg) {
                structural_type = Some(required);
            } else if required.is_union_type(reg) {
                let restricted_type = required.restrict_by_not_null_or_undefined(reg, ast);
                if restricted_type.is_structural_type(reg) {
                    // restrict to unions created by undefined `{!Type=}` type or by nullable
                    // `{?Type}` type.
                    structural_type = Some(restricted_type);
                }
            }

            if let Some(structural_type) = structural_type {
                let missing = missing.insert(BTreeSet::new());
                let mismatch = mismatch.insert(BTreeSet::new());
                let required_object = structural_type.to_maybe_object_type(reg);
                let found_object = found.to_maybe_object_type(reg);
                if let (Some(required_object), Some(found_object)) = (required_object, found_object)
                {
                    for property in required_object.get_property_names(reg, ast) {
                        let prop_required =
                            required_object.get_property_type(reg, ast, property.clone());
                        let has_property = found_object.has_property(reg, ast, property.clone());
                        if !prop_required.is_explicitly_voidable(reg, ast) || has_property {
                            if has_property {
                                if !found_object
                                    .get_property_type(reg, ast, property.clone())
                                    .is_subtype_of_with_mode(
                                        reg,
                                        ast,
                                        prop_required,
                                        subtyping_mode,
                                    )
                                {
                                    mismatch.insert(property);
                                }
                            } else {
                                missing.insert(property);
                            }
                        }
                    }
                }
            }
            self.register_mismatch_and_report_diagnostic(
                compiler,
                n,
                &TYPE_MISMATCH_WARNING,
                msg,
                found,
                required,
                missing,
                mismatch,
            );
        }
    }

    /// Used both for TYPE_MISMATCH_WARNING and INVALID_OPERAND_TYPE.
    // port: TypeValidator#registerMismatchAndReport(Node,DiagnosticType,String,JSType,JSType,Set,Set)
    #[allow(clippy::too_many_arguments)]
    fn register_mismatch_and_report_diagnostic(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        diagnostic: &'static DiagnosticType,
        msg: &str,
        found: TypeId,
        required: TypeId,
        missing: Option<BTreeSet<JsString>>,
        mismatch: Option<BTreeSet<JsString>>,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let found_required_formatted = Self::format_found_required(
            reg,
            ast,
            msg,
            found,
            required,
            missing.as_ref(),
            mismatch.as_ref(),
        );
        let err = JSError::make(ast, n, diagnostic, &[&found_required_formatted]);
        self.register_mismatch_and_report(compiler, found, required, err);
    }

    /// Registers a type mismatch into the universe of mismatches owned by this pass.
    // port: TypeValidator#registerMismatchAndReport(JSType,JSType,JSError)
    fn register_mismatch_and_report(
        &self,
        compiler: &mut AbstractCompiler,
        found: TypeId,
        required: TypeId,
        error: JSError,
    ) {
        let node = error.node().unwrap();
        compiler.report(error);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.lock_mismatches()
            .register_mismatch(reg, ast, node, found, required);
    }

    // port: TypeValidator#formatNodeLocation
    fn format_node_location(ast: &Ast, node: NodeId) -> String {
        format!(
            "{}:{}:{}",
            java_string(node.get_source_file_name(ast)),
            node.get_lineno(ast),
            node.get_charno(ast)
        )
    }

    /// Formats a found/required error message.
    // port: TypeValidator#formatFoundRequired
    fn format_found_required(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        description: &str,
        found: TypeId,
        required: TypeId,
        missing: Option<&BTreeSet<JsString>>,
        mismatch: Option<&BTreeSet<JsString>>,
    ) -> String {
        let mut found_str = JSType::to_string(found, reg, ast);
        let mut required_str = JSType::to_string(required, reg, ast);
        if found_str == required_str {
            found_str = found.to_annotation_string(reg, ast, Nullability::IMPLICIT);
            required_str = required.to_annotation_string(reg, ast, Nullability::IMPLICIT);
        }
        if found_str == required_str {
            // For some types, their annotation strings will be identical, but they might be
            // identically named structures in different scopes (and have theoretically different
            // "fully-qualified" names).
            // The general case of giving a correct fully qualified name is quite difficult, so
            // instead we address some common cases where we know that types have a concrete name
            // to use.
            if found.is_enum_element_type(reg) {
                let enum_type = found
                    .to_maybe_enum_element_type(reg)
                    .unwrap()
                    .get_enum_type(reg);
                let source = EnumType::get_source(enum_type, reg).unwrap();
                found_str = format!(
                    "{found_str} (enum definition: {})",
                    Self::format_node_location(ast, source)
                );
            }
            if required.is_enum_element_type(reg) {
                let enum_type = required
                    .to_maybe_enum_element_type(reg)
                    .unwrap()
                    .get_enum_type(reg);
                let source = EnumType::get_source(enum_type, reg).unwrap();
                required_str = format!(
                    "{required_str} (enum definition: {})",
                    Self::format_node_location(ast, source)
                );
            }
        }
        let mut missing_str = String::new();
        let mut mismatch_str = String::new();
        if let Some(missing) = missing
            && !missing.is_empty()
        {
            missing_str = join_comma(missing);
        }
        if let Some(mismatch) = mismatch
            && !mismatch.is_empty()
        {
            mismatch_str = join_comma(mismatch);
        }
        if !missing_str.is_empty() || !mismatch_str.is_empty() {
            Platform::format_message(
                FOUND_REQUIRED_MISSING,
                &[
                    description,
                    &found_str,
                    &required_str,
                    &missing_str,
                    &mismatch_str,
                ],
            )
        } else {
            Platform::format_message(FOUND_REQUIRED, &[description, &found_str, &required_str])
        }
    }

    /// This method gets the JSType from the Node argument and verifies that it is present.
    // port: TypeValidator#getJSType
    fn get_js_type(compiler: &AbstractCompiler, n: NodeId) -> TypeId {
        match n.get_jstype(&compiler.ast) {
            Some(type_) => type_,
            None => panic!("{n:?} has no JSType attached"),
        }
    }

    // port: TypeValidator#getNativeType
    fn get_native_type(compiler: &mut AbstractCompiler, type_id: JSTypeNative) -> TypeId {
        compiler.get_type_registry().get_native_type(type_id)
    }

    /// Rust-only: `getNativeType(native).isSubtypeOf(type)`.
    fn native_is_subtype_of(
        &self,
        compiler: &mut AbstractCompiler,
        native: JSTypeNative,
        type_: TypeId,
    ) -> bool {
        let native = Self::get_native_type(compiler, native);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        native.is_subtype_of(reg, ast, type_)
    }

    /// `decl`: The declaration to check. Returns whether duplicated declarations warnings should
    /// be suppressed for the given node.
    // port: TypeValidator#hasDuplicateDeclarationSuppression
    pub fn has_duplicate_declaration_suppression(
        compiler: &AbstractCompiler,
        decl: NodeId,
    ) -> bool {
        // NB: DUP_VAR_DECLARATION is somewhat arbitrary here, but it must be one of the errors
        // suppressed by the "duplicate" group.
        let original_decl_level = compiler.get_error_level(&JSError::make(
            &compiler.ast,
            decl,
            &DUP_VAR_DECLARATION,
            &["dummy", "dummy"],
        ));
        original_decl_level == CheckLevel::OFF
    }

    // port: TypeValidator#expectWellFormedTemplatizedType
    pub fn expect_well_formed_templatized_type(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut verifier = WellFormedTemplatizedTypeVerifier::new(n);
        if let Some(type_) = n.get_jstype(&compiler.ast) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            type_.visit(reg, ast, &mut verifier);
        }
        if let Some((found, required, error)) = verifier.pending_report {
            self.register_mismatch_and_report(compiler, found, required, error);
        }
    }
}

/// Rust-only: Java's `String.valueOf` of a nullable string ("null" for null).
fn java_string(s: Option<impl ToString>) -> String {
    s.map_or_else(|| "null".to_string(), |s| s.to_string())
}

/// Rust-only: Guava `Joiner.on(",").join(set)`.
fn join_comma(set: &BTreeSet<JsString>) -> String {
    set.iter()
        .map(JsString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Java's inner class: it calls the enclosing validator's `registerMismatchAndReport`, which
/// needs the compiler the visit has lent out (registry and Ast). Every case returns false at
/// once after that call and every caller propagates false unchanged, so a visit reports at most
/// once, as its very last action: the verifier records that report and
/// `expect_well_formed_templatized_type` makes it when the visit returns, which is the same
/// order of effects.
// port: TypeValidator.WellFormedTemplatizedTypeVerifier
pub struct WellFormedTemplatizedTypeVerifier {
    node: NodeId,
    pending_report: Option<(TypeId, TypeId, JSError)>,
}

impl WellFormedTemplatizedTypeVerifier {
    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#WellFormedTemplatizedTypeVerifier
    fn new(node: NodeId) -> Self {
        Self {
            node,
            pending_report: None,
        }
    }
}

impl WithDefaultCase<bool> for WellFormedTemplatizedTypeVerifier {
    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseDefault
    fn case_default(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _type: Option<TypeId>,
    ) -> bool {
        true
    }

    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseEnumElementType
    fn case_enum_element_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> bool {
        // Java's `getPrimitiveType() == null` cannot happen: an enum element always has one.
        type_.get_primitive_type(reg).visit(reg, ast, self)
    }

    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseFunctionType
    fn case_function_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        for param in type_.get_parameters(reg) {
            if !param.get_jstype().visit(reg, ast, self) {
                return false;
            }
        }
        FunctionType::get_return_type(type_, reg).visit(reg, ast, self)
    }

    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseNamedType
    fn case_named_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        NamedType::get_referenced_type(type_, reg).visit(reg, ast, self)
    }

    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseUnionType
    fn case_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        // Avoid iterators in very hot code.
        let alternates = UnionType::get_alternates(type_, reg, ast);
        let alternate_count = alternates.len();
        for i in 0..alternate_count {
            let alternative = alternates[i];
            if !alternative.visit(reg, ast, self) {
                return false;
            }
        }
        true
    }

    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseTemplatizedType
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> bool {
        let referenced_templates = TemplatizedType::get_referenced_type(type_, reg)
            .get_template_type_map(reg)
            .get_template_keys()
            .to_vec();
        let template_types = ObjectType::get_template_types(type_, reg).unwrap();
        for i in 0..template_types.len() {
            let assigned_type = template_types[i];
            if i < referenced_templates.len() {
                let template_type = referenced_templates[i];
                let bound = template_type.get_bound(reg);
                if !assigned_type.is_subtype(reg, ast, bound) {
                    let assigned_string = JSType::to_string(assigned_type, reg, ast);
                    let template_name =
                        java_string(ObjectType::get_reference_name(template_type, reg));
                    let bound_string = JSType::to_string(bound, reg, ast);
                    let error = JSError::make(
                        ast,
                        self.node,
                        &BOUNDED_GENERIC_TYPE_ERROR,
                        &[&assigned_string, &template_name, &bound_string],
                    );
                    self.pending_report = Some((assigned_type, bound, error));
                    return false;
                }
            }
        }
        true
    }

    // port: TypeValidator.WellFormedTemplatizedTypeVerifier#caseTemplateType
    fn case_template_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        template_type: TypeId,
    ) -> bool {
        !template_type.contains_cycle(reg, ast)
            && template_type.get_bound(reg).visit(reg, ast, self)
    }
}
