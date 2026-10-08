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
//   src/com/google/javascript/jscomp/ClosurePrimitiveErrors.java.

//! Common errors for Closure primitives that are reported in multiple passses.

use crate::diagnostic_type::DiagnosticType;

// port: ClosurePrimitiveErrors#DUPLICATE_MODULE
pub static DUPLICATE_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_DUPLICATE_MODULE",
    "Duplicate module: {0}. Module already declared in other file {1}",
);

// port: ClosurePrimitiveErrors#DUPLICATE_NAMESPACE
pub static DUPLICATE_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_DUPLICATE_NAMESPACE",
    "Duplicate namespace: {0}. Namespace already provided in other file {1}",
);

// port: ClosurePrimitiveErrors#DUPLICATE_NAMESPACE_AND_MODULE
pub static DUPLICATE_NAMESPACE_AND_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_DUPLICATE_NAMESPACE_AND_MODULE",
    "Duplicate module: {0}. Module already goog.provided in other file {1}",
);

// port: ClosurePrimitiveErrors#INVALID_DESTRUCTURING_FORWARD_DECLARE
pub static INVALID_DESTRUCTURING_FORWARD_DECLARE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DESTRUCTURING_FORWARD_DECLARE",
    "Cannot destructure a forward-declared type",
);

// port: ClosurePrimitiveErrors#MODULE_USES_GOOG_MODULE_GET
pub static MODULE_USES_GOOG_MODULE_GET: DiagnosticType = DiagnosticType::error(
    "JSC_MODULE_USES_GOOG_MODULE_GET",
    "It's illegal to use a 'goog.module.get' at the module top-level. Did you mean to use goog.require instead?",
);

// port: ClosurePrimitiveErrors#INVALID_FORWARD_DECLARE_NAMESPACE
pub static INVALID_FORWARD_DECLARE_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_FORWARD_DECLARE_NAMESPACE",
    "goog.forwardDeclare parameter must be a string literal.",
);

// port: ClosurePrimitiveErrors#INVALID_GET_NAMESPACE
pub static INVALID_GET_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_GET_NAMESPACE",
    "goog.module.get parameter must be a string literal.",
);

// port: ClosurePrimitiveErrors#INVALID_REQUIRE_DYNAMIC
pub static INVALID_REQUIRE_DYNAMIC: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_REQUIRE_DYNAMIC_NAMESPACE",
    "goog.requireDynamic parameter must be a string literal.",
);

// port: ClosurePrimitiveErrors#INVALID_REQUIRE_NAMESPACE
pub static INVALID_REQUIRE_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_REQUIRE_NAMESPACE",
    "goog.require parameter must be a string literal.",
);

// port: ClosurePrimitiveErrors#INVALID_REQUIRE_TYPE_NAMESPACE
pub static INVALID_REQUIRE_TYPE_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_REQUIRE_TYPE_NAMESPACE",
    "goog.requireType parameter must be a string literal.",
);

// port: ClosurePrimitiveErrors#MISSING_MODULE_OR_PROVIDE
pub static MISSING_MODULE_OR_PROVIDE: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_MODULE_OR_PROVIDE",
    "Required namespace \"{0}\" never defined.",
);

// port: ClosurePrimitiveErrors#MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE
pub static MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE",
    "Required namespace \"{0}\" never defined.",
);

// port: ClosurePrimitiveErrors#INVALID_GET_CALL_SCOPE
pub static INVALID_GET_CALL_SCOPE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_GET_CALL_SCOPE",
    "goog.module.get values should not be assigned to global variables.",
);

// port: ClosurePrimitiveErrors#INVALID_CLOSURE_CALL_SCOPE_ERROR
pub static INVALID_CLOSURE_CALL_SCOPE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_CLOSURE_CALL_ERROR",
    "Closure primitive methods (goog.provide, goog.require, goog.define, etc) must be called at file scope.",
);

// port: ClosurePrimitiveErrors#TOO_MANY_ARGUMENTS_ERROR
pub static TOO_MANY_ARGUMENTS_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_TOO_MANY_ARGUMENTS_ERROR",
    "method \"{0}\" called with more than one argument",
);

// port: ClosurePrimitiveErrors#NULL_ARGUMENT_ERROR
pub static NULL_ARGUMENT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_NULL_ARGUMENT_ERROR",
    "method \"{0}\" called without an argument",
);

// port: ClosurePrimitiveErrors#INVALID_ARGUMENT_ERROR
pub static INVALID_ARGUMENT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_ARGUMENT_ERROR",
    "method \"{0}\" called with invalid argument",
);

// port: ClosurePrimitiveErrors#GOOG_MODULE_GET_OF_WEAK_MODULE
pub static GOOG_MODULE_GET_OF_WEAK_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_GET_OF_WEAK_MODULE",
    "goog.module.get of \"{0}\" references a namespace that is only imported by goog.requireType.",
);
