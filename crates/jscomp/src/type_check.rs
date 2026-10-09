/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TypeCheck.java,
//   src/com/google/javascript/jscomp/TypedScopeCreator.java.

use crate::diagnostic_group::DiagnosticGroup;
use crate::diagnostic_type::DiagnosticType;
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::AbstractScopeHandle,
    coding_convention::SubclassType,
    compiler_pass::CompilerPass,
    destructured_target::DestructuredTarget,
    diagnostic::log_file::LogFile,
    infer_js_doc_info::InferJSDocInfo,
    j2cl_source_utils::J2clSourceUtils,
    js_error::JSError,
    js_iterables::JsIterables,
    module_import_resolver::ModuleImportResolver,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    promises::Promises,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    type_inference_pass::TypeInferencePass,
    type_validator::{self, TypeValidator},
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{
    JSTypeNative::{
        self, ARRAY_TYPE, BIGINT_OBJECT_FUNCTION_TYPE, BIGINT_TYPE, BOOLEAN_TYPE, NULL_TYPE,
        NULL_VOID, NUMBER_TYPE, OBJECT_FUNCTION_TYPE, OBJECT_TYPE, PROMISE_TYPE,
        READONLY_ARRAY_TYPE, REGEXP_TYPE, STRING_TYPE, SYMBOL_OBJECT_FUNCTION_TYPE, UNKNOWN_TYPE,
        VOID_TYPE,
    },
    JSTypeRegistry, TypeId,
    function_type::Parameter,
    js_type::SubtypingMode,
    js_type_registry::PropDefinitionKind,
    prelude::{
        EnumElementType, EnumType, FunctionType, JSType, NamedType, ObjectType, TemplatizedType,
        UnionType,
    },
    property::{Property, PropertyKey, StringKey, SymbolKey},
    rhino::js_type_expression::JSTypeExpressionExt,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jscomp_base::tri::Tri,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock};

//
// Internal errors
//
// port: TypeCheck#UNEXPECTED_TOKEN
pub static UNEXPECTED_TOKEN: DiagnosticType = DiagnosticType::error(
    "JSC_INTERNAL_ERROR_UNEXPECTED_TOKEN",
    "Internal Error: TypeCheck doesn''t know how to handle {0}",
);

//
// User warnings
//
// port: TypeCheck#DETERMINISTIC_TEST
pub static DETERMINISTIC_TEST: DiagnosticType = DiagnosticType::warning(
    "JSC_DETERMINISTIC_TEST",
    "condition always evaluates to {2}\nleft : {0}\nright: {1}",
);

// port: TypeCheck#INEXISTENT_ENUM_ELEMENT
pub static INEXISTENT_ENUM_ELEMENT: DiagnosticType = DiagnosticType::warning(
    "JSC_INEXISTENT_ENUM_ELEMENT",
    "element {0} does not exist on this enum",
);

// port: TypeCheck#INEXISTENT_PROPERTY
pub static INEXISTENT_PROPERTY: DiagnosticType = DiagnosticType::warning(
    "JSC_INEXISTENT_PROPERTY",
    "Property {0} never defined on {1}",
);

// port: TypeCheck#POSSIBLE_INEXISTENT_PROPERTY_EXPLANATION
pub const POSSIBLE_INEXISTENT_PROPERTY_EXPLANATION: &str = "\n\nThis property is accessed on a \"loose\" type, but is not defined anywhere in the program, so it must not exist.";

// port: TypeCheck#POSSIBLE_INEXISTENT_PROPERTY
pub static POSSIBLE_INEXISTENT_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_POSSIBLE_INEXISTENT_PROPERTY",
    "Property {0} never defined on {1}\n\nThis property is accessed on a \"loose\" type, but is not defined anywhere in the program, so it must not exist.",
);

// port: TypeCheck#INEXISTENT_PROPERTY_WITH_SUGGESTION
pub static INEXISTENT_PROPERTY_WITH_SUGGESTION: DiagnosticType = DiagnosticType::warning(
    "JSC_INEXISTENT_PROPERTY_WITH_SUGGESTION",
    "Property {0} never defined on {1}. Did you mean {2}?",
);

// port: TypeCheck#STRICT_INEXISTENT_PROPERTY
pub static STRICT_INEXISTENT_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_STRICT_INEXISTENT_PROPERTY",
    "Property {0} never defined on {1}",
);

// port: TypeCheck#STRICT_INEXISTENT_UNION_PROPERTY
pub static STRICT_INEXISTENT_UNION_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_STRICT_INEXISTENT_UNION_PROPERTY",
    "Property {0} not defined on all member types of {1}",
);

// port: TypeCheck#STRICT_INEXISTENT_PROPERTY_WITH_SUGGESTION
pub static STRICT_INEXISTENT_PROPERTY_WITH_SUGGESTION: DiagnosticType = DiagnosticType::disabled(
    "JSC_STRICT_INEXISTENT_PROPERTY_WITH_SUGGESTION",
    "Property {0} never defined on {1}. Did you mean {2}?",
);

// port: TypeCheck#NOT_A_CONSTRUCTOR
pub static NOT_A_CONSTRUCTOR: DiagnosticType = DiagnosticType::warning(
    "JSC_NOT_A_CONSTRUCTOR",
    "cannot instantiate non-constructor, found type: {0}",
);

// port: TypeCheck#INSTANTIATE_ABSTRACT_CLASS
pub static INSTANTIATE_ABSTRACT_CLASS: DiagnosticType = DiagnosticType::warning(
    "JSC_INSTANTIATE_ABSTRACT_CLASS",
    "cannot instantiate abstract class",
);

// port: TypeCheck#BIT_OPERATION
pub static BIT_OPERATION: DiagnosticType = DiagnosticType::warning(
    "JSC_BAD_TYPE_FOR_BIT_OPERATION",
    "operator {0} cannot be applied to {1}",
);

// port: TypeCheck#UNARY_OPERATION
pub static UNARY_OPERATION: DiagnosticType = DiagnosticType::warning(
    "JSC_BAD_TYPE_FOR_UNARY_OPERATION",
    "unary operator {0} cannot be applied to {1}",
);

// port: TypeCheck#BINARY_OPERATION
pub static BINARY_OPERATION: DiagnosticType = DiagnosticType::warning(
    "JSC_BAD_TYPES_FOR_BINARY_OPERATION",
    "operator {0} cannot be applied to {1} and {2}",
);

// port: TypeCheck#NOT_CALLABLE
pub static NOT_CALLABLE: DiagnosticType =
    DiagnosticType::warning("JSC_NOT_FUNCTION_TYPE", "{0} expressions are not callable");

// port: TypeCheck#CONSTRUCTOR_NOT_CALLABLE
pub static CONSTRUCTOR_NOT_CALLABLE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTRUCTOR_NOT_CALLABLE",
    "Constructor {0} should be called with the \"new\" keyword",
);

// port: TypeCheck#ABSTRACT_SUPER_METHOD_NOT_USABLE
pub static ABSTRACT_SUPER_METHOD_NOT_USABLE: DiagnosticType = DiagnosticType::warning(
    "JSC_ABSTRACT_SUPER_METHOD_NOT_USABLE",
    "Abstract super method {0} cannot be dereferenced",
);

// port: TypeCheck#FUNCTION_MASKS_VARIABLE
pub static FUNCTION_MASKS_VARIABLE: DiagnosticType = DiagnosticType::warning(
    "JSC_FUNCTION_MASKS_VARIABLE",
    "function {0} masks variable (IE bug)",
);

// port: TypeCheck#MULTIPLE_VAR_DEF
pub static MULTIPLE_VAR_DEF: DiagnosticType = DiagnosticType::warning(
    "JSC_MULTIPLE_VAR_DEF",
    "declaration of multiple variables with shared type information",
);

// port: TypeCheck#INVALID_INTERFACE_MEMBER_DECLARATION
pub static INVALID_INTERFACE_MEMBER_DECLARATION: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_INTERFACE_MEMBER_DECLARATION",
    "interface members can only be empty property declarations, empty functions{0}",
);

// port: TypeCheck#INTERFACE_METHOD_NOT_EMPTY
pub static INTERFACE_METHOD_NOT_EMPTY: DiagnosticType = DiagnosticType::warning(
    "JSC_INTERFACE_METHOD_NOT_EMPTY",
    "interface member functions must have an empty body",
);

// port: TypeCheck#CONFLICTING_EXTENDED_TYPE
pub static CONFLICTING_EXTENDED_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONFLICTING_EXTENDED_TYPE",
    "{1} cannot extend this type; {0}s can only extend {0}s",
);

// port: TypeCheck#ES5_CLASS_EXTENDING_ES6_CLASS
pub static ES5_CLASS_EXTENDING_ES6_CLASS: DiagnosticType = DiagnosticType::warning(
    "JSC_ES5_CLASS_EXTENDING_ES6_CLASS",
    "ES5 class {0} cannot extend ES6 class {1}",
);

// port: TypeCheck#DICT_EXTEND_STRUCT_TYPE
pub static DICT_EXTEND_STRUCT_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_DICT_EXTEND_STRUCT_TYPE",
    "@dict class {0} cannot extend @struct class {1}",
);

// port: TypeCheck#STRUCT_EXTEND_DICT_TYPE
pub static STRUCT_EXTEND_DICT_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_DICT_EXTEND_STRUCT_TYPE",
    "@struct class {0} cannot extend @dict class {1}",
);

// port: TypeCheck#ES6_CLASS_EXTENDING_CLASS_WITH_GOOG_INHERITS
pub static ES6_CLASS_EXTENDING_CLASS_WITH_GOOG_INHERITS: DiagnosticType = DiagnosticType::warning(
    "JSC_ES6_CLASS_EXTENDING_CLASS_WITH_GOOG_INHERITS",
    "Do not use goog.inherits with ES6 classes. Use the ES6 `extends` keyword to inherit instead.",
);

// port: TypeCheck#INTERFACE_EXTENDS_LOOP
pub static INTERFACE_EXTENDS_LOOP: DiagnosticType = DiagnosticType::warning(
    "JSC_INTERFACE_EXTENDS_LOOP",
    "extends loop involving {0}, loop: {1}",
);

// port: TypeCheck#CONFLICTING_IMPLEMENTED_TYPE
pub static CONFLICTING_IMPLEMENTED_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONFLICTING_IMPLEMENTED_TYPE",
    "{0} cannot implement this type; an interface can only extend, but not implement interfaces",
);

// port: TypeCheck#BAD_IMPLEMENTED_TYPE
pub static BAD_IMPLEMENTED_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_IMPLEMENTS_NON_INTERFACE",
    "can only implement interfaces",
);

// port: TypeCheck#HIDDEN_SUPERCLASS_PROPERTY
pub static HIDDEN_SUPERCLASS_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_HIDDEN_SUPERCLASS_PROPERTY",
    "property {0} already defined on superclass {1}; use @override to override it",
);

// port: TypeCheck#HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY
pub static HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_PROTOTYPAL_HIDDEN_SUPERCLASS_PROPERTY",
    "property {0} already defined on supertype {1}; use @override to override it",
);

// port: TypeCheck#HIDDEN_INTERFACE_PROPERTY
pub static HIDDEN_INTERFACE_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_HIDDEN_INTERFACE_PROPERTY",
    "property {0} already defined on interface {1}; use @override to override it",
);

// port: TypeCheck#HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY_MISMATCH
pub static HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY_MISMATCH: DiagnosticType = DiagnosticType::warning(
    "JSC_HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY_MISMATCH",
    "mismatch of the {0} property type and the type of the property it overrides from supertype {1}\noriginal: {2}\noverride: {3}",
);

// port: TypeCheck#UNKNOWN_OVERRIDE
pub static UNKNOWN_OVERRIDE: DiagnosticType = DiagnosticType::warning(
    "JSC_UNKNOWN_OVERRIDE",
    "property {0} not defined on any superclass of {1}",
);

// port: TypeCheck#UNKNOWN_PROTOTYPAL_OVERRIDE
pub static UNKNOWN_PROTOTYPAL_OVERRIDE: DiagnosticType = DiagnosticType::warning(
    "JSC_UNKNOWN_PROTOTYPAL_OVERRIDE",
    "property {0} not defined on any supertype of {1}",
);

// port: TypeCheck#UNKNOWN_EXPR_TYPE
pub static UNKNOWN_EXPR_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_UNKNOWN_EXPR_TYPE",
    "could not determine the type of this expression",
);

// port: TypeCheck#WRONG_ARGUMENT_COUNT
pub static WRONG_ARGUMENT_COUNT: DiagnosticType = DiagnosticType::warning(
    "JSC_WRONG_ARGUMENT_COUNT",
    "Function {0}: called with {1} argument(s). Function requires at least {2} argument(s){3}.",
);

// port: TypeCheck#ILLEGAL_IMPLICIT_CAST
pub static ILLEGAL_IMPLICIT_CAST: DiagnosticType = DiagnosticType::warning(
    "JSC_ILLEGAL_IMPLICIT_CAST",
    "Illegal annotation on {0}. @implicitCast may only be used in externs.",
);

// port: TypeCheck#INCOMPATIBLE_EXTENDED_PROPERTY_TYPE
pub static INCOMPATIBLE_EXTENDED_PROPERTY_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_INCOMPATIBLE_EXTENDED_PROPERTY_TYPE",
    "Interface {0} has a property {1} with incompatible types in its super interfaces {2} and {3}",
);

// port: TypeCheck#EXPECTED_THIS_TYPE
pub static EXPECTED_THIS_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_EXPECTED_THIS_TYPE",
    "\"{0}\" must be called with a \"this\" type",
);

// port: TypeCheck#IN_USED_WITH_STRUCT
pub static IN_USED_WITH_STRUCT: DiagnosticType = DiagnosticType::warning(
    "JSC_IN_USED_WITH_STRUCT",
    "Cannot use the IN operator with structs",
);

// port: TypeCheck#ILLEGAL_PROPERTY_CREATION
pub static ILLEGAL_PROPERTY_CREATION: DiagnosticType = DiagnosticType::warning(
    "JSC_ILLEGAL_PROPERTY_CREATION",
    "Cannot add a property to a struct instance after it is constructed. (If you already declared the property, make sure to give it a type.)",
);

// port: TypeCheck#ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE
pub static ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE: DiagnosticType = DiagnosticType::disabled(
    "JSC_ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE",
    "Cannot add a property to an instance of union type.",
);

// port: TypeCheck#ILLEGAL_OBJLIT_KEY
pub static ILLEGAL_OBJLIT_KEY: DiagnosticType = DiagnosticType::warning(
    "JSC_ILLEGAL_OBJLIT_KEY",
    "Illegal key, the object literal is a {0}",
);

// port: TypeCheck#ILLEGAL_CLASS_KEY
pub static ILLEGAL_CLASS_KEY: DiagnosticType =
    DiagnosticType::warning("JSC_ILLEGAL_CLASS_KEY", "Illegal key, the class is a {0}");

// port: TypeCheck#NON_STRINGIFIABLE_OBJECT_KEY
pub static NON_STRINGIFIABLE_OBJECT_KEY: DiagnosticType = DiagnosticType::warning(
    "JSC_NON_STRINGIFIABLE_OBJECT_KEY",
    "Object type \"{0}\" contains non-stringifiable key and it may lead to an error. Please use ES6 Map instead or implement your own Map structure.",
);

// port: TypeCheck#ABSTRACT_METHOD_IN_CONCRETE_CLASS
pub static ABSTRACT_METHOD_IN_CONCRETE_CLASS: DiagnosticType = DiagnosticType::warning(
    "JSC_ABSTRACT_METHOD_IN_CONCRETE_CLASS",
    "Abstract methods can only appear in abstract classes. Please declare the class as @abstract",
);

// port: TypeCheck#CONFLICTING_GETTER_SETTER_TYPE
pub static CONFLICTING_GETTER_SETTER_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONFLICTING_GETTER_SETTER_TYPE",
    "The types of the getter and setter for property ''{0}'' do not match.\ngetter type is: {1}\nsetter type is: {2}",
);

// port: TypeCheck#SAME_INTERFACE_MULTIPLE_IMPLEMENTS
pub static SAME_INTERFACE_MULTIPLE_IMPLEMENTS: DiagnosticType = DiagnosticType::warning(
    "JSC_SAME_INTERFACE_MULTIPLE_IMPLEMENTS",
    "Cannot @implement the same interface more than once\nRepeated interface: {0}",
);

// port: TypeCheck#PROPERTY_ASSIGNMENT_TO_READONLY_VALUE
pub static PROPERTY_ASSIGNMENT_TO_READONLY_VALUE: DiagnosticType = DiagnosticType::error(
    "JSC_PROPERTY_ASSIGNMENT_TO_READONLY_VALUE",
    "Should not assign to a property of readonly type ''{0}''",
);

// port: TypeCheck#ALL_DIAGNOSTICS
pub static ALL_DIAGNOSTICS: LazyLock<Arc<DiagnosticGroup>> = LazyLock::new(|| {
    Arc::new(DiagnosticGroup::new(&[
        &ABSTRACT_METHOD_IN_CONCRETE_CLASS,
        &ABSTRACT_SUPER_METHOD_NOT_USABLE,
        &BAD_IMPLEMENTED_TYPE,
        &BINARY_OPERATION,
        &BIT_OPERATION,
        &CONFLICTING_EXTENDED_TYPE,
        &CONFLICTING_GETTER_SETTER_TYPE,
        &CONFLICTING_IMPLEMENTED_TYPE,
        &CONSTRUCTOR_NOT_CALLABLE,
        &DETERMINISTIC_TEST,
        &ES5_CLASS_EXTENDING_ES6_CLASS,
        &EXPECTED_THIS_TYPE,
        &FUNCTION_MASKS_VARIABLE,
        &HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY_MISMATCH,
        &ILLEGAL_CLASS_KEY,
        &ILLEGAL_IMPLICIT_CAST,
        &ILLEGAL_OBJLIT_KEY,
        &ILLEGAL_PROPERTY_CREATION,
        &ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE,
        &INCOMPATIBLE_EXTENDED_PROPERTY_TYPE,
        &INEXISTENT_ENUM_ELEMENT,
        &INEXISTENT_PROPERTY,
        &INEXISTENT_PROPERTY_WITH_SUGGESTION,
        &INSTANTIATE_ABSTRACT_CLASS,
        &INTERFACE_METHOD_NOT_EMPTY,
        &INVALID_INTERFACE_MEMBER_DECLARATION,
        &IN_USED_WITH_STRUCT,
        &MULTIPLE_VAR_DEF,
        &NON_STRINGIFIABLE_OBJECT_KEY,
        &NOT_A_CONSTRUCTOR,
        &NOT_CALLABLE,
        &STRUCT_EXTEND_DICT_TYPE,
        &DICT_EXTEND_STRUCT_TYPE,
        &POSSIBLE_INEXISTENT_PROPERTY,
        &PROPERTY_ASSIGNMENT_TO_READONLY_VALUE,
        &crate::rhino_error_reporter::CYCLIC_INHERITANCE_ERROR,
        &crate::rhino_error_reporter::TOO_MANY_TEMPLATE_PARAMS,
        &crate::rhino_error_reporter::TYPE_PARSE_ERROR,
        &crate::rhino_error_reporter::UNRECOGNIZED_TYPE_ERROR,
        &SAME_INTERFACE_MULTIPLE_IMPLEMENTS,
        &crate::type_validator::HIDDEN_SUPERCLASS_PROPERTY_MISMATCH,
        &crate::typed_scope_creator::CTOR_INITIALIZER,
        &crate::typed_scope_creator::IFACE_INITIALIZER,
        &crate::typed_scope_creator::LENDS_ON_NON_OBJECT,
        &crate::typed_scope_creator::UNKNOWN_LENDS,
        &UNARY_OPERATION,
        &UNKNOWN_OVERRIDE,
        &UNKNOWN_PROTOTYPAL_OVERRIDE,
        &WRONG_ARGUMENT_COUNT,
    ]))
});

// port: TypeCheck#ES5_INHERITANCE_DIAGNOSTIC_GROUP
pub static ES5_INHERITANCE_DIAGNOSTIC_GROUP: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| Arc::new(DiagnosticGroup::new(&[&ES5_CLASS_EXTENDING_ES6_CLASS])));

// port: TypeCheck#GOOG_INHERITS
static GOOG_INHERITS: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.inherits"));

/// Rust-only: Java's `LinkedHashMap<Property.Key, ObjectType>` in checkInterface (PropertyKey has
/// no Hash; insertion order is kept, `put` on an existing key replaces the value in place).
#[derive(Default)]
struct PropertyKeyMap(Vec<(PropertyKey, TypeId)>);

impl PropertyKeyMap {
    fn get(&self, key: &PropertyKey) -> Option<TypeId> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }

    fn put(&mut self, key: PropertyKey, value: TypeId) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key, value)),
        }
    }

    fn put_all(&mut self, other: &PropertyKeyMap) {
        for (k, v) in &other.0 {
            self.put(k.clone(), *v);
        }
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

/// Rust-only: Java's `SuggestionPair`.
// port: TypeCheck.SuggestionPair
struct SuggestionPair {
    suggestion: String,
    // Java never reads it either (getClosestPropertySuggestion always returns null).
    #[allow(dead_code)]
    distance: i32,
}

/// Checks the types of JS expressions against any declared type information.
///
/// The Java fields `compiler` and `typeRegistry` are not stored (DESIGN §6): every method takes
/// the compiler, which owns the registry. Java shares the `TypedScopeCreator` with the compiler;
/// here TypeCheck owns it while it runs (the checkTypes factory takes it from the compiler and
/// restores it afterwards, see `into_scope_creator`).
pub struct TypeCheck {
    validator: Arc<TypeValidator>,

    reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,

    top_scope: Option<TypedScope>,

    scope_creator: Option<TypedScopeCreator>,

    report_unknown_types: bool,
    subtyping_mode: SubtypingMode,

    // This may be expensive, so don't emit these warnings if they're
    // explicitly turned off.
    report_missing_properties: bool,

    infer_js_doc_info: Option<InferJSDocInfo>,

    // These fields are used to calculate the percentage of expressions typed.
    typed_count: i32,
    null_count: i32,
    unknown_count: i32,
    in_externs: bool,

    /// Logs types for @logTypeInCompiler.
    debug_type_logger: Option<DebugTypeLogger>,
}

impl TypeCheck {
    // port: TypeCheck#TypeCheck(AbstractCompiler,ReverseAbstractInterpreter,JSTypeRegistry,TypedScope,TypedScopeCreator)
    pub fn new_with_scope(
        compiler: &mut AbstractCompiler,
        reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
        top_scope: Option<TypedScope>,
        scope_creator: Option<TypedScopeCreator>,
    ) -> Self {
        let validator = compiler.get_type_validator();
        let infer_js_doc_info = Some(InferJSDocInfo::new(compiler));
        Self {
            validator,
            reverse_interpreter,
            top_scope,
            scope_creator,
            report_unknown_types: false,
            subtyping_mode: SubtypingMode::NORMAL,
            report_missing_properties: true,
            infer_js_doc_info,
            typed_count: 0,
            null_count: 0,
            unknown_count: 0,
            in_externs: false,
            debug_type_logger: None,
        }
    }

    // port: TypeCheck#TypeCheck(AbstractCompiler,ReverseAbstractInterpreter,JSTypeRegistry)
    pub fn new(
        compiler: &mut AbstractCompiler,
        reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
    ) -> Self {
        Self::new_with_scope(compiler, reverse_interpreter, None, None)
    }

    /// Turn on the missing property check. Returns this for easy chaining.
    // port: TypeCheck#reportMissingProperties
    pub fn report_missing_properties(mut self, report: bool) -> Self {
        self.report_missing_properties = report;
        self
    }

    /// Turn on the unknown types check. Returns this for easy chaining.
    // port: TypeCheck#reportUnknownTypes
    pub fn report_unknown_types(mut self, report: bool) -> Self {
        self.report_unknown_types = report;
        self
    }

    /// Rust-only: hands back the scope creator Java shares with the compiler.
    pub fn into_scope_creator(self) -> Option<TypedScopeCreator> {
        self.scope_creator
    }

    /// Main entry point of this phase of processing (`externsRoot` may be null).
    // port: TypeCheck#process
    pub fn process_nullable(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs_root: Option<NodeId>,
        js_root: NodeId,
    ) {
        check_not_null!(self.scope_creator.as_ref());
        check_not_null!(self.top_scope);

        let externs_and_js = js_root.get_parent(compiler);
        check_state!(externs_and_js.is_some());
        check_state!(
            externs_root.is_none()
                || externs_and_js
                    .unwrap()
                    .has_child(compiler, externs_root.unwrap())
        );

        // try-with-resources: the logger closes when the block ends.
        self.debug_type_logger = Some(DebugTypeLogger::new(compiler));
        if let Some(externs_root) = externs_root {
            self.check(compiler, externs_root, true);
        }
        self.check(compiler, js_root, false);
        if let Some(logger) = self.debug_type_logger.as_mut() {
            logger.close();
        }
    }

    /// Main entry point of this phase for testing code.
    ///
    /// `externs_root` may be null or a ROOT node. If null the externs are not typechecked. Note:
    /// the externs node must always exist in the AST, even if not typechecked. `js_root` must be
    /// a ROOT node and the second child of the global ROOT.
    // port: TypeCheck#processForTesting
    pub fn process_for_testing(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs_root: Option<NodeId>,
        js_root: NodeId,
    ) -> TypedScope {
        check_state!(self.scope_creator.is_none());
        check_state!(self.top_scope.is_none());

        check_argument!(
            externs_root.is_none() || externs_root.unwrap().is_root(compiler),
            "%s",
            externs_root.map_or_else(|| "null".to_owned(), |r| r.to_string(compiler))
        );
        check_argument!(js_root.is_root(compiler), "%s", js_root.to_string(compiler));

        check_state!(
            js_root.has_parent(compiler) && js_root.get_parent(compiler).unwrap().is_root(compiler),
            "%s",
            js_root
                .get_parent(compiler)
                .map_or_else(|| "null".to_owned(), |p| p.to_string(compiler))
        );
        check_state!(
            externs_root.is_none() || externs_root.unwrap().get_next(compiler) == Some(js_root),
            "externs root must be the preceding sibling of the js root"
        );

        let scope_creator = TypedScopeCreator::new(compiler);

        let mut inference_pass = TypeInferencePass::new(
            compiler,
            Arc::clone(&self.reverse_interpreter),
            scope_creator,
        );
        let global_root = js_root.get_parent(compiler).unwrap();
        self.top_scope = Some(inference_pass.infer_all_scopes(compiler, global_root));
        self.scope_creator = Some(inference_pass.into_scope_creator());

        self.process_nullable(compiler, externs_root, js_root);

        compiler.set_type_checking_has_run(true);
        compiler.set_top_scope(self.top_scope);

        self.top_scope.unwrap()
    }

    // port: TypeCheck#check
    pub fn check(&mut self, compiler: &mut AbstractCompiler, node: NodeId, externs: bool) {
        self.in_externs = externs;
        let mut scope_creator = self.scope_creator.take();
        let top_scope = self.top_scope.unwrap();
        {
            let mut adapter = scope_creator.as_mut().map(TypedScopeCreatorAdapter);
            let mut builder = NodeTraversal::builder();
            builder.set_compiler(compiler).set_callback(self);
            if let Some(adapter) = adapter.as_mut() {
                builder.set_scope_creator(adapter);
            }
            builder.traverse_with_scope(node, top_scope);
        }
        self.scope_creator = scope_creator;
        let infer_js_doc_info = self.infer_js_doc_info.as_mut().unwrap();
        if externs {
            infer_js_doc_info.process_nullable(compiler, Some(node), None);
        } else {
            infer_js_doc_info.process_nullable(compiler, None, Some(node));
        }
    }

    // port: TypeCheck#report
    fn report(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        diagnostic_type: &'static DiagnosticType,
        arguments: &[&str],
    ) {
        let error = JSError::make(compiler, n, diagnostic_type, arguments);
        compiler.report(error);
    }

    // port: TypeCheck#shouldTraverse
    fn should_traverse_impl(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Start logging types if we're in debug mode and this node was annotated.
        self.debug_type_logger
            .as_mut()
            .unwrap()
            .maybe_start_logging_at(t.get_compiler(), n);

        if n.is_script(t) {
            if NodeUtil::is_from_type_summary(t, n) {
                // Errors in type summary files are suppressed, so no use traversing them.
                return false;
            }
            if J2clSourceUtils::is_j2cl_source_node(t, n) {
                self.subtyping_mode = SubtypingMode::IGNORE_NULL_UNDEFINED;
            } else {
                self.subtyping_mode = SubtypingMode::NORMAL;
            }
            self.validator.set_subtyping_mode(self.subtyping_mode);
        } else if n.is_function(t) {
            // normal type checking
            let outer_scope = t.get_typed_scope();
            // Check for a bug in IE9 (quirks mode) and earlier where bleeding function names would
            // refer to the wrong variable (i.e. "var x; var y = function x() { use(x); }" would pass
            // the 'x' from the outer scope, rather than the local alias bled into the function).
            let name = n.get_first_child(t).unwrap().get_string(t);
            let compiler = t.get_compiler();
            let var = outer_scope.get_var(compiler, name);
            if let Some(var) = var
                && var
                    .get_scope(compiler)
                    .has_same_container_scope(compiler, outer_scope)
                // Ideally, we would want to check whether the type in the scope
                // differs from the type being defined, but then the extern
                // redeclarations of built-in types generates spurious warnings.
                && !var
                    .get_type(compiler)
                    .is_some_and(|ty| Self::is_function_type_instance(compiler, ty))
                && !TypeValidator::has_duplicate_declaration_suppression(
                    compiler,
                    var.get_name_node(compiler).expect("NullPointerException"),
                )
            {
                let var_name = var.get_name(compiler).to_string_lossy();
                Self::report(compiler, n, &FUNCTION_MASKS_VARIABLE, &[&var_name]);
            }

            // TODO(user): Only traverse the function's body. The function's
            // name and arguments are traversed by the scope creator, and ideally
            // should not be traversed by the type checker.
        }
        true
    }

    /// Rust-only: Java's `type instanceof FunctionType` (FunctionType and its subclasses
    /// NoObjectType, NoType and NoResolvedType; a @typedef'd name's var has the NoType).
    fn is_function_type_instance(compiler: &mut AbstractCompiler, type_: TypeId) -> bool {
        let reg = compiler.get_type_registry();
        matches!(
            type_.get_type_class(reg),
            closure_jstype::js_type_class::JSTypeClass::FUNCTION
                | closure_jstype::js_type_class::JSTypeClass::NO_OBJECT
                | closure_jstype::js_type_class::JSTypeClass::NO
                | closure_jstype::js_type_class::JSTypeClass::NO_RESOLVED
        )
    }

    /// This is the meat of the type checking. It is basically one big switch, with each case
    /// representing one type of parse tree node. The individual cases are usually pretty
    /// straightforward.
    // port: TypeCheck#visit
    fn visit_impl(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let child_type: TypeId;
        let left_type: TypeId;
        let right_type: TypeId;
        let left: NodeId;
        let right: NodeId;
        // To be explicitly set to false if the node is not typeable.
        let mut typeable = true;

        self.validator
            .expect_well_formed_templatized_type(t.get_compiler(), n);

        match n.get_token(t) {
            Token::CAST => {
                let compiler = t.get_compiler();
                let expr = n.get_first_child(compiler).unwrap();
                let expr_type = self.get_js_type(compiler, expr);
                let cast_type = self.get_js_type(compiler, n);

                // TODO(johnlenz): determine if we can limit object literals in some
                // way.
                if !expr.is_object_lit(compiler) {
                    self.validator
                        .expect_can_cast(compiler, n, cast_type, expr_type);
                }
                self.ensure_typed_with_type(compiler, n, Some(cast_type));

                expr.set_jstype_before_cast(compiler, Some(expr_type));
                let restricted = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let restricted = cast_type.restrict_by_not_null_or_undefined(reg, ast);
                    restricted.is_subtype_of(reg, ast, expr_type)
                };
                if restricted || expr.is_object_lit(compiler) {
                    expr.set_jstype(compiler, Some(cast_type));
                }
            }
            Token::NAME => typeable = self.visit_name(t, n, parent),
            Token::COMMA => {
                let compiler = t.get_compiler();
                let last = n.get_last_child(compiler).unwrap();
                let ty = self.get_js_type(compiler, last);
                self.ensure_typed_with_type(compiler, n, Some(ty));
            }
            Token::THIS => {
                let scope = t.get_typed_scope();
                let compiler = t.get_compiler();
                let ty = scope.get_type_of_this(compiler);
                self.ensure_typed_with_type(compiler, n, ty);
            }
            Token::NULL => self.ensure_typed_native(t.get_compiler(), n, NULL_TYPE),
            Token::NUMBER => self.ensure_typed_native(t.get_compiler(), n, NUMBER_TYPE),
            Token::BIGINT => self.ensure_typed_native(t.get_compiler(), n, BIGINT_TYPE),
            Token::GETTER_DEF | Token::SETTER_DEF => {
                // Object literal keys are handled with OBJECTLIT
            }
            Token::ARRAYLIT => self.ensure_typed_native(t.get_compiler(), n, ARRAY_TYPE),
            Token::REGEXP => self.ensure_typed_native(t.get_compiler(), n, REGEXP_TYPE),
            Token::GETPROP => {
                self.visit_get_prop(t, n);
                let parent = parent.unwrap();
                typeable = !(parent.is_assign(t) && parent.get_first_child(t) == Some(n));
            }
            Token::OPTCHAIN_GETPROP => self.visit_opt_chain_get_prop(t.get_compiler(), n),
            Token::OPTCHAIN_GETELEM => self.visit_opt_chain_get_elem(t.get_compiler(), n),
            Token::GETELEM => {
                self.visit_get_elem(t.get_compiler(), n);
                // The type of GETELEM is always unknown, so no point counting that.
                // If that unknown leaks elsewhere (say by an assignment to another
                // variable), then it will be counted.
                typeable = false;
            }
            Token::VAR | Token::LET | Token::CONST => {
                self.visit_var(t, n);
                typeable = false;
            }
            Token::NEW => self.visit_new(t.get_compiler(), n),
            Token::OPTCHAIN_CALL => {
                // We reuse the `visitCall` functionality for OptChain call nodes because we don't
                // report an error for regular calls when the callee is null or undefined. However,
                // we make sure `typeable` isn't explicitly unset as OptChain nodes are always typed
                // during inference.
                self.visit_call(t, n)
            }
            Token::CALL => {
                self.visit_call(t, n);
                typeable = !parent.unwrap().is_expr_result(t);
            }
            Token::RETURN => {
                self.visit_return(t, n);
                typeable = false;
            }
            Token::YIELD => self.visit_yield(t, n),
            Token::DEC | Token::INC => {
                let compiler = t.get_compiler();
                left = n.get_first_child(compiler).unwrap();
                self.check_prop_creation(compiler, left);
                let n_type = self.get_js_type(compiler, n);
                let is_number = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    n_type.is_number(reg, ast)
                };
                let left_js_type = self.get_js_type(compiler, left);
                if is_number {
                    self.validator.expect_number(
                        compiler,
                        left,
                        left_js_type,
                        "increment/decrement",
                    );
                    self.ensure_typed_native(compiler, n, NUMBER_TYPE);
                } else {
                    self.validator.expect_big_int_or_number(
                        compiler,
                        left,
                        left_js_type,
                        "increment/decrement",
                    );
                }
            }
            Token::VOID => self.ensure_typed_native(t.get_compiler(), n, VOID_TYPE),
            Token::STRINGLIT | Token::TYPEOF | Token::TEMPLATELIT | Token::TEMPLATELIT_STRING => {
                self.ensure_typed_native(t.get_compiler(), n, STRING_TYPE)
            }
            Token::TAGGED_TEMPLATELIT => {
                self.visit_tagged_template_lit(t.get_compiler(), n);
                self.ensure_typed(t.get_compiler(), n);
            }
            Token::BITNOT => self.visit_bitwise_not(t.get_compiler(), n),
            Token::POS => self.visit_unary_plus(t.get_compiler(), n),
            Token::NEG => self.visit_unary_minus(t.get_compiler(), n),
            Token::EQ | Token::NE | Token::SHEQ | Token::SHNE => {
                let compiler = t.get_compiler();
                left = n.get_first_child(compiler).unwrap();
                right = n.get_last_child(compiler).unwrap();

                if left.is_type_of(compiler) {
                    if right.is_string_lit(compiler) {
                        // Display: Java's UTF-8 error stream text (an unpaired surrogate is '?').
                        let s = right.get_string(compiler).to_string();
                        self.check_typeof_string(compiler, right, &s);
                    }
                } else if right.is_type_of(compiler) && left.is_string_lit(compiler) {
                    // Display: Java's UTF-8 error stream text (an unpaired surrogate is '?').
                    let s = left.get_string(compiler).to_string();
                    self.check_typeof_string(compiler, left, &s);
                }

                left_type = self.get_js_type(compiler, left);
                right_type = self.get_js_type(compiler, right);

                // We do not want to warn about explicit comparisons to VOID. People
                // often do this if they think their type annotations screwed up.
                //
                // We do want to warn about cases where people compare things like
                // (Array|null) == (Function|null)
                // because it probably means they screwed up.
                //
                // This heuristic here is not perfect, but should catch cases we
                // care about without too many false negatives.
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let left_type_restricted = left_type.restrict_by_not_null_or_undefined(reg, ast);
                let right_type_restricted = right_type.restrict_by_not_null_or_undefined(reg, ast);

                let mut result = Tri::UNKNOWN;
                if n.is_eq(ast) || n.is_ne(ast) {
                    result = left_type_restricted
                        .test_for_equality(reg, ast, right_type_restricted)
                        .expect("NullPointerException");
                    if n.is_ne(ast) {
                        result = result.not();
                    }
                } else {
                    // SHEQ or SHNE
                    if !left_type_restricted.can_test_for_shallow_equality_with(
                        reg,
                        ast,
                        right_type_restricted,
                    ) {
                        result = if n.is_sheq(ast) {
                            Tri::FALSE
                        } else {
                            Tri::TRUE
                        };
                    }
                }

                if result != Tri::UNKNOWN {
                    let left_string = Self::type_to_string(compiler, left_type);
                    let right_string = Self::type_to_string(compiler, right_type);
                    let result_string = result.to_string();
                    Self::report(
                        compiler,
                        n,
                        &DETERMINISTIC_TEST,
                        &[&left_string, &right_string, &result_string],
                    );
                }
                self.ensure_typed_native(compiler, n, BOOLEAN_TYPE);
            }
            Token::LT | Token::LE | Token::GT | Token::GE => {
                let left_side = n.get_first_child(t).unwrap();
                let right_side = n.get_last_child(t).unwrap();
                let compiler = t.get_compiler();
                left_type = self.get_js_type(compiler, left_side);
                right_type = self.get_js_type(compiler, right_side);
                let (right_unknown, left_unknown, right_numeric, left_numeric) = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    (
                        right_type.is_unknown_type(reg, ast),
                        left_type.is_unknown_type(reg, ast),
                        right_type.is_big_int_or_number(reg, ast),
                        left_type.is_big_int_or_number(reg, ast),
                    )
                };
                if right_unknown {
                    // validate comparable left
                    self.validator.expect_unknown_or_comparable(
                        compiler,
                        left_side,
                        left_type,
                        "left side of comparison",
                    );
                } else if left_unknown {
                    // validate comparable right
                    self.validator.expect_unknown_or_comparable(
                        compiler,
                        right_side,
                        right_type,
                        "right side of comparison",
                    );
                } else if right_numeric {
                    // validate left operand for numeric comparison
                    self.validator.expect_big_int_or_number(
                        compiler,
                        left_side,
                        left_type,
                        "left side of numeric comparison",
                    );
                } else if left_numeric {
                    // validate right operand for numeric comparison
                    self.validator.expect_big_int_or_number(
                        compiler,
                        right_side,
                        right_type,
                        "right side of numeric comparison",
                    );
                } else {
                    let error_msg = "expected matching types in comparison";
                    self.validator.expect_matching_types_strict(
                        compiler, n, left_type, right_type, error_msg,
                    );
                    let both_number_context = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        left_type.matches_number_context(reg, ast)
                            && right_type.matches_number_context(reg, ast)
                    };
                    if !both_number_context {
                        // Whether the comparison is numeric will be determined at runtime
                        // each time the expression is evaluated. Regardless, both operands
                        // should match a string context.
                        let mut message = "left side of comparison";
                        self.validator
                            .expect_string(compiler, left_side, left_type, message);
                        let string_type = self.get_native_type(compiler, STRING_TYPE);
                        self.validator.expect_not_null_or_undefined(
                            t,
                            left_side,
                            left_type,
                            message,
                            string_type,
                        );
                        message = "right side of comparison";
                        let compiler = t.get_compiler();
                        self.validator
                            .expect_string(compiler, right_side, right_type, message);
                        let string_type = self.get_native_type(compiler, STRING_TYPE);
                        self.validator.expect_not_null_or_undefined(
                            t,
                            right_side,
                            right_type,
                            message,
                            string_type,
                        );
                    }
                }
                self.ensure_typed_native(t.get_compiler(), n, BOOLEAN_TYPE);
            }
            Token::IN => {
                let compiler = t.get_compiler();
                left = n.get_first_child(compiler).unwrap();
                right = n.get_last_child(compiler).unwrap();
                right_type = self.get_js_type(compiler, right);
                let left_js_type = self.get_js_type(compiler, left);
                self.validator.expect_string_or_symbol(
                    compiler,
                    left,
                    left_js_type,
                    "left side of 'in'",
                );
                self.validator
                    .expect_object(compiler, n, right_type, "'in' requires an object");
                let is_struct = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    right_type.is_struct(reg, ast)
                };
                if is_struct {
                    Self::report(compiler, right, &IN_USED_WITH_STRUCT, &[]);
                }
                self.ensure_typed_native(compiler, n, BOOLEAN_TYPE);
            }
            Token::INSTANCEOF => {
                let compiler = t.get_compiler();
                left = n.get_first_child(compiler).unwrap();
                right = n.get_last_child(compiler).unwrap();
                let right_js_type = self.get_js_type(compiler, right);
                right_type = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    right_js_type.restrict_by_not_null_or_undefined(reg, ast)
                };
                let left_js_type = self.get_js_type(compiler, left);
                self.validator.expect_any_object(
                    compiler,
                    left,
                    left_js_type,
                    "deterministic instanceof yields false",
                );
                self.validator.expect_actual_object(
                    compiler,
                    right,
                    right_type,
                    "instanceof requires an object",
                );
                self.ensure_typed_native(compiler, n, BOOLEAN_TYPE);
            }
            Token::ASSIGN | Token::ASSIGN_OR | Token::ASSIGN_AND | Token::ASSIGN_COALESCE => {
                self.visit_assign(t, n);
                typeable = false;
            }
            Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_SUB
            | Token::ASSIGN_ADD
            | Token::ASSIGN_MUL
            | Token::ASSIGN_EXPONENT => {
                let compiler = t.get_compiler();
                let first = n.get_first_child(compiler).unwrap();
                self.check_prop_creation(compiler, first);
                let op = n.get_token(compiler);
                self.visit_binary_operator(compiler, op, n);
            }
            Token::LSH
            | Token::RSH
            | Token::URSH
            | Token::DIV
            | Token::MOD
            | Token::BITOR
            | Token::BITXOR
            | Token::BITAND
            | Token::SUB
            | Token::ADD
            | Token::MUL
            | Token::EXPONENT => {
                let compiler = t.get_compiler();
                let op = n.get_token(compiler);
                self.visit_binary_operator(compiler, op, n);
            }
            Token::TRUE | Token::FALSE | Token::NOT | Token::DELPROP => {
                self.ensure_typed_native(t.get_compiler(), n, BOOLEAN_TYPE)
            }
            Token::CASE => {
                let compiler = t.get_compiler();
                let switch_condition = parent.unwrap().get_previous(compiler).unwrap();
                let switch_condition_type = self.get_js_type(compiler, switch_condition);
                let first = n.get_first_child(compiler).unwrap();
                let case_type = self.get_js_type(compiler, first);
                self.validator.expect_switch_matches_case(
                    compiler,
                    n,
                    switch_condition_type,
                    case_type,
                );
                typeable = false;
            }
            Token::WITH => {
                let compiler = t.get_compiler();
                let child = n.get_first_child(compiler).unwrap();
                child_type = self.get_js_type(compiler, child);
                self.validator.expect_object(
                    compiler,
                    child,
                    child_type,
                    "with requires an object",
                );
                typeable = false;
            }
            Token::FUNCTION => self.visit_function(t.get_compiler(), n),
            Token::CLASS => self.visit_class(t.get_compiler(), n),
            Token::MODULE_BODY => self.visit_module_body(t, n),
            // These nodes have no interesting type behavior.
            // These nodes require data flow analysis.
            Token::PARAM_LIST
            | Token::STRING_KEY
            | Token::MEMBER_FUNCTION_DEF
            | Token::COMPUTED_PROP
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_FIELD_DEF
            | Token::LABEL
            | Token::LABEL_NAME
            | Token::SWITCH
            | Token::SWITCH_BODY
            | Token::BREAK
            | Token::CATCH
            | Token::TRY
            | Token::SCRIPT
            | Token::EXPORT
            | Token::EXPORT_SPEC
            | Token::EXPORT_SPECS
            | Token::IMPORT
            | Token::IMPORT_SPEC
            | Token::IMPORT_SPECS
            | Token::IMPORT_STAR
            | Token::EXPR_RESULT
            | Token::BLOCK
            | Token::ROOT
            | Token::EMPTY
            | Token::DEFAULT_CASE
            | Token::CONTINUE
            | Token::DEBUGGER
            | Token::THROW
            | Token::DO
            | Token::IF
            | Token::WHILE
            | Token::FOR
            | Token::TEMPLATELIT_SUB
            | Token::ITER_REST
            | Token::OBJECT_REST
            | Token::DESTRUCTURING_LHS => typeable = false,
            Token::DYNAMIC_IMPORT => self.visit_dynamic_import(t, n),
            Token::ARRAY_PATTERN => {
                let compiler = t.get_compiler();
                self.ensure_typed(compiler, n);
                let ty = self.get_js_type(compiler, n);
                self.validator.expect_autoboxes_to_iterable(
                    compiler,
                    n,
                    ty,
                    "array pattern destructuring requires an Iterable",
                );
            }
            Token::OBJECT_PATTERN => self.visit_object_pattern(t.get_compiler(), n),
            Token::DEFAULT_VALUE => {
                let first = n.get_first_child(t).unwrap();
                let second = n.get_second_child(t).unwrap();
                let second_type = self.get_js_type(t.get_compiler(), second);
                self.check_can_assign_to_with_scope(
                    t,
                    n,
                    first,
                    second_type,
                    /* info= */ None,
                    "default value has wrong type",
                );

                // Every other usage of a destructuring pattern is checked while visiting the
                // pattern, but default values are different because they are a conditional
                // assignment and the pattern is not given the default value's type
                let compiler = t.get_compiler();
                let lhs = n.get_first_child(compiler).unwrap();
                let rhs = n.get_second_child(compiler).unwrap();
                if lhs.is_array_pattern(compiler) {
                    let rhs_type = self.get_js_type(compiler, rhs);
                    self.validator.expect_autoboxes_to_iterable(
                        compiler,
                        rhs,
                        rhs_type,
                        "array pattern destructuring requires an Iterable",
                    );
                } else if lhs.is_object_pattern(compiler) {
                    // Verify that the value is not null/undefined, since those can't be
                    // destructured.
                    let rhs_type = self.get_js_type(compiler, rhs);
                    self.validator.expect_object(
                        compiler,
                        rhs,
                        rhs_type,
                        "cannot destructure a 'null' or 'undefined' default value",
                    );
                }

                typeable = false;
            }
            Token::CLASS_MEMBERS => {
                let compiler = t.get_compiler();
                let parent = parent.unwrap();
                let parent_fn_type = {
                    let parent_type = parent.get_jstype(compiler).expect("NullPointerException");
                    parent_type
                        .to_maybe_function_type(compiler.get_type_registry())
                        .expect("NullPointerException")
                };
                let typ =
                    FunctionType::get_instance_type(parent_fn_type, compiler.get_type_registry())
                        .expect("NullPointerException");
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    let owner = n.get_parent(compiler).unwrap();
                    self.visit_object_or_class_literal_key(compiler, c, owner, typ);
                    if c.is_setter_def(compiler) || c.is_getter_def(compiler) {
                        self.check_getter_or_setter_type(compiler, c, parent_fn_type);
                    }
                    child = c.get_next(compiler);
                }
                typeable = false;
            }
            Token::FOR_IN => {
                let compiler = t.get_compiler();
                let obj = n.get_second_child(compiler).unwrap();
                let obj_type = self.get_js_type(compiler, obj);
                let is_struct = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    obj_type.is_struct(reg, ast)
                };
                if is_struct {
                    Self::report(compiler, obj, &IN_USED_WITH_STRUCT, &[]);
                }
                typeable = false;
            }
            Token::FOR_OF | Token::FOR_AWAIT_OF => {
                let compiler = t.get_compiler();
                let second = n.get_second_child(compiler).unwrap();
                self.ensure_typed(compiler, second);
                typeable = false;
                // These nodes are typed during the type inference.
            }
            Token::SUPER
            | Token::NEW_TARGET
            | Token::IMPORT_META
            | Token::AWAIT
            | Token::AND
            | Token::HOOK
            | Token::OR
            | Token::COALESCE => self.ensure_typed(t.get_compiler(), n),
            Token::OBJECTLIT => {
                let compiler = t.get_compiler();
                // If this is an enum, then give that type to the objectlit as well.
                let parent_type = parent.unwrap().get_jstype(compiler);
                // Rust-only: Java's `instanceof EnumType` (the exact class, not a proxy to one).
                if parent_type.is_some_and(|pt| {
                    pt.to_maybe_enum_type(compiler.get_type_registry()) == Some(pt)
                }) {
                    self.ensure_typed_with_type(compiler, n, parent_type);
                } else {
                    self.ensure_typed(compiler, n);
                }
                let typ = self.get_js_type(compiler, n);
                let mut key = n.get_first_child(compiler);
                while let Some(k) = key {
                    self.visit_object_or_class_literal_key(compiler, k, n, typ);
                    key = k.get_next(compiler);
                }
            }
            Token::ITER_SPREAD | Token::OBJECT_SPREAD => {
                self.check_spread(t.get_compiler(), n);
                typeable = false;
            }
            _ => {
                let compiler = t.get_compiler();
                let token = n.get_token(compiler).to_string();
                Self::report(compiler, n, &UNEXPECTED_TOKEN, &[&token]);
                self.ensure_typed(compiler, n);
            }
        }

        // Visit the body of blockless arrow functions
        if NodeUtil::is_blockless_arrow_function_result(t, n) {
            self.visit_implicit_return_expression(t, n);
        }

        // Visit the loop initializer of a for-of loop
        // We do this check here, instead of when visiting FOR_OF, in order to get the correct
        // TypedScope.
        let n_parent = n.get_parent(t).unwrap();
        if (n_parent.is_for_of(t) || n_parent.is_for_await_of(t))
            && n_parent.get_first_child(t) == Some(n)
        {
            self.check_for_of_types(t, n_parent);
        }

        // Don't count externs since the user's code may not even use that part.
        typeable = typeable && !self.in_externs;

        // Record typing logs and metrics.
        if typeable {
            self.debug_type_logger
                .as_mut()
                .unwrap()
                .maybe_log_type_of_node(t.get_compiler(), n);
            self.do_percent_typed_accounting(t.get_compiler(), n);
        }

        self.check_jsdoc_info_contains_object_with_bad_key(t.get_compiler(), n);

        // If this is the node which started logging types, stop logging.
        self.debug_type_logger
            .as_mut()
            .unwrap()
            .stop_logging_if_this_is_where_we_started(n);
    }

    // port: TypeCheck#visitUnaryPlus
    fn visit_unary_plus(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let n_type = self.get_js_type(compiler, n);
        if n_type.is_no_type(compiler.get_type_registry()) {
            let op = NodeUtil::op_to_str(n.get_token(compiler)).unwrap_or("null");
            let first = n.get_first_child(compiler).unwrap();
            let child_type = self.get_js_type(compiler, first);
            let child_string = Self::type_to_string(compiler, child_type);
            Self::report(compiler, n, &UNARY_OPERATION, &[op, &child_string]);
        } else {
            self.ensure_typed_native(compiler, n, NUMBER_TYPE);
        }
    }

    // port: TypeCheck#visitUnaryMinus
    fn visit_unary_minus(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let operator_type = self.get_js_type(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        let child_type = self.get_js_type(compiler, first);
        let is_number = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            operator_type.is_number(reg, ast)
        };
        if is_number {
            // This condition is meant to catch any old cases (where bigint isn't involved)
            self.validator
                .expect_number(compiler, n, child_type, "sign operator");
            self.ensure_typed_native(compiler, n, NUMBER_TYPE);
        } else {
            self.validator.expect_big_int_or_number(
                compiler,
                n,
                child_type,
                "unary minus operator",
            );
        }
    }

    // port: TypeCheck#visitBitwiseNOT
    fn visit_bitwise_not(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let operator_type = self.get_js_type(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        let child_type = self.get_js_type(compiler, first);
        let is_number = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            operator_type.is_number(reg, ast)
        };
        if is_number {
            // This condition is meant to catch any old cases (where bigint isn't involved)
            let matches_number_context = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                child_type.matches_number_context(reg, ast)
            };
            if !matches_number_context {
                let op = NodeUtil::op_to_str(n.get_token(compiler)).unwrap_or("null");
                let child_string = Self::type_to_string(compiler, child_type);
                Self::report(compiler, n, &BIT_OPERATION, &[op, &child_string]);
            } else {
                self.validator
                    .expect_number_strict(compiler, n, child_type, "bitwise NOT");
            }
            self.ensure_typed_native(compiler, n, NUMBER_TYPE);
        } else {
            self.validator
                .expect_big_int_or_number(compiler, n, child_type, "bitwise NOT");
        }
    }

    // port: TypeCheck#checkSpread
    fn check_spread(&mut self, compiler: &mut AbstractCompiler, spread_node: NodeId) {
        let target = spread_node.get_only_child(compiler);
        self.ensure_typed(compiler, target);
        let target_type = self.get_js_type(compiler, target);

        let spread_parent = spread_node.get_parent(compiler).unwrap();
        match spread_parent.get_token(compiler) {
            Token::OBJECTLIT => {
                // Case: `var x = {A: a, B: b, ...obj}`.
                // Nothing to check about object spread.
            }
            Token::ARRAYLIT | Token::CALL | Token::OPTCHAIN_CALL | Token::NEW => {
                // Case: `var x = [a, b, ...itr]`
                // Case: `var x = fn(a, b, ...itr)`
                // Case: `var x = fn?.(a, b, ...itr)`
                self.validator.expect_autoboxes_to_iterable(
                    compiler,
                    target,
                    target_type,
                    "Spread operator only applies to Iterable types",
                );
            }
            _ => panic!(
                "Unexpected parent of SPREAD: {}",
                spread_parent.to_string_tree(compiler)
            ),
        }
    }

    // port: TypeCheck#checkTypeofString
    fn check_typeof_string(&mut self, compiler: &mut AbstractCompiler, n: NodeId, s: &str) {
        if !(s == "number"
            || s == "string"
            || s == "boolean"
            || s == "undefined"
            || s == "function"
            || s == "object"
            || s == "symbol"
            || s == "unknown"
            || s == "bigint")
        {
            self.validator.expect_valid_typeof_name(compiler, n, s);
        }
    }

    /// Counts the given node in the typed statistics.
    // port: TypeCheck#doPercentTypedAccounting
    fn do_percent_typed_accounting(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let type_ = n.get_jstype(compiler);
        match type_ {
            None => self.null_count += 1,
            Some(type_)
                if {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    type_.is_unknown_type(reg, ast)
                } =>
            {
                if self.report_unknown_types
                    && !n.get_parent(compiler).unwrap().is_expr_result(compiler)
                {
                    let error = JSError::make(compiler, n, &UNKNOWN_EXPR_TYPE, &[]);
                    compiler.report(error);
                }
                self.unknown_count += 1;
            }
            Some(_) => self.typed_count += 1,
        }
    }

    /// Verifies that a user did not give a getter and setter different types, as the type system
    /// will arbitrarily take the first of the types if different.
    // port: TypeCheck#checkGetterOrSetterType
    fn check_getter_or_setter_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        child: NodeId,
        class_type: TypeId,
    ) {
        let property_name = child.get_string(compiler);

        let method_type = child
            .get_last_child(compiler)
            .unwrap()
            .get_jstype(compiler)
            .expect("NullPointerException")
            .to_maybe_function_type(compiler.get_type_registry())
            .expect("NullPointerException");
        let property_type = if child.is_getter_def(compiler) {
            self.determine_getter_type(compiler, method_type)
        } else {
            let parameters = method_type.get_parameters(compiler.get_type_registry());
            check_argument!(parameters.len() == 1, "expected one element");
            parameters[0].get_jstype()
        };
        let is_static = child.is_static_member(compiler);
        let official_property_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if is_static {
                class_type.get_property_type(reg, ast, property_name.clone())
            } else {
                let prototype = class_type.get_prototype(reg, ast);
                prototype.get_property_type(reg, ast, property_name.clone())
            }
        };
        let equal = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            property_type.equals(reg, ast, official_property_type)
        };
        if !equal {
            // TODO(b/116797078): make this not an error - instead, store the getter and setter
            // types separately
            let is_getter = child.is_getter_def(compiler);
            let property_string = Self::type_to_string(compiler, property_type);
            let official_string = Self::type_to_string(compiler, official_property_type);
            let property_name = property_name.to_string_lossy();
            Self::report(
                compiler,
                child,
                &CONFLICTING_GETTER_SETTER_TYPE,
                &[
                    &property_name,
                    if is_getter {
                        &property_string
                    } else {
                        &official_string
                    },
                    if is_getter {
                        &official_string
                    } else {
                        &property_string
                    },
                ],
            );
        }
    }

    // port: TypeCheck#determineGetterType
    fn determine_getter_type(
        &self,
        compiler: &mut AbstractCompiler,
        method_type: TypeId,
    ) -> TypeId {
        // TODO(sdh): consider only falling back on unknown if the function body is empty?  But
        // we need to not report a conflicting type error if there's different unknowns.
        let reg = compiler.get_type_registry();
        if !method_type.is_return_type_inferred(reg) {
            method_type.get_return_type(reg)
        } else {
            reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)
        }
    }
    /// Visits an assignment `lvalue = rvalue`. If the `lvalue` is a prototype modification, we
    /// change the schema of the object type it is referring to.
    ///
    /// `assign`: the assign node (`assign.isAssign()` is an implicit invariant)
    // port: TypeCheck#visitAssign
    fn visit_assign(&mut self, t: &mut NodeTraversal<'_>, assign: NodeId) {
        let info = assign.get_jsdoc_info(t);
        let lvalue = assign.get_first_child(t).unwrap();
        let rvalue = assign.get_last_child(t).unwrap();

        let right_type = self.get_js_type(t.get_compiler(), rvalue);
        self.check_can_assign_to_with_scope(
            t,
            assign,
            lvalue,
            right_type,
            info.as_deref(),
            "assignment",
        );
        self.ensure_typed_with_type(t.get_compiler(), assign, Some(right_type));
    }

    /// Checks that we can assign the given right type to the given lvalue or destructuring
    /// pattern.
    ///
    /// See `check_can_assign_to_name_getprop_or_getelem` for more details.
    ///
    /// `node_to_warn`: A node to report type mismatch warnings on. `lvalue`: The lvalue to which
    /// we're assigning or a destructuring pattern. `right_type`: The type we're assigning to the
    /// lvalue. `msg`: A message to report along with any type mismatch warnings.
    // port: TypeCheck#checkCanAssignToWithScope
    fn check_can_assign_to_with_scope(
        &mut self,
        t: &mut NodeTraversal<'_>,
        node_to_warn: NodeId,
        lvalue: NodeId,
        right_type: TypeId,
        info: Option<&JSDocInfo>,
        msg: &str,
    ) {
        if lvalue.is_destructuring_pattern(t) {
            self.check_destructuring_assignment(t, node_to_warn, lvalue, right_type, msg);
        } else {
            self.check_can_assign_to_name_getprop_or_getelem(
                t,
                node_to_warn,
                lvalue,
                right_type,
                info,
                msg,
            );
        }
    }

    /// Recursively checks that an assignment to a destructuring pattern is valid for all the
    /// lvalues contained in the pattern (including in nested patterns).
    // port: TypeCheck#checkDestructuringAssignment
    fn check_destructuring_assignment(
        &mut self,
        t: &mut NodeTraversal<'_>,
        node_to_warn: NodeId,
        pattern: NodeId,
        right_type: TypeId,
        msg: &str,
    ) {
        let targets = DestructuredTarget::create_all_non_empty_targets_in_pattern(
            t.get_compiler(),
            Some(right_type),
            pattern,
        );
        for target in targets {
            // TODO(b/77597706): this is not very efficient because it re-infers the types below,
            // which we already did once in TypeInference. don't repeat the work.
            let inferred = {
                let (reg, ast) = t.get_compiler().get_type_registry_and_ast();
                target.infer_type(reg, ast)
            };
            self.check_can_assign_to_with_scope(
                t,
                node_to_warn,
                target.get_node(),
                inferred,
                /* info= */ None,
                msg,
            );
        }
    }

    /// Checks that we can assign the given right type to the given lvalue.
    ///
    /// If the lvalue is a qualified name, and has a declared type in the given scope, uses the
    /// declared type of the qualified name instead of the type on the node.
    ///
    /// `node_to_warn`: A node to report type mismatch warnings on. `lvalue`: The lvalue to which
    /// we're assigning - a NAME, GETELEM, or GETPROP. `right_type`: The type we're assigning to
    /// the lvalue. `msg`: A message to report along with any type mismatch warnings.
    // port: TypeCheck#checkCanAssignToNameGetpropOrGetelem
    fn check_can_assign_to_name_getprop_or_getelem(
        &mut self,
        t: &mut NodeTraversal<'_>,
        node_to_warn: NodeId,
        lvalue: NodeId,
        right_type: TypeId,
        info: Option<&JSDocInfo>,
        msg: &str,
    ) {
        check_argument!(
            lvalue.is_name(t)
                || lvalue.is_get_prop(t)
                || lvalue.is_get_elem(t)
                || lvalue.is_cast(t),
            "%s",
            lvalue.to_string(t)
        );

        // Ensure our LHS is not readonly.
        self.check_not_readonly_property_assignment(t.get_compiler(), lvalue);

        if lvalue.is_get_prop(t) {
            let object = lvalue.get_first_child(t).unwrap();
            let object_js_type = self.get_js_type(t.get_compiler(), object);
            let pname = lvalue.get_string(t);

            // the first name in this getprop refers to an interface
            // we perform checks in addition to the ones below
            if object.is_get_prop(t) {
                let compiler = t.get_compiler();
                let object_first = object.get_first_child(compiler).unwrap();
                let js_type = self.get_js_type(compiler, object_first);
                if js_type.is_interface(compiler.get_type_registry())
                    && object.get_string_ref(compiler) == "prototype"
                {
                    self.visit_interface_property_assignment(compiler, object, lvalue);
                }
            }

            self.check_enum_alias(t, info, right_type, node_to_warn);
            self.check_prop_creation(t.get_compiler(), lvalue);

            let compiler = t.get_compiler();
            // Prototype assignments are special, because they actually affect
            // the definition of a class. These are mostly validated
            // during TypedScopeCreator, and we only look for the "dumb" cases here.
            // object.prototype = ...;
            if pname == "prototype" {
                self.validator.expect_can_assign_to_prototype(
                    compiler,
                    object_js_type,
                    node_to_warn,
                    right_type,
                );
                return;
            }

            // The generic checks for 'object.property' when 'object' is known,
            // and 'property' is declared on it.
            // object.property = ...;
            let object_cast_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                object_js_type
                    .restrict_by_not_null_or_undefined(reg, ast)
                    .to_maybe_object_type(reg)
            };
            let expected_property_type = self.get_property_type_if_declared(
                compiler,
                object_cast_type,
                StringKey::new(pname.clone()).into(),
            );

            self.check_property_inheritance_on_assignment(
                compiler,
                node_to_warn,
                object,
                StringKey::new(pname.clone()).into(),
                info,
                expected_property_type,
            );

            // If we successfully found a non-unknown declared type, validate the assignment and
            // don't do any further checks.
            let is_unknown = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                expected_property_type.is_unknown_type(reg, ast)
            };
            if !is_unknown {
                // Note: if the property has @implicitCast at its declaration, we don't check any
                // assignments to it.
                if !Self::property_is_implicit_cast(compiler, object_cast_type, &pname) {
                    self.validator.expect_can_assign_to_property_of(
                        compiler,
                        node_to_warn,
                        right_type,
                        expected_property_type,
                        object,
                        &pname.to_string_lossy(),
                    );
                }
                return;
            }
        } else if lvalue.is_get_elem(t)
            && lvalue
                .get_second_child(t)
                .unwrap()
                .get_jstype(t)
                .expect("NullPointerException")
                .is_known_symbol_value_type(t.get_compiler().get_type_registry())
        {
            let compiler = t.get_compiler();
            let object = lvalue.get_first_child(compiler).unwrap();
            let object_js_type = self.get_js_type(compiler, object);
            let object_cast_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                object_js_type
                    .restrict_by_not_null_or_undefined(reg, ast)
                    .to_maybe_object_type(reg)
            };
            let property = lvalue
                .get_last_child(compiler)
                .unwrap()
                .get_jstype(compiler)
                .expect("NullPointerException")
                .to_maybe_known_symbol_type(compiler.get_type_registry())
                .expect("NullPointerException");
            let expected_property_type = self.get_property_type_if_declared(
                compiler,
                object_cast_type,
                SymbolKey::new(property).into(),
            );
            self.check_property_inheritance_on_assignment(
                compiler,
                node_to_warn,
                object,
                SymbolKey::new(property).into(),
                info,
                expected_property_type,
            );
        }

        // Check qualified name sets to 'object' and 'object.property'.
        // This can sometimes handle cases when the type of 'object' is not known.
        // e.g.,
        // var obj = createUnknownType();
        // /** @type {number} */ obj.foo = true;
        let mut left_type = self.get_js_type(t.get_compiler(), lvalue);
        if lvalue.is_qualified_name(t) {
            // variable with inferred type case
            let scope = t.get_typed_scope();
            let compiler = t.get_compiler();
            let q_name = lvalue.get_qualified_name(compiler).unwrap();
            let var = scope.get_var(compiler, q_name);
            if let Some(var) = var {
                if var.is_type_inferred(compiler) {
                    return;
                }

                if NodeUtil::get_root_of_qualified_name(compiler, lvalue).is_this(compiler)
                    && scope != var.get_scope(compiler)
                {
                    // Don't look at "this.foo" variables from other scopes.
                    return;
                }

                if let Some(var_type) = var.get_type(compiler) {
                    left_type = var_type;
                }
            }
        } // Fall through case for arbitrary LHS and arbitrary RHS.

        self.validator.expect_can_assign_to(
            t.get_compiler(),
            node_to_warn,
            right_type,
            left_type,
            msg,
        );
    }

    // port: TypeCheck#checkPropCreation
    fn check_prop_creation(&mut self, compiler: &mut AbstractCompiler, lvalue: NodeId) {
        if lvalue.is_get_prop(compiler) {
            let first = lvalue.get_first_child(compiler).unwrap();
            let obj_type = self.get_js_type(compiler, first);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if !obj_type.is_empty_type(reg) && !obj_type.is_unknown_type(reg, ast) {
                let prop_name = lvalue.get_string(ast);
                let kind = reg.can_property_be_defined(ast, obj_type, prop_name);
                if kind != PropDefinitionKind::KNOWN {
                    if obj_type.is_struct(reg, ast) {
                        if obj_type
                            .restrict_by_not_null_or_undefined(reg, ast)
                            .is_union_type(reg)
                        {
                            Self::report(
                                compiler,
                                lvalue,
                                &ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE,
                                &[],
                            );
                        } else {
                            Self::report(compiler, lvalue, &ILLEGAL_PROPERTY_CREATION, &[]);
                        }
                    } else {
                        // null checks are reported elsewhere
                        let null_void = reg.get_native_type(NULL_VOID);
                        if !obj_type.is_no_type(reg)
                            && !obj_type.is_unknown_type(reg, ast)
                            && obj_type.is_subtype_of(reg, ast, null_void)
                        {
                            return;
                        }

                        self.report_missing_property(
                            compiler,
                            Some(first),
                            obj_type,
                            lvalue,
                            kind,
                            true,
                        );
                    }
                }
            }
        }
    }

    // port: TypeCheck#checkPropertyInheritanceOnAssignment
    fn check_property_inheritance_on_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        assign: NodeId,
        object: NodeId,
        property: PropertyKey,
        info: Option<&JSDocInfo>,
        property_type: TypeId,
    ) {
        // Inheritance checks for prototype properties.
        //
        // TODO(nicksantos): This isn't the right place to do this check. We
        // really want to do this when we're looking at the constructor.
        // We'd find all its properties and make sure they followed inheritance
        // rules, like we currently do for @implements to make sure
        // all the methods are implemented.
        //
        // As-is, this misses many other ways to override a property.

        if object.is_get_prop(compiler) && object.get_string_ref(compiler) == "prototype" {
            // ASSIGN = assign
            //   GETPROP
            //     GETPROP = object
            //       ? = preObject
            //       STRING = "prototype"
            //     STRING = property

            let pre_object = object.get_first_child(compiler).unwrap();
            let ctor_type = self
                .get_js_type(compiler, pre_object)
                .to_maybe_function_type(compiler.get_type_registry());
            let Some(ctor_type) = ctor_type else {
                return;
            };
            if !ctor_type.has_instance_type(compiler.get_type_registry()) {
                return;
            }

            self.check_declared_property_against_nominal_inheritance(
                compiler,
                assign,
                ctor_type,
                &property,
                info,
                property_type,
            );
            self.check_abstract_method_in_concrete_class(compiler, assign, ctor_type, info);
        } else {
            // ASSIGN = assign
            //   GETPROP
            //     ? = object
            //     STRING = property
            // or
            //  ASSIGN = assign
            //    GETELEM
            //      ? = object
            //      ? = property

            // We only care about checking a static property assignment.
            let ctor_type = self
                .get_js_type(compiler, object)
                .to_maybe_function_type(compiler.get_type_registry());
            let Some(ctor_type) = ctor_type else {
                return;
            };
            if !ctor_type.has_instance_type(compiler.get_type_registry()) {
                return;
            }

            self.check_declared_property_against_prototypal_inheritance(
                compiler,
                assign,
                ctor_type,
                &property,
                info,
                property_type,
            );
        }
    }

    // port: TypeCheck#checkPropertyInheritanceOnPrototypeLitKey
    fn check_property_inheritance_on_prototype_lit_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: NodeId,
        property_name: &PropertyKey,
        type_: TypeId,
    ) {
        // Inheritance checks for prototype objlit properties.
        //
        // TODO(nicksantos): This isn't the right place to do this check. We
        // really want to do this when we're looking at the constructor.
        // We'd find all its properties and make sure they followed inheritance
        // rules, like we currently do for @implements to make sure
        // all the methods are implemented.
        //
        // As-is, this misses many other ways to override a property.
        //
        // object.prototype = { key: function() {} };
        let owner_function = type_.get_owner_function(compiler.get_type_registry());
        self.check_property_inheritance(compiler, key, property_name, owner_function, type_);
    }

    // port: TypeCheck#checkPropertyInheritanceOnClassMember
    fn check_property_inheritance_on_class_member(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: NodeId,
        property_name: &PropertyKey,
        ctor_type: TypeId,
    ) {
        if key.is_static_member(compiler) {
            let info = key.get_jsdoc_info(compiler);
            let property_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                ctor_type.get_property_type(reg, ast, property_name.clone())
            };
            self.check_declared_property_against_prototypal_inheritance(
                compiler,
                key,
                ctor_type,
                property_name,
                info.as_deref(),
                property_type,
            );
        } else {
            let instance_type = ctor_type
                .get_instance_type(compiler.get_type_registry())
                .expect("NullPointerException");
            self.check_property_inheritance(
                compiler,
                key,
                property_name,
                Some(ctor_type),
                instance_type,
            );
        }
    }

    // port: TypeCheck#checkPropertyInheritance
    fn check_property_inheritance(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: NodeId,
        property_name: &PropertyKey,
        ctor_type: Option<TypeId>,
        type_: TypeId,
    ) {
        let Some(ctor_type) = ctor_type else {
            return;
        };
        if !ctor_type.has_instance_type(compiler.get_type_registry()) {
            return;
        }

        let first = key.get_first_child(compiler).expect("NullPointerException");
        let info = key.get_jsdoc_info(compiler);
        let property_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            type_.get_property_type(reg, ast, property_name.clone())
        };
        self.check_declared_property_against_nominal_inheritance(
            compiler,
            first,
            ctor_type,
            property_name,
            info.as_deref(),
            property_type,
        );
        self.check_abstract_method_in_concrete_class(compiler, key, ctor_type, info.as_deref());
    }

    /// Validates all keys in an object pattern
    ///
    /// Validating the types assigned to any lhs nodes in the pattern is done at the
    /// ASSIGN/VAR/PARAM_LIST/etc. node
    // port: TypeCheck#visitObjectPattern
    fn visit_object_pattern(&mut self, compiler: &mut AbstractCompiler, pattern: NodeId) {
        let pattern_type = self.get_js_type(compiler, pattern);
        self.validator.expect_object(
            compiler,
            pattern,
            pattern_type,
            "cannot destructure 'null' or 'undefined'",
        );
        let mut child = pattern.get_first_child(compiler);
        while let Some(c) = child {
            let target = DestructuredTarget::create_target(compiler, Some(pattern_type), c);

            if target.has_computed_property(compiler) {
                let computed_property = target.get_computed_property(compiler).unwrap();
                let computed_first = computed_property.get_first_child(compiler).unwrap();
                let index_type = self.get_js_type(compiler, computed_first);
                self.validator.expect_index_match(
                    compiler,
                    computed_property,
                    pattern_type,
                    index_type,
                );
            } else if target.has_string_key(compiler) {
                let string_key = target.get_string_key(compiler).unwrap();
                if !string_key.is_quoted_string_key(compiler) {
                    let is_dict = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        pattern_type.is_dict(reg, ast)
                    };
                    if is_dict {
                        Self::report(
                            compiler,
                            string_key,
                            &type_validator::ILLEGAL_PROPERTY_ACCESS,
                            &["unquoted", "dict"],
                        );
                    }
                    // check for missing properties given `const {a} = obj;` but not
                    // `const {'a': a} = obj;`
                    let target_type = self.get_js_type(compiler, target.get_node());
                    self.check_property_access_for_destructuring(
                        compiler,
                        pattern,
                        pattern_type,
                        string_key,
                        target_type,
                    );
                } else {
                    let is_struct = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        pattern_type.is_struct(reg, ast)
                    };
                    if is_struct {
                        // check that we are not accessing a struct with a quoted string
                        Self::report(
                            compiler,
                            string_key,
                            &type_validator::ILLEGAL_PROPERTY_ACCESS,
                            &["quoted", "struct"],
                        );
                    }
                }
            }
            child = c.get_next(compiler);
        }
        self.ensure_typed(compiler, pattern);
    }

    /// Visits an object literal field definition `key : value`, or a class member definition
    /// `key() { ... }` If the `lvalue` is a prototype modification, we change the schema of the
    /// object type it is referring to.
    ///
    /// `key`: the ASSIGN, STRING_KEY, MEMBER_FUNCTION_DEF, SPREAD, COMPUTED_PROPERTY,
    /// MEMBER_FIELD_DEF, or COMPUTED_FIELD_DEF node. `owner`: the parent node, either OBJECTLIT
    /// or CLASS_MEMBERS. `owner_type`: the instance type of the enclosing object/class.
    // port: TypeCheck#visitObjectOrClassLiteralKey
    fn visit_object_or_class_literal_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: NodeId,
        owner: NodeId,
        owner_type: TypeId,
    ) {
        // Do not validate object lit value types in externs. We don't really care,
        // and it makes it easier to generate externs.
        if owner.is_from_externs(compiler) {
            self.ensure_typed(compiler, key);
            return;
        }

        // Validate computed properties similarly to how we validate GETELEMs.
        if key.is_computed_prop(compiler) || key.is_computed_field_def(compiler) {
            let key_first = key.get_first_child(compiler).unwrap();
            let key_type = self.get_js_type(compiler, key_first);
            self.validator
                .expect_index_match(compiler, key, owner_type, key_type);
            if key_type.is_known_symbol_value_type(compiler.get_type_registry()) {
                let known_symbol = key_type
                    .to_maybe_known_symbol_type(compiler.get_type_registry())
                    .unwrap();
                if owner.is_class(compiler) {
                    let class_constructor_type = {
                        let owner_js_type =
                            owner.get_jstype(compiler).expect("NullPointerException");
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        owner_js_type.assert_function_type(reg, ast)
                    };
                    self.check_property_inheritance_on_class_member(
                        compiler,
                        key,
                        &SymbolKey::new(known_symbol).into(),
                        class_constructor_type,
                    );
                } else {
                    check_state!(
                        owner.is_object_lit(compiler),
                        "Unexpected owner %s",
                        owner.to_string(compiler)
                    );
                    // Check if this property has an expected declared type on the owner.
                    let object_cast_type = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        owner_type
                            .restrict_by_not_null_or_undefined(reg, ast)
                            .to_maybe_object_type(reg)
                    };
                    let expected_property_type = self.get_property_type_if_declared(
                        compiler,
                        object_cast_type,
                        SymbolKey::new(known_symbol).into(),
                    );
                    let key_second = key.get_second_child(compiler).unwrap();
                    let value_type = self.get_js_type(compiler, key_second);
                    let display_name = known_symbol
                        .get_display_name(compiler.get_type_registry())
                        .unwrap_or_else(|| "null".to_owned());
                    self.validator.expect_can_assign_to_property_of(
                        compiler,
                        key,
                        expected_property_type,
                        value_type,
                        owner,
                        &display_name,
                    );
                }
            }
            return;
        }

        if key.is_block(compiler) {
            return;
        }

        if key.is_quoted_string_key(compiler) {
            // NB: this case will never be triggered for member functions, since we store quoted
            // member functions as computed properties. This case does apply to regular string key
            // properties, getters, and setters.
            // See also https://github.com/google/closure-compiler/issues/3071
            let is_struct = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                owner_type.is_struct(reg, ast)
            };
            if is_struct {
                let diagnostic = if owner.is_class(compiler) {
                    &ILLEGAL_CLASS_KEY
                } else {
                    &ILLEGAL_OBJLIT_KEY
                };
                Self::report(compiler, key, diagnostic, &["struct"]);
            }
        } else {
            // we have either a non-quoted string or a member function def
            // Neither is allowed for an @dict type except for "constructor" as a special case.
            let is_dict = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                owner_type.is_dict(reg, ast)
            };
            if is_dict && !NodeUtil::is_es6_constructor_member_function_def(compiler, key) {
                // Object literals annotated as @dict may only have
                // If you annotate a class with @dict, only the constructor can be a non-computed
                // property.
                let diagnostic = if owner.is_class(compiler) {
                    &ILLEGAL_CLASS_KEY
                } else {
                    &ILLEGAL_OBJLIT_KEY
                };
                Self::report(compiler, key, diagnostic, &["dict"]);
            }
        }

        if key.is_spread(compiler) {
            // Rely on type inference to figure out what the key/types this adds to this object.
            return;
        }

        // TODO(johnlenz): Validate get and set function declarations are valid
        // as is the functions can have "extraneous" bits.

        // For getter and setter property definitions the
        // r-value type != the property type.
        let Some(rvalue) = key.get_first_child(compiler) else {
            self.ensure_typed(compiler, key);
            return;
        };
        let rvalue_type = self.get_js_type(compiler, rvalue);
        let mut right_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            Self::get_object_lit_key_type_from_value_type(reg, ast, key, Some(rvalue_type))
        };
        if right_type.is_none() {
            right_type = Some(self.get_native_type(compiler, UNKNOWN_TYPE));
        }
        let right_type = right_type.unwrap();

        // Validate value is assignable to the key type.
        let key_type = self.get_js_type(compiler, key);
        let property_name = NodeUtil::get_object_or_class_lit_key_name(compiler, key);
        let mut allowed_value_type = key_type;
        let reg = compiler.get_type_registry();
        if allowed_value_type.is_enum_element_type(reg) {
            allowed_value_type = allowed_value_type
                .to_maybe_enum_element_type(reg)
                .unwrap()
                .get_primitive_type(reg);
        }

        let valid = self.validator.expect_can_assign_to_property_of(
            compiler,
            key,
            right_type,
            allowed_value_type,
            owner,
            &property_name.to_string_lossy(),
        );
        if valid {
            self.ensure_typed_with_type(compiler, key, Some(right_type));
        } else {
            self.ensure_typed(compiler, key);
        }

        // Validate inheritance for classes and object literals used as prototypes
        let property_key: PropertyKey = StringKey::new(property_name).into();
        if owner.is_class(compiler) {
            let class_constructor_type = {
                let owner_js_type = owner.get_jstype(compiler).expect("NullPointerException");
                let (reg, ast) = compiler.get_type_registry_and_ast();
                owner_js_type.assert_function_type(reg, ast)
            };
            self.check_property_inheritance_on_class_member(
                compiler,
                key,
                &property_key,
                class_constructor_type,
            );
        } else if let Some(owner_object_type) =
            owner_type.to_maybe_object_type(compiler.get_type_registry())
        {
            self.check_property_inheritance_on_prototype_lit_key(
                compiler,
                key,
                &property_key,
                owner_object_type,
            );
        }
    }

    /// Returns true if any type in the chain has an implicitCast annotation for the given
    /// property.
    // port: TypeCheck#propertyIsImplicitCast
    fn property_is_implicit_cast(
        compiler: &mut AbstractCompiler,
        type_: Option<TypeId>,
        prop: &JsString,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let mut type_ = type_;
        while let Some(ty) = type_ {
            let doc_info = ty.get_own_property_jsdoc_info(reg, ast, prop.clone());
            if doc_info.is_some_and(|info| info.is_implicit_cast()) {
                return true;
            }
            type_ = ty.get_implicit_prototype(reg, ast);
        }
        false
    }

    /// Given a `ctor_type`, check that the property (`property_name`), on the corresponding
    /// instance type (`receiverType`), conforms to inheritance rules.
    ///
    /// This method only checks nominal inheritance (via extends and implements declarations).
    /// Compare to `check_declared_property_against_prototypal_inheritance`.
    ///
    /// To be conformant, the `property_name` must
    ///
    /// - Carry the `@override` annotation iff it is an override.
    /// - Be typed as a subtype of the type of `property_name` on all supertypes of
    ///   `receiverType`.
    // port: TypeCheck#checkDeclaredPropertyAgainstNominalInheritance
    fn check_declared_property_against_nominal_inheritance(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        ctor_type: TypeId,
        property_name: &PropertyKey,
        info: Option<&JSDocInfo>,
        property_type: TypeId,
    ) {
        // No need to check special properties; @override is not required for them, nor they are
        // manually typed by the developers.
        if property_name.matches("__proto__") || property_name.matches("constructor") {
            return;
        }

        // Interfaces are checked elsewhere.
        if ctor_type.is_interface(compiler.get_type_registry()) {
            return;
        }

        // If the supertype doesn't resolve correctly, we've warned about this already.
        if Self::has_unknown_or_empty_supertype(compiler, ctor_type) {
            return;
        }

        let mut found_property = false;

        let super_ctor = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            ctor_type.get_super_class_constructor(reg, ast)
        };
        if let Some(super_ctor) = super_ctor {
            let prop_slot = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let super_instance = super_ctor
                    .get_instance_type(reg)
                    .expect("NullPointerException");
                super_instance.find_closest_definition(reg, ast, property_name.clone())
            };
            let super_class_has_property = prop_slot
                .is_some_and(|slot| !slot.is_owned_by_interface(compiler.get_type_registry()));
            found_property |= super_class_has_property;

            if super_class_has_property {
                let prop_slot = prop_slot.unwrap();
                let reg = compiler.get_type_registry();
                let super_class = prop_slot.get_owner_instance_type(reg);
                let super_class_has_declared_property =
                    !prop_slot.get_value().is_type_inferred(reg);
                if super_class_has_declared_property {
                    if Self::is_declared_locally(compiler, ctor_type, property_name)
                        && !Self::declares_override(info)
                    {
                        let reg = compiler.get_type_registry();
                        let human = property_name.human_readable_name(reg).to_string_lossy();
                        let reference_name = super_class
                            .get_reference_name(reg)
                            .map_or_else(|| "null".to_owned(), |name| name.to_string_lossy());
                        Self::report(
                            compiler,
                            n,
                            &HIDDEN_SUPERCLASS_PROPERTY,
                            &[&human, &reference_name],
                        );
                    }
                    let type_of_this = closure_jstype::function_type::get_type_of_this(
                        ctor_type,
                        compiler.get_type_registry(),
                    );
                    self.validator.check_property_type(
                        compiler,
                        n,
                        type_of_this,
                        super_class,
                        property_name.clone(),
                        property_type,
                    );
                }
            }
        }

        let implemented_interfaces = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            ctor_type.get_all_implemented_interfaces(reg, ast)
        };
        for implemented_interface in implemented_interfaces {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if implemented_interface.is_unknown_type(reg, ast)
                || implemented_interface.is_empty_type(reg)
            {
                continue;
            }
            check_state!(
                implemented_interface.is_instance_type(reg),
                "%s",
                JSType::to_string(implemented_interface, reg, ast)
            );
            let prop_slot =
                implemented_interface.find_closest_definition(reg, ast, property_name.clone());
            let interface_has_property = prop_slot.is_some();
            found_property |= interface_has_property;

            if interface_has_property && !Self::declares_override(info) {
                let human = property_name.human_readable_name(reg).to_string_lossy();
                let reference_name = prop_slot
                    .unwrap()
                    .get_owner_instance_type(reg)
                    .get_reference_name(reg)
                    .map_or_else(|| "null".to_owned(), |name| name.to_string_lossy());
                Self::report(
                    compiler,
                    n,
                    &HIDDEN_INTERFACE_PROPERTY,
                    &[&human, &reference_name],
                );
            }
        }

        if !found_property && Self::declares_override(info) {
            let reg = compiler.get_type_registry();
            let human = property_name.human_readable_name(reg).to_string_lossy();
            let reference_name = ctor_type
                .get_instance_type(reg)
                .expect("NullPointerException")
                .get_reference_name(reg)
                .map_or_else(|| "null".to_owned(), |name| name.to_string_lossy());
            Self::report(compiler, n, &UNKNOWN_OVERRIDE, &[&human, &reference_name]);
        }
    }

    // port: TypeCheck#isDeclaredLocally
    fn is_declared_locally(
        compiler: &mut AbstractCompiler,
        ctor_type: TypeId,
        property_name: &PropertyKey,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        check_state!(ctor_type.is_constructor(reg));
        let prototype = ctor_type.get_prototype(reg, ast);
        prototype.has_own_property(reg, ast, property_name.clone())
            || ctor_type
                .get_instance_type(reg)
                .expect("NullPointerException")
                .has_own_property(reg, ast, property_name.clone())
    }

    /// Given a `receiver_type`, check that the property (`property_name`) conforms to
    /// inheritance rules.
    ///
    /// This method only checks prototypal inheritance (via the prototype chain). Compare to
    /// `check_declared_property_against_nominal_inheritance`.
    ///
    /// To be conformant, the `property_name` must
    ///
    /// - Carry the `@override` annotation iff it is an override.
    /// - Be typed as a subtype of the type of `property_name` on all supertypes of
    ///   `receiver_type`.
    // port: TypeCheck#checkDeclaredPropertyAgainstPrototypalInheritance
    fn check_declared_property_against_prototypal_inheritance(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        receiver_type: TypeId,
        property_name: &PropertyKey,
        info: Option<&JSDocInfo>,
        property_type: TypeId,
    ) {
        // TODO(nickreid): Right now this is only expected to run on ctors. However, it wouldn't be
        // bad if it ran on more things. Consider a precondition on the arguments.

        let declared_override = Self::declares_override(info);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let mut supertype_with_property = None;
        let mut chain = receiver_type.get_implicit_prototype_chain();
        while let Some(type_) = chain.next(reg, ast) {
            // We want to report the supertype that actually had the overridden declaration.
            if type_.has_own_property(reg, ast, property_name.clone()) {
                // We only care about the lowest match in the chain because it must be the most
                // specific.
                supertype_with_property = Some(type_);
                break;
            }
        }

        match supertype_with_property {
            None => {
                // TODO(b/144327372): stop loosening typechecking for forward supertype references
                // once the type system correctly models this case.
                if declared_override
                    && !receiver_type.loosen_typechecking_due_to_forward_referenced_supertype(reg)
                {
                    let human = property_name.human_readable_name(reg).to_string_lossy();
                    let receiver_string = JSType::to_string(receiver_type, reg, ast);
                    Self::report(
                        compiler,
                        n, //
                        &UNKNOWN_PROTOTYPAL_OVERRIDE,
                        &[&human, &receiver_string],
                    );
                }
            }
            Some(supertype_with_property) => {
                if !declared_override {
                    let human = property_name.human_readable_name(reg).to_string_lossy();
                    let supertype_string = JSType::to_string(supertype_with_property, reg, ast);
                    Self::report(
                        compiler,
                        n, //
                        &HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY,
                        &[&human, &supertype_string],
                    );
                }

                let (reg, ast) = compiler.get_type_registry_and_ast();
                let overridden_property_type =
                    supertype_with_property.get_property_type(reg, ast, property_name.clone());
                if !property_type.is_subtype_of(reg, ast, overridden_property_type) {
                    let human = property_name.human_readable_name(reg).to_string_lossy();
                    let supertype_string = JSType::to_string(supertype_with_property, reg, ast);
                    let overridden_string = JSType::to_string(overridden_property_type, reg, ast);
                    let property_string = JSType::to_string(property_type, reg, ast);
                    Self::report(
                        compiler,
                        n,
                        &HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY_MISMATCH,
                        &[
                            &human,
                            &supertype_string,
                            &overridden_string,
                            &property_string,
                        ],
                    );
                }
            }
        }
    }

    // port: TypeCheck#checkAbstractMethodInConcreteClass
    fn check_abstract_method_in_concrete_class(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        ctor_type: TypeId,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };
        if !info.is_abstract() {
            return;
        }

        let reg = compiler.get_type_registry();
        if ctor_type.is_constructor(reg) && !FunctionType::is_abstract(ctor_type, reg) {
            Self::report(compiler, n, &ABSTRACT_METHOD_IN_CONCRETE_CLASS, &[]);
        }
    }

    /// Given a constructor or an interface type, find out whether the unknown type is a
    /// supertype of the current type.
    // port: TypeCheck#hasUnknownOrEmptySupertype
    fn has_unknown_or_empty_supertype(compiler: &mut AbstractCompiler, ctor: TypeId) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        check_argument!(ctor.is_constructor(reg) || ctor.is_interface(reg));
        check_argument!(!ctor.is_unknown_type(reg, ast));

        // The type system should notice inheritance cycles on its own
        // and break the cycle.
        let mut ctor = ctor;
        loop {
            let maybe_super_instance_type = ctor
                .get_prototype(reg, ast)
                .get_implicit_prototype(reg, ast);
            let Some(maybe_super_instance_type) = maybe_super_instance_type else {
                return false;
            };
            if maybe_super_instance_type.is_unknown_type(reg, ast)
                || maybe_super_instance_type.is_empty_type(reg)
            {
                return true;
            }
            let Some(next) = ObjectType::get_constructor(maybe_super_instance_type, reg) else {
                return false;
            };
            ctor = next;
            check_state!(ctor.is_constructor(reg) || ctor.is_interface(reg));
        }
    }

    /// Returns the type expected when using the key.
    ///
    /// @param key A OBJECTLIT key node.
    // port: TypeCheck#getObjectLitKeyTypeFromValueType
    pub fn get_object_lit_key_type_from_value_type(
        reg: &closure_jstype::JSTypeRegistry,
        ast: &closure_rhino::node::Ast,
        key: closure_rhino::node::NodeId,
        value_type: Option<closure_jstype::TypeId>,
    ) -> Option<closure_jstype::TypeId> {
        use closure_jstype::prelude::{FunctionType, JSType};
        use closure_rhino::token::Token;
        let mut value_type = value_type;
        if let Some(vt) = value_type {
            match key.get_token(ast) {
                Token::GETTER_DEF => {
                    // GET must always return a function type.
                    if vt.is_function_type(reg) {
                        let fntype = vt.to_maybe_function_type(reg).unwrap();
                        value_type = Some(fntype.get_return_type(reg));
                    } else {
                        return None;
                    }
                }
                Token::SETTER_DEF => {
                    if vt.is_function_type(reg) {
                        // SET must always return a function type.
                        let fntype = vt.to_maybe_function_type(reg).unwrap();
                        let param = fntype.get_parameters(reg)[0];
                        // SET function must always have one parameter.
                        value_type = Some(param.get_jstype());
                    } else {
                        return None;
                    }
                }
                _ => {}
            }
        }
        value_type
    }

    /// Visits an lvalue node for cases such as
    ///
    /// ```text
    /// interface.prototype.property = ...;
    /// ```
    // port: TypeCheck#visitInterfacePropertyAssignment
    fn visit_interface_property_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        object: NodeId,
        lvalue: NodeId,
    ) {
        if !lvalue.get_parent(compiler).unwrap().is_assign(compiler) {
            // assignments to interface properties cannot be in destructuring patterns or for-of
            // loops
            Self::report_invalid_interface_member_declaration(compiler, object);
            return;
        }
        let assign = lvalue.get_parent(compiler).unwrap();
        let rvalue = assign.get_second_child(compiler).unwrap();
        let rvalue_type = self.get_js_type(compiler, rvalue);

        // Only 2 values are allowed for interface methods:
        //    goog.abstractMethod
        //    function () {};
        // Other (non-method) interface properties must be stub declarations without assignments,
        // e.g.
        //     someinterface.prototype.nonMethodProperty;
        // which is why we enforce that `rvalueType.isFunctionType()`.
        if !rvalue_type.is_function_type(compiler.get_type_registry()) {
            Self::report_invalid_interface_member_declaration(compiler, object);
        }

        if rvalue.is_function(compiler)
            && !NodeUtil::is_empty_block(compiler, NodeUtil::get_function_body(compiler, rvalue))
        {
            let abstract_method_name = compiler.get_coding_convention().get_abstract_method_name();
            let abstract_method_name =
                abstract_method_name.map_or_else(|| "null".to_owned(), |n| n.to_string_lossy());
            Self::report(
                compiler,
                object,
                &INTERFACE_METHOD_NOT_EMPTY,
                &[&abstract_method_name],
            );
        }
    }

    // port: TypeCheck#reportInvalidInterfaceMemberDeclaration
    fn report_invalid_interface_member_declaration(
        compiler: &mut AbstractCompiler,
        interface_node: NodeId,
    ) {
        let abstract_method_name = compiler.get_coding_convention().get_abstract_method_name();
        // This is bad i18n style but we don't localize our compiler errors.
        let abstract_method_message = match abstract_method_name {
            Some(name) => format!(", or {}", name.to_string_lossy()),
            None => String::new(),
        };
        Self::report(
            compiler,
            interface_node,
            &INVALID_INTERFACE_MEMBER_DECLARATION,
            &[&abstract_method_message],
        );
    }

    /// Visits a NAME node.
    ///
    /// `t`: The node traversal object that supplies context, such as the scope chain to use in
    /// name lookups as well as error reporting. `n`: The node being visited. Returns whether the
    /// node is typeable or not.
    // port: TypeCheck#visitName
    fn visit_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) -> bool {
        // Skip empty function expression names. They don't need a type.
        let is_function_name =
            n.get_parent(t).unwrap().is_function(t) && n.is_first_child_of(t, parent);
        if is_function_name && n.get_string_ref(t).is_empty() {
            return false;
        }

        // the compiler's unknown type reporting allows certain names to be untyped if declared
        // but never used in an expression. The associated AST nodes still need to be typed to
        // prevent future passes from crashing, though.
        let mut report_if_missing_type = true;

        // At this stage, we need to determine whether this is a leaf
        // node in an expression (which therefore needs to have a type
        // assigned for it) versus some other decorative node that we
        // can safely ignore.
        let parent = parent.expect("NullPointerException");
        let parent_node_type = parent.get_token(t);
        if is_function_name
            || parent_node_type == Token::CATCH
            || parent_node_type == Token::PARAM_LIST
            || NodeUtil::is_name_declaration(t, Some(parent))
        {
            report_if_missing_type = false;
        }

        // Not need to type first key in for-in or for-of.
        if NodeUtil::is_enhanced_for(t, parent) && parent.get_first_child(t) == Some(n) {
            report_if_missing_type = false;
        }

        let mut type_ = n.get_jstype(t);
        if type_.is_none() {
            type_ = Some(self.get_native_type(t.get_compiler(), UNKNOWN_TYPE));
            // TODO(b/149843534): crash instead of defaulting to '?' when the var is null.
            let scope = t.get_typed_scope();
            let compiler = t.get_compiler();
            let name = n.get_string(compiler);
            let var = scope.get_var(compiler, name);
            if let Some(var) = var {
                type_ = Some(check_not_null!(var.get_type(compiler)));
            }
        }
        self.ensure_typed_with_type(t.get_compiler(), n, type_);
        report_if_missing_type
    }

    /// Visits the loop variable of a FOR_OF and FOR_AWAIT_OF and verifies the type being
    /// assigned to it.
    // port: TypeCheck#checkForOfTypes
    fn check_for_of_types(&mut self, t: &mut NodeTraversal<'_>, for_of: NodeId) {
        let mut lhs = for_of.get_first_child(t).unwrap();
        let iterable = for_of.get_second_child(t).unwrap();
        let compiler = t.get_compiler();
        let iterable_type = self.get_js_type(compiler, iterable);

        let actual_type: TypeId = if for_of.is_for_await_of(compiler) {
            let maybe_type = self
                .validator
                .expect_autoboxes_to_iterable_or_async_iterable(
                    compiler,
                    iterable,
                    iterable_type,
                    "Can only async iterate over a (non-null) Iterable or AsyncIterable type",
                );

            let Some(maybe_type) = maybe_type else {
                // Not iterable or async iterable, error reported by
                // expectAutoboxesToIterableOrAsyncIterable.
                return;
            };

            maybe_type
        } else {
            self.validator.expect_autoboxes_to_iterable(
                compiler,
                iterable,
                iterable_type,
                "Can only iterate over a (non-null) Iterable type",
            );

            {
                // Convert primitives to their wrapper type and remove null/undefined
                // If iterable is a union type, autoboxes each member of the union.
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let iterable_value_template = reg.get_iterable_value_template();
                iterable_type
                    .autobox(reg, ast)
                    .get_template_type_map(reg)
                    .get_resolved_template_type(reg, ast, iterable_value_template)
            }
        };

        if NodeUtil::is_name_declaration(compiler, Some(lhs)) {
            // e.g. get "x" given the VAR in "for (var x of arr) {"
            lhs = lhs.get_first_child(compiler).unwrap();
        }
        if lhs.is_destructuring_lhs(compiler) {
            // e.g. get `[x, y]` given the VAR in `for (var [x, y] of arr) {`
            lhs = lhs.get_first_child(compiler).unwrap();
        }

        let info = lhs.get_jsdoc_info(compiler);
        self.check_can_assign_to_with_scope(
            t,
            for_of,
            lhs,
            actual_type,
            info.as_deref(),
            "declared type of for-of loop variable does not match inferred type",
        );
    }

    /// Visits a GETPROP node.
    ///
    /// `t`: The node traversal object that supplies context, such as the scope chain to use in
    /// name lookups as well as error reporting. `n`: The node being visited.
    // port: TypeCheck#visitGetProp
    fn visit_get_prop(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        // obj.prop or obj.method()
        // Lots of types can appear on the left, a call to a void function can
        // never be on the left. getPropertyType will decide what is acceptable
        // and what isn't.
        let obj_node = n.get_first_child(t).unwrap();
        let child_type = self.get_js_type(t.get_compiler(), obj_node);

        let is_dict = {
            let (reg, ast) = t.get_compiler().get_type_registry_and_ast();
            child_type.is_dict(reg, ast)
        };
        if is_dict {
            Self::report(
                t.get_compiler(),
                n,
                &type_validator::ILLEGAL_PROPERTY_ACCESS,
                &["'.'", "dict"],
            );
        } else {
            let object_type = self.get_native_type(t.get_compiler(), OBJECT_TYPE);
            if self.validator.expect_not_null_or_undefined(
                t,
                n,
                child_type,
                "No properties on this expression",
                object_type,
            ) {
                self.check_property_access_for_get_prop(t.get_compiler(), n);
            }
        }
        self.ensure_typed(t.get_compiler(), n);
    }

    /// Visits a OPTCHAIN_GETPROP node.
    ///
    /// `opt_chain_get_prop`: The node being visited.
    // port: TypeCheck#visitOptChainGetProp
    fn visit_opt_chain_get_prop(
        &mut self,
        compiler: &mut AbstractCompiler,
        opt_chain_get_prop: NodeId,
    ) {
        // obj?.prop
        // Unlike GETPROP, a call to a void function can also be on the lhs for an OPTCHAIN_GETPROP.
        let property = opt_chain_get_prop.get_last_child(compiler).unwrap();
        let obj_node = opt_chain_get_prop.get_first_child(compiler).unwrap();
        let child_type = self.get_js_type(compiler, obj_node);

        let (is_dict, is_unknown) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let is_dict = child_type.is_dict(reg, ast);
            (is_dict, !is_dict && child_type.is_unknown_type(reg, ast))
        };
        if is_dict {
            Self::report(
                compiler,
                property,
                &type_validator::ILLEGAL_PROPERTY_ACCESS,
                &["'?.'", "dict"],
            );
        } else if !is_unknown {
            self.check_property_access_for_get_prop(compiler, opt_chain_get_prop);
        }
        self.ensure_typed(compiler, opt_chain_get_prop);
    }

    // port: TypeCheck#checkPropertyAccessForGetProp
    fn check_property_access_for_get_prop(
        &mut self,
        compiler: &mut AbstractCompiler,
        get_prop: NodeId,
    ) {
        check_argument!(
            get_prop.is_get_prop(compiler) || get_prop.is_opt_chain_get_prop(compiler),
            "%s",
            get_prop.to_string(compiler)
        );
        let obj_node = get_prop.get_first_child(compiler).unwrap();
        let obj_type = self.get_js_type(compiler, obj_node);
        let prop_type = self.get_js_type(compiler, get_prop);

        self.check_abstract_property_access(compiler, get_prop);
        self.check_property_access(compiler, obj_type, get_prop, prop_type, Some(obj_node));
    }

    // port: TypeCheck#checkPropertyAccessForDestructuring
    fn check_property_access_for_destructuring(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
        object_type: TypeId,
        string_key: NodeId,
        inferred_prop_type: TypeId,
    ) {
        check_argument!(
            pattern.is_destructuring_pattern(compiler),
            "%s",
            pattern.to_string(compiler)
        );
        check_argument!(
            string_key.is_string_key(compiler),
            "%s",
            string_key.to_string(compiler)
        );

        // Get the object node being destructured when it exists. These cases have an actual node
        // `obj`:
        //   const {a} = obj;
        //   ({a} = obj);
        // while these do not:
        //    for (const {a} of arrayOfObjects) {
        //    const {{a}} = obj;
        let mut obj_node = None;
        let pattern_parent = pattern.get_parent(compiler).unwrap();
        if (pattern_parent.is_assign(compiler) || pattern_parent.is_destructuring_lhs(compiler))
            && pattern.get_next(compiler).is_some()
        {
            obj_node = pattern.get_next(compiler);
        }
        self.check_property_access(
            compiler,
            object_type,
            string_key,
            inferred_prop_type,
            obj_node,
        );
    }

    /// Warns if @abstract methods are dereferenced, with some false negatives
    ///
    /// This method only handles some cases that we are certain are incorrect. e.g. we are lenient
    /// about union types, and we don't track an abstract methods once they are reassigned to new
    /// variables.
    ///
    /// This method's logic is complicated to avoid spurious warnings. Sometimes a reference to
    /// something typed @abstract is okay. For example, don't warn on `this.foo();` in
    /// `/** @abstract * / class Base { /** @abstract * / foo() {} bar() { this.foo(); } }`
    ///
    /// The `this` object with which `Base.prototype.bar` is called must be a concrete subclass of
    /// Base. To avoid false positives, we warn only in the following cases:
    ///
    /// - (a) the function is accessed off `super`
    /// - (b) the function is accessed off a .prototype
    /// - (c) the function is transpiled from a goog.base superclass reference
    // port: TypeCheck#checkAbstractPropertyAccess
    fn check_abstract_property_access(&mut self, compiler: &mut AbstractCompiler, method: NodeId) {
        if NodeUtil::is_lhs_of_assign(compiler, method) {
            // Allow declaring abstract methods. (This assumes they are never re-assigned)
            //   /** @abstract */ Foo.prototype.bar = function() {}
            return;
        }

        let method_js_type = self.get_js_type(compiler, method);
        let reg = compiler.get_type_registry();
        let method_type = method_js_type.to_maybe_function_type(reg);
        let Some(method_type) = method_type else {
            return;
        };
        if !method_type.is_abstract(reg) || method_type.is_constructor(reg) {
            // Ignore non-abstract methods and @abstract constructors. An @abstract constructor is
            // still callable.
            return;
        }
        let display_name = method_type
            .get_display_name(reg)
            .unwrap_or_else(|| "null".to_string());

        let object_node = method.get_first_child(compiler).unwrap();
        if object_node.is_super(compiler) {
            // case (a)
            // `super.foo()` definitely refers to `Superclass.prototype.foo`, not an override.
            // At parse time, `Subclass.prototype` becomes a lower bound for what `super`
            // evaluates to, even if the `this` object changes. So `super` will never resolve to a
            // concrete subclass.
            Self::report(
                compiler,
                method,
                &ABSTRACT_SUPER_METHOD_NOT_USABLE,
                &[&display_name],
            );
        } else if object_node.is_get_prop(compiler) {
            let object_prop = object_node.get_string(compiler);
            if object_prop == "prototype" // case (b), e.g. `Foo.prototype.bar`
                || compiler
                    .get_coding_convention()
                    .is_super_class_reference(&object_prop)
            {
                // case (c)
                Self::report(
                    compiler,
                    method,
                    &ABSTRACT_SUPER_METHOD_NOT_USABLE,
                    &[&display_name],
                );
            }
        }
    }

    /// Emits a warning if we can prove that a property cannot possibly be defined on an object.
    /// Note the difference between JS and a strictly statically typed language: we're checking if
    /// the property *cannot be defined*, whereas a java compiler would check if the property *can
    /// be undefined.
    ///
    /// This method handles property access in both GETPROPs and object destructuring.
    /// Consequentially some of its arguments are optional - the actual object node and the getprop
    /// - while others are required.
    ///
    /// `obj_node`: the actual node representing the object we're accessing. optional because
    /// destructuring accesses MAY not have an actual object node
    // port: TypeCheck#checkPropertyAccess
    fn check_property_access(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut child_type: TypeId,
        prop_node: NodeId,
        prop_type: TypeId,
        obj_node: Option<NodeId>,
    ) {
        let is_getprop = NodeUtil::is_normal_or_opt_chain_get_prop(compiler, prop_node);
        let prop_name = prop_node.get_string(compiler);

        let prop_type_is_unknown = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let unknown = reg.get_native_type(UNKNOWN_TYPE);
            prop_type.equals(reg, ast, unknown)
        };
        if prop_type_is_unknown {
            let object_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                child_type = child_type.autobox(reg, ast);
                closure_jstype::object_type::cast(reg, Some(child_type))
            };
            if let Some(object_type) = object_type {
                // We special-case object types so that checks on enums can be
                // much stricter, and so that we can use hasProperty (which is much
                // faster in most cases).
                let missing = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let unknown = reg.get_native_type(UNKNOWN_TYPE);
                    !object_type.has_property(reg, ast, StringKey::new(prop_name.clone()))
                        || object_type.equals(reg, ast, unknown)
                };
                if missing {
                    let is_enum_type = {
                        let reg = compiler.get_type_registry();
                        object_type.to_maybe_enum_type(reg) == Some(object_type)
                    };
                    if is_enum_type {
                        Self::report(
                            compiler,
                            prop_node,
                            &INEXISTENT_ENUM_ELEMENT,
                            &[&prop_name.to_string()],
                        );
                    } else {
                        self.check_property_access_helper(
                            compiler,
                            object_type,
                            prop_node,
                            obj_node,
                            false,
                        );
                    }
                }
            } else {
                self.check_property_access_helper(compiler, child_type, prop_node, obj_node, false);
            }
        } else if child_type.is_union_type(compiler.get_type_registry())
            && is_getprop
            && !Self::is_l_value_get_prop(compiler, prop_node)
        {
            // NOTE: strict property assignment checks are done on assignment.
            self.check_property_access_helper(compiler, child_type, prop_node, obj_node, true);
        }
    }

    // port: TypeCheck#isLValueGetProp
    fn is_l_value_get_prop(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        // TODO(b/77597706): this won't work for destructured lvalues
        (NodeUtil::is_update_operator(compiler, parent)
            || NodeUtil::is_assignment_op(compiler, parent))
            && parent.get_first_child(compiler) == Some(n)
    }

    /// `strict_check`: Whether this is a check that is only performed when "strict missing
    /// properties" checks are enabled.
    // port: TypeCheck#checkPropertyAccessHelper
    fn check_property_access_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        object_type: TypeId,
        prop_node: NodeId,
        obj_node: Option<NodeId>,
        strict_check: bool,
    ) {
        let is_getprop = NodeUtil::is_normal_or_opt_chain_get_prop(compiler, prop_node);
        let prop_name = prop_node.get_string(compiler);

        if !self.report_missing_properties
            || object_type.is_empty_type(compiler.get_type_registry())
            || (is_getprop && Self::allow_strict_property_access_on_node(compiler, prop_node))
        {
            return;
        }

        let (kind, is_unknown_type) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let kind = reg.can_property_be_defined(ast, object_type, prop_name);
            (kind, object_type.is_unknown_type(reg, ast))
        };
        if kind == PropDefinitionKind::KNOWN {
            return;
        }
        // If the property definition is known, but only loosely associated,
        // only report a "strict error" which can be optional as code is migrated.
        let is_loosely_associated =
            kind == PropDefinitionKind::LOOSE || kind == PropDefinitionKind::LOOSE_UNION;
        if is_loosely_associated && is_unknown_type {
            // We still don't want to report this.
            return;
        }

        let is_struct = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            object_type.is_struct(reg, ast)
        };
        let loose_property_declaration =
            !is_struct && is_getprop && Self::is_q_name_assignment_target(compiler, prop_node);
        // always false for destructuring
        let maybe_prop_existence_check = !is_struct
            && is_getprop
            && Self::allow_loose_property_access_on_node(compiler, prop_node);
        // Traditionally, we would not report a warning for "loose" properties, but we want to be
        // able to be more strict, so introduce an optional warning.
        let strict_report = strict_check
            || is_loosely_associated
            || loose_property_declaration
            || maybe_prop_existence_check;

        self.report_missing_property(
            compiler,
            obj_node,
            object_type,
            prop_node,
            kind,
            strict_report,
        );
    }

    // port: TypeCheck#reportMissingProperty
    fn report_missing_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        obj_node: Option<NodeId>,
        object_type: TypeId,
        prop_node: NodeId,
        kind: PropDefinitionKind,
        strict_report: bool,
    ) {
        let prop_name = prop_node.get_string(compiler);

        let object_native = self.get_native_type(compiler, OBJECT_TYPE);
        let low_confidence = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let is_object_type = object_type.equals(reg, ast, object_native);
            object_type.is_unknown_type(reg, ast) || object_type.is_all_type(reg) || is_object_type
        };

        let is_known_to_union_member = kind == PropDefinitionKind::LOOSE_UNION;

        let mut pair = None;
        if !low_confidence && !is_known_to_union_member {
            pair = Self::get_closest_property_suggestion(
                object_type,
                &prop_name,
                (prop_name.length() as i32 - 1) / 4,
            );
        }
        let object_name = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            match obj_node {
                Some(obj_node) => reg.get_readable_type_name(ast, obj_node),
                None => object_type.to_string(reg, ast),
            }
        };
        if let Some(pair) = pair {
            let report_type = if strict_report {
                &STRICT_INEXISTENT_PROPERTY_WITH_SUGGESTION
            } else {
                &INEXISTENT_PROPERTY_WITH_SUGGESTION
            };
            Self::report(
                compiler,
                prop_node,
                report_type,
                &[&prop_name.to_string(), &object_name, &pair.suggestion],
            );
        } else {
            let report_type = if strict_report {
                if is_known_to_union_member {
                    &STRICT_INEXISTENT_UNION_PROPERTY
                } else {
                    &STRICT_INEXISTENT_PROPERTY
                }
            } else if low_confidence {
                &POSSIBLE_INEXISTENT_PROPERTY
            } else {
                &INEXISTENT_PROPERTY
            };
            Self::report(
                compiler,
                prop_node,
                report_type,
                &[&prop_name.to_string(), &object_name],
            );
        }
    }

    // port: TypeCheck#allowStrictPropertyAccessOnNode
    fn allow_strict_property_access_on_node(compiler: &AbstractCompiler, n: NodeId) -> bool {
        n.get_parent(compiler).unwrap().is_type_of(compiler)
    }

    /// Checks whether a property access is either (1) a stub declaration, or (2) a property
    /// presence or absence test.
    ///
    /// Presence and absence are both allowed here because both are valid for conditional use of a
    /// property (e.g. `if (x.y != null) use(x.y);` but also the opposite, `if (x.y == null)
    /// return; use(x.y);`).
    // port: TypeCheck#allowLoosePropertyAccessOnNode
    fn allow_loose_property_access_on_node(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        NodeUtil::is_property_test(compiler, n)
            || NodeUtil::is_property_absence_test(compiler, n)
            // Stub property declaration
            || (n.is_qualified_name(compiler) && parent.is_expr_result(compiler))
    }

    // port: TypeCheck#isQNameAssignmentTarget
    fn is_q_name_assignment_target(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        n.is_qualified_name(compiler)
            && parent.is_assign(compiler)
            && parent.get_first_child(compiler) == Some(n)
    }

    // port: TypeCheck#getClosestPropertySuggestion
    fn get_closest_property_suggestion(
        object_type: TypeId,
        prop_name: &JsString,
        max_distance: i32,
    ) -> Option<SuggestionPair> {
        let _ = (object_type, prop_name, max_distance);
        None
    }

    /// Visits a GETELEM node.
    ///
    /// `n`: The node being visited.
    // port: TypeCheck#visitGetElem
    fn visit_get_elem(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.get_first_child(compiler).unwrap();
        let last = n.get_last_child(compiler).unwrap();
        let obj_type = self.get_js_type(compiler, first);
        let index_type = self.get_js_type(compiler, last);
        self.validator
            .expect_index_match(compiler, n, obj_type, index_type);
        self.ensure_typed(compiler, n);
    }

    /// Visits a OPTCHAIN_GETELEM node.
    ///
    /// `opt_chain_get_elem`: The node being visited.
    // port: TypeCheck#visitOptChainGetElem
    fn visit_opt_chain_get_elem(
        &mut self,
        compiler: &mut AbstractCompiler,
        opt_chain_get_elem: NodeId,
    ) {
        let obj = opt_chain_get_elem.get_first_child(compiler).unwrap();
        let obj_type = self.get_js_type(compiler, obj);
        let reg = compiler.get_type_registry();
        if obj_type.is_null_type(reg) || obj_type.is_void_type(reg) {
            // no error reported when conditionally checking a null or void lhs object.
            // e.g. `a?.[b]` should not report if `a` is null`
            self.ensure_typed_native(compiler, opt_chain_get_elem, VOID_TYPE);
        } else {
            self.visit_get_elem(compiler, opt_chain_get_elem);
        }
    }

    /// Visits a VAR node.
    ///
    /// `t`: The node traversal object that supplies context, such as the scope chain to use in
    /// name lookups as well as error reporting. `n`: The node being visited.
    // port: TypeCheck#visitVar
    fn visit_var(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        // Handle var declarations in for-of loops separately from regular var declarations.
        let n_parent = n.get_parent(t).unwrap();
        if n_parent.is_for_of(t) || n_parent.is_for_in(t) || n_parent.is_for_await_of(t) {
            return;
        }

        // TODO(nicksantos): Fix this so that the doc info always shows up
        // on the NAME node. We probably want to wait for the parser
        // merge to fix this.
        let var_info = if n.has_one_child(t) {
            n.get_jsdoc_info(t)
        } else {
            None
        };
        let mut child = n.get_first_child(t);
        while let Some(c) = child {
            if c.is_name(t) {
                let value = c.get_first_child(t);

                if let Some(value) = value {
                    let value_type = self.get_js_type(t.get_compiler(), value);
                    let mut info = c.get_jsdoc_info(t);
                    if info.is_none() {
                        info = var_info.clone();
                    }

                    self.check_enum_alias(t, info.as_deref(), value_type, value);
                    self.check_can_assign_to_with_scope(
                        t,
                        value,
                        c,
                        value_type,
                        info.as_deref(),
                        "initializing variable",
                    );
                }
            } else {
                check_state!(c.is_destructuring_lhs(t), "%s", c.to_string(t));
                let name = c.get_first_child(t).unwrap();
                let value = c.get_second_child(t).unwrap();
                let value_type = self.get_js_type(t.get_compiler(), value);
                self.check_can_assign_to_with_scope(
                    t,
                    c,
                    name,
                    value_type,
                    /* info= */ None,
                    "initializing variable",
                );
            }
            child = c.get_next(t);
        }
    }

    /// Visits a NEW node.
    // port: TypeCheck#visitNew
    fn visit_new(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let constructor = n.get_first_child(compiler).unwrap();
        let constructor_type = self.get_js_type(compiler, constructor);
        let (type_, not_a_constructor) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let type_ = constructor_type.restrict_by_not_null_or_undefined(reg, ast);
            let symbol_ctor = reg.get_native_type(SYMBOL_OBJECT_FUNCTION_TYPE);
            let bigint_ctor = reg.get_native_type(BIGINT_OBJECT_FUNCTION_TYPE);
            let not_a_constructor = !Self::could_be_a_constructor(reg, ast, type_)
                || type_.equals(reg, ast, symbol_ctor)
                || type_.equals(reg, ast, bigint_ctor);
            (type_, not_a_constructor)
        };
        if not_a_constructor {
            let type_string = Self::type_to_string(compiler, type_);
            Self::report(compiler, n, &NOT_A_CONSTRUCTOR, &[&type_string]);
            self.ensure_typed(compiler, n);
            return;
        }

        let reg = compiler.get_type_registry();
        let fn_type = type_.to_maybe_function_type(reg);
        if let Some(fn_type) = fn_type.filter(|f| f.has_instance_type(reg)) {
            if fn_type.is_abstract(compiler.get_type_registry()) {
                Self::report(compiler, n, &INSTANTIATE_ABSTRACT_CLASS, &[]);
            }

            self.visit_argument_list(compiler, n, fn_type);

            let obj_type = FunctionType::get_instance_type(fn_type, compiler.get_type_registry());
            let instance = match obj_type {
                Some(obj_type) => obj_type,
                None => self.get_native_type(compiler, UNKNOWN_TYPE),
            };
            self.ensure_typed_with_type(compiler, n, Some(instance));
        } else {
            self.ensure_typed(compiler, n);
        }
    }

    // port: TypeCheck#couldBeAConstructor
    fn could_be_a_constructor(reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        type_.is_constructor(reg) || type_.is_empty_type(reg) || type_.is_unknown_type(reg, ast)
    }

    /// Check whether there's any property conflict for a particular super interface
    ///
    /// `n`: The node being visited. `function_name`: The function name being checked.
    /// `properties`: The property names in the super interfaces that have been visited.
    /// `current_properties`: The property names in the super interface that have been visited.
    /// `interface_type`: The super interface that is being visited.
    // port: TypeCheck#checkInterfaceConflictProperties
    fn check_interface_conflict_properties(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        function_name: &str,
        properties: &PropertyKeyMap,
        current_properties: &mut PropertyKeyMap,
        interface_type: TypeId,
    ) {
        let current_property_names: Vec<PropertyKey> = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let implicit_proto = interface_type.get_implicit_prototype(reg, ast);
            match implicit_proto {
                // This can be the case if interfaceType is proxy to a non-existent
                // object (which is a bad type annotation, but shouldn't crash).
                None => Vec::new(),
                Some(implicit_proto) => implicit_proto.get_own_property_keys(reg),
            }
        };
        for name in current_property_names {
            let o_type = properties.get(&name);
            current_properties.put(name.clone(), interface_type);
            if let Some(o_type) = o_type {
                let compatible = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let this_prop_type = interface_type.get_property_type(reg, ast, name.clone());
                    let o_prop_type = o_type.get_property_type(reg, ast, name.clone());
                    this_prop_type.is_subtype_of_with_mode(
                        reg,
                        ast,
                        o_prop_type,
                        self.subtyping_mode,
                    ) || o_prop_type.is_subtype_of_with_mode(
                        reg,
                        ast,
                        this_prop_type,
                        self.subtyping_mode,
                    ) || (this_prop_type.is_function_type(reg)
                        && o_prop_type.is_function_type(reg)
                        && {
                            let this_fn = this_prop_type.to_maybe_function_type(reg).unwrap();
                            let o_fn = o_prop_type.to_maybe_function_type(reg).unwrap();
                            this_fn.has_equal_call_type(reg, ast, o_fn)
                        })
                };
                if compatible {
                    continue;
                }
                let human_readable_name = name
                    .human_readable_name(compiler.get_type_registry())
                    .to_string_lossy();
                let o_type_string = Self::type_to_string(compiler, o_type);
                let interface_type_string = Self::type_to_string(compiler, interface_type);
                let error = JSError::make(
                    compiler,
                    n,
                    &INCOMPATIBLE_EXTENDED_PROPERTY_TYPE,
                    &[
                        function_name,
                        &human_readable_name,
                        &o_type_string,
                        &interface_type_string,
                    ],
                );
                compiler.report(error);
            }
        }
        let extended = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            interface_type.get_ctor_extended_interfaces(reg, ast)
        };
        for i_type in extended {
            self.check_interface_conflict_properties(
                compiler,
                n,
                function_name,
                properties,
                current_properties,
                i_type,
            );
        }
    }

    /// Rust-only: `JSType.toMaybeFunctionType(n.getJSType())` followed by a dereference (Java
    /// throws NullPointerException when it is null).
    fn to_maybe_function_type_of_node(compiler: &mut AbstractCompiler, n: NodeId) -> TypeId {
        let type_ = n.get_jstype(compiler);
        type_
            .and_then(|t| t.to_maybe_function_type(compiler.get_type_registry()))
            .expect("NullPointerException")
    }

    /// Visits a `Token::FUNCTION` node.
    ///
    /// `n`: The node being visited.
    // port: TypeCheck#visitFunction
    fn visit_function(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if NodeUtil::is_es6_constructor(compiler, n) {
            return; // These will be checked via the CLASS node.
        }

        let function_type = Self::to_maybe_function_type_of_node(compiler, n);
        let reg = compiler.get_type_registry();
        if function_type.is_constructor(reg) {
            self.check_constructor(compiler, n, function_type);
        } else if function_type.is_interface(reg) {
            self.check_interface(compiler, n, function_type);
        } else if n.is_async_generator_function(compiler) {
            // An async generator function must return a AsyncGenerator or supertype of
            // AsyncGenerator
            let return_type = function_type.get_return_type(compiler.get_type_registry());
            self.validator.expect_async_generator_supertype(
                compiler,
                n,
                return_type,
                "An async generator function must return a (supertype of) AsyncGenerator",
            );
        } else if n.is_generator_function(compiler) {
            // A generator function must return a Generator or supertype of Generator
            let return_type = function_type.get_return_type(compiler.get_type_registry());
            self.validator.expect_generator_supertype(
                compiler,
                n,
                return_type,
                "A generator function must return a (supertype of) Generator",
            );
        } else if n.is_async_function(compiler) {
            // An async function must return a Promise or supertype of Promise
            let return_type = function_type.get_return_type(compiler.get_type_registry());
            self.validator
                .expect_valid_async_return_type(compiler, n, return_type);
        }
    }

    /// Visits a CLASS node.
    // port: TypeCheck#visitClass
    fn visit_class(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let function_type = Self::to_maybe_function_type_of_node(compiler, n);
        let extends_clause = n.get_second_child(compiler).unwrap();
        if !extends_clause.is_empty(compiler) {
            // Ensure that the `extends` clause is actually a constructor or interface.  If it is,
            // but it's the wrong one then checkConstructor or checkInterface will warn.
            let super_type = extends_clause
                .get_jstype(compiler)
                .expect("NullPointerException");
            let reg = compiler.get_type_registry();
            if super_type.is_constructor(reg) || super_type.is_interface(reg) {
                let super_ctor = super_type.to_maybe_function_type(reg);
                self.validator
                    .expect_extends(compiler, n, function_type, super_ctor);
            } else if !{
                let (reg, ast) = compiler.get_type_registry_and_ast();
                super_type.is_unknown_type(reg, ast)
            } {
                // Only give this error for supertypes *known* to be wrong - unresolved types are
                // OK here.
                let kind = if function_type.is_constructor(compiler.get_type_registry()) {
                    "constructor"
                } else {
                    "interface"
                };
                let best_name = Self::get_best_function_name(compiler, n);
                let error =
                    JSError::make(compiler, n, &CONFLICTING_EXTENDED_TYPE, &[kind, &best_name]);
                compiler.report(error);
            }
        }
        let reg = compiler.get_type_registry();
        if function_type.is_constructor(reg) {
            self.check_constructor(compiler, n, function_type);
        } else if function_type.is_interface(reg) {
            self.check_interface(compiler, n, function_type);
        } else {
            panic!(
                "IllegalStateException: CLASS node's type must be either constructor or interface: {}",
                Self::type_to_string(compiler, function_type)
            );
        }
    }

    /// Checks a constructor, which may be either an ES5-style FUNCTION node, or a CLASS node.
    // port: TypeCheck#checkConstructor
    fn check_constructor(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        function_type: TypeId,
    ) {
        let object_function_type = self.get_native_type(compiler, OBJECT_FUNCTION_TYPE);
        let (base_constructor, extends_interface) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let base_constructor = function_type.get_super_class_constructor(reg, ast);
            let equals_object_function =
                base_constructor.is_some_and(|b| b.equals(reg, ast, object_function_type));
            let extends_interface =
                !equals_object_function && base_constructor.is_some_and(|b| b.is_interface(reg));
            (base_constructor, extends_interface)
        };
        if extends_interface {
            // Warn if a class extends an interface.
            let best_name = Self::get_best_function_name(compiler, n);
            let error = JSError::make(
                compiler,
                n,
                &CONFLICTING_EXTENDED_TYPE,
                &["constructor", &best_name],
            );
            compiler.report(error);
        } else {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if n.is_function(ast)
                && base_constructor.is_some_and(|b| {
                    FunctionType::get_source(b, reg).is_some_and(|s| s.is_class(ast))
                })
                && !FunctionType::get_source(function_type, reg)
                    .expect("NullPointerException")
                    .is_class(ast)
            {
                let base_constructor = base_constructor.unwrap();
                // Warn if an ES5 class extends an ES6 class.
                let function_name = function_type
                    .get_display_name(reg)
                    .unwrap_or_else(|| "null".to_string());
                let base_name = base_constructor
                    .get_display_name(reg)
                    .unwrap_or_else(|| "null".to_string());
                let error = JSError::make(
                    compiler,
                    n,
                    &ES5_CLASS_EXTENDING_ES6_CLASS,
                    &[&function_name, &base_name],
                );
                compiler.report(error);
            }
            if let Some(base_constructor) = base_constructor {
                Self::check_struct_dict_subtyping(compiler, n, function_type, base_constructor);
            }

            // Warn if any @implemented types are not interfaces or if there are any duplicates
            let mut already_seen_interfaces: Vec<TypeId> = Vec::new();

            let own_implemented =
                function_type.get_own_implemented_interfaces(compiler.get_type_registry());
            for mut base_interface in own_implemented {
                let mut bad_implemented_type = false;
                let reg = compiler.get_type_registry();
                let base_interface_obj =
                    closure_jstype::object_type::cast(reg, Some(base_interface));
                if let Some(base_interface_obj) = base_interface_obj {
                    let interface_constructor = base_interface_obj.get_constructor(reg);
                    if interface_constructor.is_some_and(|c| !c.is_interface(reg)) {
                        bad_implemented_type = true;
                    }
                } else {
                    bad_implemented_type = true;
                }
                if bad_implemented_type {
                    let best_name = Self::get_best_function_name(compiler, n);
                    Self::report(compiler, n, &BAD_IMPLEMENTED_TYPE, &[&best_name]);
                }
                // Disallow implementing both `Foo<string>` and `Foo<number>`, for example.
                base_interface = Self::normalize_templatized_type(compiler, base_interface);
                let already_seen = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    already_seen_interfaces
                        .iter()
                        .any(|&seen| seen.equals(reg, ast, base_interface))
                };
                if already_seen {
                    let type_string = Self::type_to_string(compiler, base_interface);
                    Self::report(
                        compiler,
                        n,
                        &SAME_INTERFACE_MULTIPLE_IMPLEMENTS,
                        &[&type_string],
                    );
                } else {
                    already_seen_interfaces.push(base_interface);
                }
            }
            // check properties
            self.validator
                .expect_all_interface_properties(compiler, n, function_type);
            if !function_type.is_abstract(compiler.get_type_registry()) {
                self.validator
                    .expect_abstract_methods_implemented(compiler, n, function_type);
            }
        }
    }

    /// Normalizes `Foo<string>` to just `Foo` and is the identify function for other types
    // port: TypeCheck#normalizeTemplatizedType
    fn normalize_templatized_type(
        compiler: &mut AbstractCompiler,
        maybe_templatized_type: TypeId,
    ) -> TypeId {
        let reg = compiler.get_type_registry();
        if !maybe_templatized_type.is_templatized_type(reg) {
            return maybe_templatized_type;
        }
        maybe_templatized_type
            .to_maybe_templatized_type(reg)
            .unwrap()
            .get_raw_type(reg)
    }

    // port: TypeCheck#checkStructDictSubtyping
    fn check_struct_dict_subtyping(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        subtype: TypeId,
        supertype: TypeId,
    ) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let diagnostic = if subtype.makes_dicts(reg, ast) && supertype.makes_structs(reg, ast) {
            Some(&DICT_EXTEND_STRUCT_TYPE)
        } else if subtype.makes_structs(reg, ast) && supertype.makes_dicts(reg, ast) {
            Some(&STRUCT_EXTEND_DICT_TYPE)
        } else {
            None
        };
        if let Some(diagnostic) = diagnostic {
            let subtype_name = subtype
                .get_display_name(reg)
                .unwrap_or_else(|| "null".to_string());
            let supertype_name = supertype
                .get_display_name(reg)
                .unwrap_or_else(|| "null".to_string());
            let error = JSError::make(compiler, n, diagnostic, &[&subtype_name, &supertype_name]);
            compiler.report(error);
        }
    }

    /// Checks an interface, which may be either an ES5-style FUNCTION node, or a CLASS node.
    // port: TypeCheck#checkInterface
    fn check_interface(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        function_type: TypeId,
    ) {
        // Interface must extend only interfaces
        let extended = function_type.get_extended_interfaces(compiler.get_type_registry());
        for ext_interface in &extended {
            let reg = compiler.get_type_registry();
            let ctor = ext_interface.get_constructor(reg);
            if ctor.is_some_and(|c| !c.is_interface(reg)) {
                let best_name = Self::get_best_function_name(compiler, n);
                let error = JSError::make(
                    compiler,
                    n,
                    &CONFLICTING_EXTENDED_TYPE,
                    &["interface", &best_name],
                );
                compiler.report(error);
            }
        }

        // Check whether the extended interfaces have any conflicts
        if function_type.get_extended_interfaces_count(compiler.get_type_registry()) > 1 {
            // Only check when extending more than one interfaces
            let mut properties = PropertyKeyMap::default();
            let mut current_properties = PropertyKeyMap::default();
            for interface_type in
                function_type.get_extended_interfaces(compiler.get_type_registry())
            {
                current_properties.clear();
                let best_name = Self::get_best_function_name(compiler, n);
                self.check_interface_conflict_properties(
                    compiler,
                    n,
                    &best_name,
                    &properties,
                    &mut current_properties,
                    interface_type,
                );
                properties.put_all(&current_properties);
            }
        }

        let loop_path = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            function_type.check_extends_loop(reg, ast)
        };
        if let Some(loop_path) = loop_path {
            let reg = compiler.get_type_registry();
            let display_name = |t: TypeId| {
                t.get_display_name(reg)
                    .unwrap_or_else(|| "null".to_string())
            };
            let str_path = loop_path
                .iter()
                .map(|&t| display_name(t))
                .collect::<Vec<_>>()
                .join(" -> ");
            let first = display_name(loop_path[0]);
            let error = JSError::make(compiler, n, &INTERFACE_EXTENDS_LOOP, &[&first, &str_path]);
            compiler.report(error);
        }

        self.validator
            .expect_all_interface_properties(compiler, n, function_type);
    }

    // port: TypeCheck#getBestFunctionName
    fn get_best_function_name(compiler: &AbstractCompiler, n: NodeId) -> String {
        check_state!(n.is_class(compiler) || n.is_function(compiler));
        let name =
            NodeUtil::get_best_l_value_name(compiler, NodeUtil::get_best_l_value(compiler, n));
        match name {
            Some(name) => name.to_string_lossy(),
            None => format!(
                "<anonymous@{}:{}>",
                n.get_source_file_name(compiler)
                    .unwrap_or_else(|| "null".to_string()),
                n.get_lineno(compiler)
            ),
        }
    }

    /// Validate class-defining calls. Because JS has no 'native' syntax for defining classes, we
    /// need to do this manually.
    // port: TypeCheck#checkCallConventions
    fn check_call_conventions(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let relationship = {
            let compiler = t.get_compiler();
            compiler
                .get_coding_convention()
                .get_classes_defined_by_call(&compiler.ast, n)
        };
        let scope = t.get_typed_scope();
        let Some(relationship) = relationship else {
            return;
        };
        let compiler = t.get_compiler();
        let super_class = Self::lookup_qualified_name(
            compiler,
            scope,
            &QualifiedName::of(relationship.superclass_name.clone()),
        );
        let super_class_instance_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            TypeValidator::get_instance_of_ctor(reg, ast, super_class)
        };
        let sub_class = Self::lookup_qualified_name(
            compiler,
            scope,
            &QualifiedName::of(relationship.subclass_name.clone()),
        );
        let sub_class_instance = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            TypeValidator::get_instance_of_ctor(reg, ast, sub_class)
        };
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if relationship.r#type == SubclassType::INHERITS
            && super_class_instance_type.is_some_and(|s| !s.is_empty_type(reg))
            && sub_class_instance.is_some_and(|s| !s.is_empty_type(reg))
        {
            let first = n.get_first_child(ast).unwrap();
            if first.is_qualified_name(ast)
                && GOOG_INHERITS.matches(ast, first)
                && sub_class
                    .expect("NullPointerException")
                    .to_maybe_function_type(reg)
                    .and_then(|f| FunctionType::get_source(f, reg))
                    .is_some_and(|s| s.is_class(ast))
            {
                let error = JSError::make(
                    compiler,
                    n,
                    &ES6_CLASS_EXTENDING_CLASS_WITH_GOOG_INHERITS,
                    &[],
                );
                compiler.report(error);
            }
            self.validator.expect_super_type(
                compiler,
                n,
                super_class_instance_type.unwrap(),
                sub_class_instance.unwrap(),
            );
        }
    }

    /// Rust-only: `scope.lookupQualifiedName(qname)` on a TypedScope (StaticTypedScope default
    /// method), run on the scope's canonical registry view.
    fn lookup_qualified_name(
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
        qname: &QualifiedName,
    ) -> Option<TypeId> {
        // Java's first `getSlot(qname.join())` creates the implicit `this`/`super`/`arguments` var
        // of the scope that makes it (AbstractScope#getOwnImplicitSlot, computeIfAbsent); the
        // read-only view only finds implicit vars that already exist, so make them here first.
        let joined = qname.join(compiler);
        let _ = scope.get_slot(compiler, joined);
        let view = scope.as_static_typed_scope(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        view.lookup_qualified_name(reg, ast, qname)
    }

    /// Visits a CALL node.
    ///
    /// `t`: The node traversal object that supplies context, such as the scope chain to use in
    /// name lookups as well as error reporting. `n`: The node being visited.
    // port: TypeCheck#visitCall
    fn visit_call(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        self.check_call_conventions(t, n);

        let compiler = t.get_compiler();
        let child = n.get_first_child(compiler).unwrap();
        let child_js_type = self.get_js_type(compiler, child);
        let (child_type, can_be_called) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let child_type = child_js_type.restrict_by_not_null_or_undefined(reg, ast);
            (child_type, child_type.can_be_called(reg, ast))
        };

        if !can_be_called {
            let type_string = Self::type_to_string(compiler, child_type);
            Self::report(compiler, n, &NOT_CALLABLE, &[&type_string]);
            self.ensure_typed(compiler, n);
            return;
        }

        // A couple of types can be called as if they were functions.
        // If it is a function type, then validate parameters.
        if child_type.is_function_type(compiler.get_type_registry()) {
            let function_type = child_type
                .to_maybe_function_type(compiler.get_type_registry())
                .unwrap();

            // Non-native constructors should not be called directly
            // unless they specify a return type
            let constructor_not_callable = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                function_type.is_constructor(reg)
                    && !function_type.is_native_object_type(reg)
                    && (function_type.get_return_type(reg).is_unknown_type(reg, ast)
                        || function_type.get_return_type(reg).is_void_type(reg))
                    && !n.get_first_child(ast).unwrap().is_super(ast)
            };
            if constructor_not_callable {
                let type_string = Self::type_to_string(compiler, child_type);
                Self::report(compiler, n, &CONSTRUCTOR_NOT_CALLABLE, &[&type_string]);
            }

            // Functions with explicit 'this' types must be called in a GETPROP or GETELEM.
            if function_type.is_ordinary_function(compiler.get_type_registry())
                && !NodeUtil::is_normal_or_opt_chain_get(compiler, child)
            {
                let allowed = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let receiver_type = function_type
                        .get_type_of_this(reg)
                        .expect("NullPointerException");
                    receiver_type.is_unknown_type(reg, ast)
                        || receiver_type.is_all_type(reg)
                        || receiver_type.is_void_type(reg)
                        || (receiver_type.is_object_type(reg, ast)
                            && receiver_type
                                .to_object_type(reg)
                                .unwrap()
                                .is_native_object_type(reg))
                };
                if allowed {
                    // Allow these special cases.
                } else {
                    let type_string = Self::type_to_string(compiler, function_type);
                    Self::report(compiler, n, &EXPECTED_THIS_TYPE, &[&type_string]);
                }
            }

            self.visit_argument_list(compiler, n, function_type);
            let return_type = function_type.get_return_type(compiler.get_type_registry());
            self.ensure_typed_with_type(compiler, n, Some(return_type));
        } else {
            self.ensure_typed(compiler, n);
        }

        // TODO(nicksantos): Add something to check for calls of RegExp objects,
        // which is not supported by IE. Either say something about the return type
        // or warn about the non-portability of the call or both.
    }

    /// Visits the parameters of a CALL or a NEW node.
    // port: TypeCheck#visitArgumentList
    fn visit_argument_list(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        function_type: TypeId,
    ) {
        let parameters = function_type
            .get_parameters(compiler.get_type_registry())
            .into_iter();
        let arguments = NodeUtil::get_invocation_args_as_iterable(compiler, call).into_iter();
        self.check_arguments_match_parameters(
            compiler,
            call,
            function_type,
            arguments,
            parameters,
            0,
        );
    }

    /// Checks that a list of arguments match a list of formal parameters
    ///
    /// If given a TAGGED_TEMPLATE_LIT, the given Iterator should only contain the parameters
    /// corresponding to the actual template lit sub arguments, skipping over the first parameter.
    ///
    /// `first_parameter_index`: The index of the first parameter in the given Iterator in the
    /// function type's parameter list.
    // port: TypeCheck#checkArgumentsMatchParameters
    fn check_arguments_match_parameters(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        function_type: TypeId,
        arguments: impl Iterator<Item = NodeId>,
        mut parameters: impl Iterator<Item = Parameter>,
        first_parameter_index: i32,
    ) {
        let mut spread_argument_count = 0;
        let mut normal_argument_count = first_parameter_index;
        let mut check_argument_type_against_parameter = true;
        let mut parameter: Option<Parameter> = None;
        for argument in arguments {
            // get the next argument

            // Count normal & spread arguments.
            if argument.is_spread(compiler) {
                // we have some form of this case
                // someCall(arg1, arg2, ...firstSpreadExpression, argN, ...secondSpreadExpression)
                spread_argument_count += 1;
                // Once we see a spread parameter, we can no longer match up arguments with
                // parameters.
                check_argument_type_against_parameter = false;
            } else {
                normal_argument_count += 1;
            }

            // Get the next parameter, if we're still matching parameters and arguments.
            if check_argument_type_against_parameter {
                if let Some(next) = parameters.next() {
                    parameter = Some(next);
                } else if parameter.as_ref().is_some_and(Parameter::is_variadic) {
                    // use varargs for all remaining parameters
                } else {
                    // else we ran out of parameters and will report that after this loop
                    parameter = None;
                    check_argument_type_against_parameter = false;
                }
            }

            if check_argument_type_against_parameter {
                let argument_type = self.get_js_type(compiler, argument);
                let parameter_type = parameter
                    .as_ref()
                    .expect("NullPointerException")
                    .get_jstype();
                self.validator.expect_argument_matches_parameter(
                    compiler,
                    argument,
                    argument_type,
                    parameter_type,
                    call,
                    normal_argument_count,
                );
            }
        }

        let reg = compiler.get_type_registry();
        let min_arity = function_type.get_min_arity(reg) as i32;
        let max_arity = function_type.get_max_arity(reg) as i32;
        let max_arity_text = if max_arity == i32::MAX {
            String::new()
        } else {
            format!(" and no more than {max_arity} argument(s)")
        };

        if spread_argument_count > 0 {
            if normal_argument_count > max_arity {
                // We cannot reliably check whether the total argument count is wrong, but we can
                // at least tell if there are more arguments than the function can handle even
                // ignoring the spreads.
                let callee_name = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    reg.get_readable_type_name_no_deref(ast, call.get_first_child(ast).unwrap())
                };
                Self::report(
                    compiler,
                    call,
                    &WRONG_ARGUMENT_COUNT,
                    &[
                        &callee_name,
                        &format!("at least {normal_argument_count}"),
                        &min_arity.to_string(),
                        &max_arity_text,
                    ],
                );
            }
        } else if min_arity > normal_argument_count || max_arity < normal_argument_count {
            let callee_name = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                reg.get_readable_type_name_no_deref(ast, call.get_first_child(ast).unwrap())
            };
            Self::report(
                compiler,
                call,
                &WRONG_ARGUMENT_COUNT,
                &[
                    &callee_name,
                    &normal_argument_count.to_string(),
                    &min_arity.to_string(),
                    &max_arity_text,
                ],
            );
        }
    }

    /// Visits an arrow function expression body.
    // port: TypeCheck#visitImplicitReturnExpression
    fn visit_implicit_return_expression(&mut self, t: &mut NodeTraversal<'_>, expr_node: NodeId) {
        let enclosing_function = t.get_enclosing_function().unwrap();
        let compiler = t.get_compiler();
        let js_type = self.get_js_type(compiler, enclosing_function);
        if js_type.is_function_type(compiler.get_type_registry()) {
            let function_type = js_type
                .to_maybe_function_type(compiler.get_type_registry())
                .unwrap();

            // Java: `if (expectedReturnType == null) expectedReturnType = VOID_TYPE` — the Rust
            // FunctionType#getReturnType is never null, so only the async branch remains.
            let mut expected_return_type =
                function_type.get_return_type(compiler.get_type_registry());
            // if no return type is specified, undefined must be returned
            // (it's a void function)
            if enclosing_function.is_async_function(compiler) {
                // Unwrap the async function's declared return type.
                let (reg, ast) = compiler.get_type_registry_and_ast();
                expected_return_type =
                    Promises::create_async_returnable_type(reg, ast, expected_return_type);
            }

            // Fetch the returned value's type
            let actual_return_type = self.get_js_type(compiler, expr_node);

            self.validator.expect_can_assign_to(
                compiler,
                expr_node,
                actual_return_type,
                expected_return_type,
                "inconsistent return type",
            );
        }
    }

    /// Visits a RETURN node.
    ///
    /// `t`: The node traversal object that supplies context, such as the scope chain to use in
    /// name lookups as well as error reporting. `n`: The node being visited.
    // port: TypeCheck#visitReturn
    fn visit_return(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let enclosing_function = t.get_enclosing_function().unwrap();
        let compiler = t.get_compiler();

        let js_type = self.get_js_type(compiler, enclosing_function);

        if js_type.is_function_type(compiler.get_type_registry()) {
            let function_type = js_type
                .to_maybe_function_type(compiler.get_type_registry())
                .unwrap();

            // Java: `if (returnType == null) returnType = VOID_TYPE` — the Rust
            // FunctionType#getReturnType is never null, so that branch is dropped.
            let mut return_type = function_type.get_return_type(compiler.get_type_registry());
            // if no return type is specified, undefined must be returned
            // (it's a void function)
            if enclosing_function.is_generator_function(compiler) {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                // Unwrap the template variable from a generator function's declared return type.
                // e.g. if returnType is "Generator<string, number, void>", the generator should
                // return "number".
                return_type = JsIterables::get_return_element_type(return_type, reg, ast);

                if enclosing_function.is_async_generator_function(ast) {
                    // Can return x|IThenable<x> in an AsyncGenerator<x>, no await needed. Note
                    // that we must first wrap the type in IThenable as createAsyncReturnableType
                    // will map a non-IThenable to `?`.
                    let wrapped = Promises::wrap_in_i_thenable(reg, ast, return_type);
                    return_type = Promises::create_async_returnable_type(reg, ast, wrapped);
                }
            } else if enclosing_function.is_async_function(compiler) {
                // e.g. `!Promise<string>` => `string|!IThenable<string>`
                // We transform the expected return type rather than the actual return type so
                // that the extual return type is always reported to the user. This was felt to be
                // clearer.
                let (reg, ast) = compiler.get_type_registry_and_ast();
                return_type = Promises::create_async_returnable_type(reg, ast, return_type);
            } else if return_type.is_void_type(compiler.get_type_registry())
                && function_type.is_constructor(compiler.get_type_registry())
            {
                // Allow constructors to use empty returns for flow control.
                if !n.has_children(compiler) {
                    return;
                }

                // Allow constructors to return its own instance type
                return_type =
                    FunctionType::get_instance_type(function_type, compiler.get_type_registry())
                        .expect("NullPointerException");
            }

            // fetching the returned value's type
            let mut value_node = n.get_first_child(compiler);
            let actual_return_type;
            match value_node {
                None => {
                    actual_return_type = self.get_native_type(compiler, VOID_TYPE);
                    value_node = Some(n);
                }
                Some(value) => {
                    actual_return_type = self.get_js_type(compiler, value);
                }
            }

            // verifying
            self.validator.expect_can_assign_to(
                compiler,
                value_node.unwrap(),
                actual_return_type,
                return_type,
                "inconsistent return type",
            );
        }
    }

    /// Visits a YIELD node.
    // port: TypeCheck#visitYield
    fn visit_yield(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let enclosing_function = t.get_enclosing_function().unwrap();
        let compiler = t.get_compiler();
        let js_type = self.get_js_type(compiler, enclosing_function);

        let mut declared_yield_type = self.get_native_type(compiler, UNKNOWN_TYPE);
        if js_type.is_function_type(compiler.get_type_registry()) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let function_type = js_type.to_maybe_function_type(reg).unwrap();
            let return_type = function_type.get_return_type(reg);
            declared_yield_type = JsIterables::get_element_type(return_type, reg, ast);

            if enclosing_function.is_async_generator_function(ast) {
                // Can yield x|IThenable<x> in an AsyncGenerator<x>, no await needed. Note that we
                // must first wrap the type in IThenable as createAsyncReturnableType will map a
                // non-IThenable to `?`.
                let wrapped = Promises::wrap_in_i_thenable(reg, ast, declared_yield_type);
                declared_yield_type = Promises::create_async_returnable_type(reg, ast, wrapped);
            }
        }

        // fetching the yielded value's type
        let mut value_node = n.get_first_child(compiler);
        let mut actual_yield_type;
        match value_node {
            None => {
                actual_yield_type = self.get_native_type(compiler, VOID_TYPE);
                value_node = Some(n);
            }
            Some(value) => {
                actual_yield_type = self.get_js_type(compiler, value);
            }
        }

        if n.is_yield_all(compiler) {
            if enclosing_function.is_async_generator_function(compiler) {
                let maybe_actual_yield_type = self
                    .validator
                    .expect_autoboxes_to_iterable_or_async_iterable(
                        compiler,
                        n,
                        actual_yield_type,
                        "Expression yield* expects an iterable or async iterable",
                    );
                let Some(maybe_actual_yield_type) = maybe_actual_yield_type else {
                    // don't do any further typechecking of the yield* type.
                    return;
                };
                actual_yield_type = maybe_actual_yield_type;
            } else {
                if !self.validator.expect_autoboxes_to_iterable(
                    compiler,
                    n,
                    actual_yield_type,
                    "Expression yield* expects an iterable",
                ) {
                    // don't do any further typechecking of the yield* type.
                    return;
                }
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let template_type = reg.get_iterable_value_template();
                actual_yield_type = actual_yield_type
                    .autobox(reg, ast)
                    .get_template_type_map(reg)
                    .get_resolved_template_type(reg, ast, template_type);
            }
        }

        // verifying
        self.validator.expect_can_assign_to(
            compiler,
            value_node.unwrap(),
            actual_yield_type,
            declared_yield_type,
            "Yielded type does not match declared return type.",
        );
    }

    // port: TypeCheck#visitTaggedTemplateLit
    fn visit_tagged_template_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let tag = n.get_first_child(compiler).unwrap();
        let (tag_type, can_be_called, is_function_type) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let tag_type = tag
                .get_jstype(ast)
                .expect("NullPointerException")
                .restrict_by_not_null_or_undefined(reg, ast);
            (
                tag_type,
                tag_type.can_be_called(reg, ast),
                tag_type.is_function_type(reg),
            )
        };

        if !can_be_called {
            let type_string = Self::type_to_string(compiler, tag_type);
            Self::report(compiler, n, &NOT_CALLABLE, &[&type_string]);
            return;
        } else if !is_function_type {
            // A few types, like the unknown, regexp, and bottom types, can be called as if they
            // are functions. Return if we have one of those types that is not actually a known
            // function.
            return;
        }

        let tag_fn_type = tag_type
            .to_maybe_function_type(compiler.get_type_registry())
            .unwrap();
        let mut parameters = tag_fn_type
            .get_parameters(compiler.get_type_registry())
            .into_iter()
            .peekable();

        // The tag function gets an array of all the template lit substitutions as its first
        // argument, but there's no actual AST node representing that array so we typecheck it
        // separately from the other tag arguments.

        // Validate that the tag function takes at least one parameter
        if parameters.peek().is_none() {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let tag_name = reg.get_readable_type_name_no_deref(ast, tag);
            let args_count = NodeUtil::get_invocation_args_count(ast, n).to_string();
            Self::report(
                compiler,
                n,
                &WRONG_ARGUMENT_COUNT,
                &[
                    &tag_name,
                    &args_count,
                    "0",
                    " and no more than 0 argument(s)",
                ],
            );
            return;
        }

        // Validate that the first parameter is a supertype of ITemplateArray
        let first_parameter = parameters.next().unwrap();
        let parameter_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            first_parameter
                .get_jstype()
                .restrict_by_not_null_or_undefined(reg, ast)
        };
        self.validator.expect_i_template_array_supertype(
            compiler,
            tag,
            parameter_type,
            "Invalid type for the first parameter of tag function",
        );

        // Validate the remaining parameters (the template literal substitutions)
        let arguments = NodeUtil::get_invocation_args_as_iterable(compiler, n).into_iter();
        self.check_arguments_match_parameters(compiler, n, tag_fn_type, arguments, parameters, 1);
    }

    /// This function unifies the type checking involved in the core binary operators and the
    /// corresponding assignment operators. The representation used internally is such that
    /// common code can handle both kinds of operators easily.
    ///
    /// `op`: The operator. `n`: The node being checked.
    // port: TypeCheck#visitBinaryOperator
    fn visit_binary_operator(&mut self, compiler: &mut AbstractCompiler, op: Token, n: NodeId) {
        let operator_type = self.get_js_type(compiler, n);
        let left = n.get_first_child(compiler).unwrap();
        let left_type = self.get_js_type(compiler, left);
        let right = n.get_last_child(compiler).unwrap();
        let right_type = self.get_js_type(compiler, right);
        if operator_type.is_no_type(compiler.get_type_registry()) {
            // An attempt to mix bigint with other types
            let op_str = NodeUtil::op_to_str(n.get_token(compiler)).unwrap_or("null");
            let left_string = Self::type_to_string(compiler, left_type);
            let right_string = Self::type_to_string(compiler, right_type);
            Self::report(
                compiler,
                n,
                &BINARY_OPERATION,
                &[op_str, &left_string, &right_string],
            );
            return;
        }
        let operator_is_number = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            operator_type.is_number(reg, ast)
        };
        match op {
            Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::LSH
            | Token::RSH
            | Token::ASSIGN_URSH
            | Token::URSH => {
                if operator_is_number {
                    // TypeInference set the operator type to 'number', so we know bigint isn't
                    // involved.
                    // NOTE: >>> and >>>= aren't valid operations for bigint, so TypeInference
                    //     ignores the operand types for it and always sets the operator's type to
                    //     'number'. Thus, if one of the operands is a bigint, we'll end up
                    //     reporting that as an error here.
                    let op_str = NodeUtil::op_to_str(n.get_token(compiler)).unwrap_or("null");
                    let left_matches = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        left_type.matches_number_context(reg, ast)
                    };
                    if !left_matches {
                        let left_string = Self::type_to_string(compiler, left_type);
                        Self::report(compiler, left, &BIT_OPERATION, &[op_str, &left_string]);
                    } else {
                        self.validator.expect_number_strict(
                            compiler,
                            n,
                            left_type,
                            &format!("operator {op_str}"),
                        );
                    }
                    let right_matches = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        right_type.matches_number_context(reg, ast)
                    };
                    if !right_matches {
                        let right_string = Self::type_to_string(compiler, right_type);
                        Self::report(compiler, right, &BIT_OPERATION, &[op_str, &right_string]);
                    } else {
                        self.validator.expect_number_strict(
                            compiler,
                            n,
                            right_type,
                            &format!("operator {op_str}"),
                        );
                    }
                } else {
                    self.validator.expect_big_int_or_number(
                        compiler,
                        left,
                        left_type,
                        "left operand",
                    );
                    self.validator.expect_big_int_or_number(
                        compiler,
                        right,
                        right_type,
                        "right operand",
                    );
                }
            }
            Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_MUL
            | Token::ASSIGN_SUB
            | Token::ASSIGN_EXPONENT
            | Token::DIV
            | Token::MOD
            | Token::MUL
            | Token::SUB
            | Token::EXPONENT => {
                if operator_is_number {
                    // TypeInference set the operator type to 'number', so we know bigint isn't
                    // involved.
                    self.validator
                        .expect_number(compiler, left, left_type, "left operand");
                    self.validator
                        .expect_number(compiler, right, right_type, "right operand");
                } else {
                    self.validator.expect_big_int_or_number(
                        compiler,
                        left,
                        left_type,
                        "left operand",
                    );
                    self.validator.expect_big_int_or_number(
                        compiler,
                        right,
                        right_type,
                        "right operand",
                    );
                }
            }
            Token::ASSIGN_BITAND
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITOR
            | Token::BITAND
            | Token::BITXOR
            | Token::BITOR => {
                if operator_is_number {
                    // This condition is meant to catch any old cases (where bigint isn't involved)
                    self.validator.expect_bitwiseable(
                        compiler,
                        left,
                        left_type,
                        "bad left operand to bitwise operator",
                    );
                    self.validator.expect_bitwiseable(
                        compiler,
                        right,
                        right_type,
                        "bad right operand to bitwise operator",
                    );
                } else {
                    self.validator.expect_big_int_or_number(
                        compiler,
                        left,
                        left_type,
                        "bad left operand to bitwise operator",
                    );
                    self.validator.expect_big_int_or_number(
                        compiler,
                        right,
                        right_type,
                        "bad right operand to bitwise operator",
                    );
                }
            }
            Token::ASSIGN_ADD | Token::ADD => {}
            _ => Self::report(compiler, n, &UNEXPECTED_TOKEN, &[&op.to_string()]),
        }
        self.ensure_typed(compiler, n);
    }

    /// Validates the implicit assignment to the global for a legacy goog.module
    // port: TypeCheck#visitModuleBody
    fn visit_module_body(&mut self, t: &mut NodeTraversal<'_>, module_body: NodeId) {
        let compiler = t.get_compiler();
        let associated_module = {
            let module_map = compiler.get_module_map().cloned();
            ModuleImportResolver::get_module_from_scope_root(
                module_map.as_deref(),
                compiler,
                module_body,
            )
        }
        .expect("NullPointerException");
        if !associated_module.metadata().is_legacy_goog_module() {
            return;
        }
        let module_name = QualifiedName::of(
            associated_module
                .closure_namespace()
                .expect("NullPointerException")
                .clone(),
        );
        let goog_module_call = module_body.get_first_child(compiler).unwrap();
        let top_scope = self.top_scope.unwrap();
        let module_body_type = module_body
            .get_jstype(compiler)
            .expect("NullPointerException");
        if module_name.is_simple(compiler) {
            let joined = module_name.join(compiler);
            let global_var = top_scope
                .get_var(compiler, joined)
                .expect("NullPointerException");
            let global_type = global_var.get_type(compiler).expect("NullPointerException");
            self.validator.expect_can_assign_to(
                compiler,
                goog_module_call,
                module_body_type,
                global_type,
                "legacy goog.module export",
            );
        } else {
            let owner = module_name
                .get_owner(compiler)
                .expect("NullPointerException");
            let parent_type = Self::lookup_qualified_name(compiler, top_scope, &owner);
            let parent_object_type =
                parent_type.and_then(|p| p.to_maybe_object_type(compiler.get_type_registry()));
            let Some(parent_object_type) = parent_object_type else {
                return;
            };
            let component = module_name.get_component(compiler);
            let property_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                parent_object_type.get_property_type(reg, ast, component.clone())
            };
            let type_name_supplier =
                |c: &mut AbstractCompiler| owner.join(&c.ast).to_string_lossy();
            self.validator.expect_can_assign_to_property_of_type(
                compiler,
                goog_module_call,
                module_body_type,
                property_type,
                parent_object_type,
                &type_name_supplier,
                &component.to_string_lossy(),
            );
        }
    }

    /// Validates that a dynamic import statement has a single child of type string
    // port: TypeCheck#visitDynamicImport
    fn visit_dynamic_import(&mut self, t: &mut NodeTraversal<'_>, dynamic_import: NodeId) {
        let compiler = t.get_compiler();
        let promise_type = self.get_native_type(compiler, PROMISE_TYPE);
        self.ensure_typed_with_type(compiler, dynamic_import, Some(promise_type));

        let import_specifier = dynamic_import.get_first_child(compiler).unwrap();
        let import_specifier_type = import_specifier.get_jstype(compiler);
        match import_specifier_type {
            None => self.ensure_typed_native(compiler, import_specifier, STRING_TYPE),
            Some(import_specifier_type) => {
                let string_type = self.get_native_type(compiler, STRING_TYPE);
                self.validator.expect_not_null_or_undefined(
                    t,
                    import_specifier,
                    import_specifier_type,
                    "dynamic import specifier",
                    string_type,
                );
            }
        }
    }

    /// Checks enum aliases.
    ///
    /// We verify that the enum element type of the enum used for initialization is a subtype of
    /// the enum element type of the enum the value is being copied in.
    ///
    /// Example:
    ///
    /// ```text
    /// var myEnum = myOtherEnum;
    /// ```
    ///
    /// Enum aliases are irregular, so we need special code for this :(
    ///
    /// `value_type`: the type of the value used for initialization of the enum.
    /// `node_to_warn`: the node on which to issue warnings on.
    // port: TypeCheck#checkEnumAlias
    fn check_enum_alias(
        &mut self,
        t: &mut NodeTraversal<'_>,
        decl_info: Option<&JSDocInfo>,
        value_type: TypeId,
        node_to_warn: NodeId,
    ) {
        let Some(decl_info) = decl_info else {
            return;
        };
        if !decl_info.has_enum_parameter_type() {
            return;
        }

        if !value_type.is_enum_type(t.get_compiler().get_type_registry()) {
            return;
        }

        let scope = t.get_typed_scope();
        let compiler = t.get_compiler();
        let reg = compiler.get_type_registry();
        let value_enum_type = value_type.to_maybe_enum_type(reg).unwrap();
        let value_enum_primitive_type =
            EnumType::get_elements_type(value_enum_type, reg).get_primitive_type(reg);
        let scope_view = scope.as_static_typed_scope_arc(compiler);
        let declared = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            decl_info
                .get_enum_parameter_type()
                .expect("NullPointerException")
                .evaluate(reg, ast, Some(scope_view))
        };
        self.validator.expect_can_assign_to(
            compiler,
            node_to_warn,
            value_enum_primitive_type,
            declared,
            "incompatible enum element types",
        );
    }

    /// This method gets the JSType from the Node argument and verifies that it is present.
    // port: TypeCheck#getJSType
    fn get_js_type(&self, compiler: &mut AbstractCompiler, n: NodeId) -> TypeId {
        let js_type = n.get_jstype(compiler);
        match js_type {
            None => {
                // TODO(nicksantos): This branch indicates a compiler bug, not worthy of
                // halting the compilation but we should log this and analyze to track
                // down why it happens. This is not critical and will be resolved over
                // time as the type checker is extended.
                self.get_native_type(compiler, UNKNOWN_TYPE)
            }
            Some(js_type) => js_type,
        }
    }

    /// Returns the type of the property with the given name if declared. Otherwise returns
    /// unknown.
    // port: TypeCheck#getPropertyTypeIfDeclared
    fn get_property_type_if_declared(
        &self,
        compiler: &mut AbstractCompiler,
        object_type: Option<TypeId>,
        property_name: PropertyKey,
    ) -> TypeId {
        if let Some(object_type) = object_type {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if JSType::has_property(object_type, reg, ast, property_name.clone())
                && !object_type.is_property_type_inferred(reg, ast, property_name.clone())
            {
                return object_type.get_property_type(reg, ast, property_name);
            }
        }
        self.get_native_type(compiler, UNKNOWN_TYPE)
    }

    // TODO(nicksantos): TypeCheck should never be attaching types to nodes.
    // All types should be attached by TypeInference. This is not true today
    // for legacy reasons. There are a number of places where TypeInference
    // doesn't attach a type, as a signal to TypeCheck that it needs to check
    // that node's type.

    /// Ensure that the given node has a type. If it does not have one, attach the UNKNOWN_TYPE.
    // port: TypeCheck#ensureTyped(Node)
    fn ensure_typed(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let unknown = self.get_native_type(compiler, UNKNOWN_TYPE);
        self.ensure_typed_with_type(compiler, n, Some(unknown));
    }

    // port: TypeCheck#ensureTyped(Node,JSTypeNative)
    fn ensure_typed_native(&self, compiler: &mut AbstractCompiler, n: NodeId, type_: JSTypeNative) {
        let native = self.get_native_type(compiler, type_);
        self.ensure_typed_with_type(compiler, n, Some(native));
    }

    /// Ensures the node is typed.
    // port: TypeCheck#ensureTyped(Node,JSType)
    fn ensure_typed_with_type(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: Option<TypeId>,
    ) {
        // Make sure FUNCTION nodes always get function type.
        check_state!(
            !n.is_function(compiler) || {
                let type_ = type_.expect("NullPointerException");
                let (reg, ast) = compiler.get_type_registry_and_ast();
                type_.is_function_type(reg) || type_.is_unknown_type(reg, ast)
            }
        );
        if n.get_jstype(compiler).is_none() {
            n.set_jstype(compiler, type_);
        }
    }

    /// Returns the percentage of nodes typed by the type checker: a number between 0.0 and 100.0.
    // port: TypeCheck#getTypedPercent
    pub fn get_typed_percent(&self) -> f64 {
        let total = self.null_count + self.unknown_count + self.typed_count;
        if total == 0 {
            0.0
        } else {
            (100.0 * f64::from(self.typed_count)) / f64::from(total)
        }
    }

    // port: TypeCheck#getNativeType
    fn get_native_type(&self, compiler: &mut AbstractCompiler, type_id: JSTypeNative) -> TypeId {
        compiler.get_type_registry().get_native_type(type_id)
    }

    /// Checks if current node contains js docs and checks all types specified in the js doc
    /// whether they have Objects with potentially invalid keys. For example: `Object<!Object,
    /// number>`. If such type is found, a warning is reported for the current node.
    // port: TypeCheck#checkJsdocInfoContainsObjectWithBadKey
    fn check_jsdoc_info_contains_object_with_bad_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) {
        if n.get_jsdoc_info_ref(compiler).is_some() {
            let info = n.get_jsdoc_info(compiler).unwrap();
            self.check_type_contains_object_with_bad_key(compiler, n, info.get_type());
            self.check_type_contains_object_with_bad_key(compiler, n, info.get_return_type());
            self.check_type_contains_object_with_bad_key(compiler, n, info.get_typedef_type());
            for param in info.get_parameter_names() {
                let param_type = info.get_parameter_type(param);
                self.check_type_contains_object_with_bad_key(compiler, n, param_type);
            }
        }
    }

    // port: TypeCheck#checkTypeContainsObjectWithBadKey
    fn check_type_contains_object_with_bad_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: Option<Arc<JSTypeExpression>>,
    ) {
        if let Some(type_) = type_
            && let Some(real_type) = type_.get_root().get_jstype(compiler)
        {
            let object_with_bad_key =
                self.find_object_with_non_stringifiable_key(compiler, real_type, &mut Vec::new());
            if let Some(object_with_bad_key) = object_with_bad_key {
                let s = Self::type_to_string(compiler, object_with_bad_key);
                Self::report(compiler, n, &NON_STRINGIFIABLE_OBJECT_KEY, &[&s]);
            }
        }
    }

    /// Checks whether type is useful as the key of an object property access. This means it
    /// should be either stringifiable or a symbol. Stringifiable types are types that can be
    /// converted to string and give unique results for different objects. For example objects
    /// have native toString() method that on chrome returns "[object Object]" for all objects
    /// making it useless when used as keys. At the same time native types like numbers can be
    /// safely converted to strings and used as keys. Also user might have provided custom
    /// toString() methods for a class making it suitable for using as key.
    // port: TypeCheck#isReasonableObjectPropertyKey
    fn is_reasonable_object_property_key(
        &self,
        compiler: &mut AbstractCompiler,
        type_: TypeId,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        // Check built-in types
        if type_.is_unknown_type(reg, ast)
            || type_.is_number(reg, ast)
            || type_.is_string(reg, ast)
            || type_.is_symbol(reg, ast)
            || type_.is_boolean_object_type(reg)
            || type_.is_boolean_value_type(reg)
            || type_.is_date_type(reg)
            || type_.is_regexp_type(reg)
            || type_.is_interface(reg)
            || type_.is_record_type(reg)
            || type_.is_null_type(reg)
            || type_.is_void_type(reg)
        {
            return true;
        }

        // For enums check that underlying type is stringifiable.
        if let Some(enum_element) = type_.to_maybe_enum_element_type(reg) {
            let primitive = enum_element.get_primitive_type(reg);
            return self.is_reasonable_object_property_key(compiler, primitive);
        }

        // Array is stringifiable if it doesn't have template type or if it does have it, the
        // template type must be also stringifiable.
        // Good: Array, Array.<number>
        // Bad: Array.<!Object>
        if type_.is_array_type(reg) {
            return true;
        }
        if type_.is_templatized_type(reg) {
            let templatized_type = type_.to_maybe_templatized_type(reg).unwrap();
            if TemplatizedType::get_referenced_type(templatized_type, reg).is_array_type(reg) {
                let first = ObjectType::get_template_types(templatized_type, reg)
                    .expect("NullPointerException")[0];
                return self.is_reasonable_object_property_key(compiler, first);
            }
        }

        // Named types are usually @typedefs. For such types we need to check underlying type
        // specified in @typedef annotation.
        if let Some(named_type) = type_.to_maybe_named_type(reg)
            && named_type == type_
        {
            let referenced = NamedType::get_referenced_type(named_type, reg);
            return self.is_reasonable_object_property_key(compiler, referenced);
        }

        // For union type every alternate must be stringifiable.
        if type_.is_union_type(reg) {
            let alternates = type_
                .to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast);
            for &alternate_type in alternates.iter() {
                if !self.is_reasonable_object_property_key(compiler, alternate_type) {
                    return false;
                }
            }
            return true;
        }

        // Handle interfaces and classes.
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if type_.is_object(reg, ast) {
            let object_type = type_.to_maybe_object_type(reg).unwrap();
            let constructor = object_type.get_constructor(reg);
            // Interfaces considered stringifiable as user might implement toString() method in
            // classes-implementations.
            if let Some(constructor) = constructor
                && constructor.is_interface(reg)
            {
                return true;
            }
            // This is user-defined class so check if it has custom toString() method.
            return self.class_has_to_string(compiler, object_type);
        }
        false
    }

    /// Checks whether current type is Object type with non-stringifable key.
    // port: TypeCheck#isObjectTypeWithNonStringifiableKey
    fn is_object_type_with_non_stringifiable_key(
        &self,
        compiler: &mut AbstractCompiler,
        type_: TypeId,
    ) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !type_.is_templatized_type(reg) {
            // TODO(nickreid): Why don't we care about types like `Foo extends Object<Bar, Qux>`?
            return false;
        }

        let template_type_map = type_.get_template_type_map(reg);
        let object_index_key = reg.get_object_index_key();
        if template_type_map.has_template_key(object_index_key) {
            let resolved = template_type_map.get_resolved_template_type(reg, ast, object_index_key);
            !self.is_reasonable_object_property_key(compiler, resolved)
        } else {
            false
        }
    }

    /// Checks whether type (or one of its component if is composed type like union or
    /// templatized type) has Object with non-stringifiable key. For example `Object.<!Object,
    /// number>`.
    ///
    /// Returns non-stringifiable type which is used as key or null if all there are no such
    /// types. Java's `Set<JSType>` membership uses `JSType#equals`; Rust keeps the insertion
    /// ordered list and tests membership with `equals`.
    // port: TypeCheck#findObjectWithNonStringifiableKey
    fn find_object_with_non_stringifiable_key(
        &self,
        compiler: &mut AbstractCompiler,
        type_: TypeId,
        already_checked_types: &mut Vec<TypeId>,
    ) -> Option<TypeId> {
        let already_checked = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            already_checked_types
                .iter()
                .any(|&checked| checked.equals(reg, ast, type_))
        };
        if already_checked {
            // This can happen in recursive types. Current type already being checked earlier in
            // stacktrace so now we just skip it.
            return None;
        } else {
            already_checked_types.push(type_);
        }
        if self.is_object_type_with_non_stringifiable_key(compiler, type_) {
            return Some(type_);
        }
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if type_.is_union_type(reg) {
            let alternates = type_
                .to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast);
            for &alternate_type in alternates.iter() {
                let result = self.find_object_with_non_stringifiable_key(
                    compiler,
                    alternate_type,
                    already_checked_types,
                );
                if result.is_some() {
                    return result;
                }
            }
        }
        let reg = compiler.get_type_registry();
        if type_.is_templatized_type(reg) {
            let templatized = type_.to_maybe_templatized_type(reg).unwrap();
            let template_types =
                ObjectType::get_template_types(templatized, reg).expect("NullPointerException");
            for template_type in template_types {
                let result = self.find_object_with_non_stringifiable_key(
                    compiler,
                    template_type,
                    already_checked_types,
                );
                if result.is_some() {
                    return result;
                }
            }
        }
        let reg = compiler.get_type_registry();
        if type_.is_ordinary_function(reg) {
            let function = type_.to_maybe_function_type(reg).unwrap();
            for parameter in function.get_parameters(reg) {
                let result = self.find_object_with_non_stringifiable_key(
                    compiler,
                    parameter.get_jstype(),
                    already_checked_types,
                );
                if result.is_some() {
                    return result;
                }
            }
            let return_type = function.get_return_type(compiler.get_type_registry());
            return self.find_object_with_non_stringifiable_key(
                compiler,
                return_type,
                already_checked_types,
            );
        }
        None
    }

    /// Checks whether class has overridden toString() method. All objects has native toString()
    /// method but we ignore it as it is not useful so we need user-provided toString() method.
    // port: TypeCheck#classHasToString
    fn class_has_to_string(&self, compiler: &mut AbstractCompiler, type_: TypeId) -> bool {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let to_string_property = type_.get_own_slot(reg, ast, "toString");
        if let Some(to_string_property) = to_string_property {
            return to_string_property.get_type(reg).is_function_type(reg);
        }
        let parent = type_.get_implicit_prototype(reg, ast);
        if let Some(parent) = parent
            && !parent.is_native_object_type(reg)
        {
            return self.class_has_to_string(compiler, parent);
        }
        false
    }

    /// Given the LHS of a property or element assignment, checks that the type that we're
    /// assigning into is not readonly (ReadonlyArray, in particular).
    ///
    /// This is basically unsound since it checks just "ReadonlyArray" so casting up to
    /// "Iterable" or down to "Array" will defeat it; but it should catch basic errors.
    // port: TypeCheck#checkNotReadonlyPropertyAssignment
    fn check_not_readonly_property_assignment(&self, compiler: &mut AbstractCompiler, lhs: NodeId) {
        // We only care about element or property assignments.
        if !lhs.is_get_prop(compiler) && !lhs.is_get_elem(compiler) {
            return;
        }

        // If we do not have type information, drop out.
        let lhs_type = lhs.get_first_child(compiler).unwrap().get_jstype(compiler);
        let Some(lhs_type) = lhs_type else {
            return;
        };

        // We could be a reference to "ReadonlyArray" or a templatized wrapper.
        let ro_array = self.get_native_type(compiler, READONLY_ARRAY_TYPE);
        let alternates = Self::flatten_union(compiler, lhs_type);
        for &type_ in alternates.iter() {
            let matches = {
                let referenced = Self::maybe_referenced_type(compiler, type_);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                ro_array.equals(reg, ast, type_) || ro_array.equals(reg, ast, referenced)
            };
            if matches {
                let s = Self::type_to_string(compiler, type_);
                Self::report(compiler, lhs, &PROPERTY_ASSIGNMENT_TO_READONLY_VALUE, &[&s]);
                break;
            }
        }
    }

    // port: TypeCheck#maybeReferencedType
    fn maybe_referenced_type(compiler: &mut AbstractCompiler, type_: TypeId) -> Option<TypeId> {
        let reg = compiler.get_type_registry();
        let maybe_templatized = type_.to_maybe_templatized_type(reg);
        maybe_templatized.map(|t| TemplatizedType::get_referenced_type(t, reg))
    }

    // port: TypeCheck#flattenUnion
    fn flatten_union(compiler: &mut AbstractCompiler, maybe_union: TypeId) -> Arc<Vec<TypeId>> {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let union = maybe_union.to_maybe_union_type(reg);
        match union {
            None => Arc::new(vec![maybe_union]),
            Some(union) => union.get_alternates(reg, ast),
        }
    }

    // port: TypeCheck#declaresOverride
    fn declares_override(jsdoc: Option<&JSDocInfo>) -> bool {
        jsdoc.is_some_and(JSDocInfo::is_override)
    }

    /// Rust-only: Java's `type.toString()`.
    fn type_to_string(compiler: &mut AbstractCompiler, type_: TypeId) -> String {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        JSType::to_string(type_, reg, ast)
    }
}

impl CompilerPass for TypeCheck {
    // port: TypeCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs_root: NodeId, js_root: NodeId) {
        self.process_nullable(compiler, Some(externs_root), js_root);
    }
}

impl Callback for TypeCheck {
    // port: TypeCheck#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        self.should_traverse_impl(t, n, parent)
    }

    // port: TypeCheck#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_impl(t, n, parent);
    }
}

/// Rust-only: the TypeCheck's `TypedScopeCreator` as the NodeTraversal's scope creator (Java
/// hands the same object to `NodeTraversal.Builder#setScopeCreator`).
struct TypedScopeCreatorAdapter<'a>(&'a mut TypedScopeCreator);

impl ScopeCreator for TypedScopeCreatorAdapter<'_> {
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        let parent = parent.map(AbstractScopeHandle::from);
        self.create_abstract_scope(compiler, n, parent)
            .untyped(compiler)
    }

    // port: TypedScopeCreator#createScope(Node,AbstractScope)
    fn create_abstract_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<AbstractScopeHandle>,
    ) -> AbstractScopeHandle {
        // checkArgument(parent == null || parent instanceof TypedScope)
        let parent = parent.map(|parent| parent.typed(compiler));
        AbstractScopeHandle::Typed(self.0.create_scope(compiler, n, parent))
    }
}

/// Logs types for @logTypeInCompiler.
// port: TypeCheck.DebugTypeLogger
struct DebugTypeLogger {
    /// Output log file for @logTypeInCompiler.
    type_log_file: Option<Box<dyn LogFile>>,

    /// The node where we started logging, if we're logging.
    parent_node: Option<NodeId>,
}

impl DebugTypeLogger {
    // port: TypeCheck.DebugTypeLogger#DebugTypeLogger
    fn new(compiler: &AbstractCompiler) -> Self {
        let type_log_file = if compiler.is_debug_logging_enabled() {
            Some(compiler.create_or_reopen_log(
                "DebugTypeLogger",
                "types_logged_in_compiler.log",
                &[],
            ))
        } else {
            None
        };
        Self {
            type_log_file,
            parent_node: None,
        }
    }

    // port: TypeCheck.DebugTypeLogger#maybeStartLoggingAt
    fn maybe_start_logging_at(&mut self, compiler: &AbstractCompiler, n: NodeId) {
        // We don't log outside debug mode.
        if !compiler.is_debug_logging_enabled() {
            return;
        }

        // We're already logging.
        if self.parent_node.is_some() {
            return;
        }

        // We only start logging on @logTypeInCompiler.
        let info = n.get_jsdoc_info(compiler);
        if info.is_none() || !info.unwrap().get_log_type_in_compiler() {
            return;
        }

        // Otherwise start logging here.
        self.parent_node = Some(n);
    }

    // port: TypeCheck.DebugTypeLogger#stopLoggingIfThisIsWhereWeStarted
    fn stop_logging_if_this_is_where_we_started(&mut self, n: NodeId) {
        if Some(n) == self.parent_node {
            self.parent_node = None;
        }
    }

    // port: TypeCheck.DebugTypeLogger#close
    fn close(&mut self) {
        if let Some(type_log_file) = self.type_log_file.as_mut() {
            type_log_file.close();
        }
    }

    // port: TypeCheck.DebugTypeLogger#maybeLogTypeOfNode
    fn maybe_log_type_of_node(&mut self, compiler: &AbstractCompiler, n: NodeId) {
        // If we have no parent, we're not logging.
        if self.parent_node.is_none() {
            return;
        }

        if n.get_jstype(compiler).is_none() {
            return;
        }

        let text = n.to_string_with_options(
            compiler, /* printSource= */ true, /* printAnnotations= */ true,
            /* printType= */ true,
        );
        self.type_log_file
            .as_mut()
            .expect("NullPointerException")
            .log_string(&text);
    }
}
