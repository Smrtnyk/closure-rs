/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PolymerPassErrors.java.

// STAND-IN: PolymerPassErrors belongs to the Polymer passes, out of scope (docs/PORTING.md §2); not ported

use crate::diagnostic_type::DiagnosticType;

// port: PolymerPassErrors#POLYMER_DESCRIPTOR_NOT_VALID
pub static POLYMER_DESCRIPTOR_NOT_VALID: DiagnosticType = DiagnosticType::warning(
    "JSC_POLYMER_DESCRIPTOR_NOT_VALID",
    "The argument to Polymer() is not an obj lit or the Polymer 2 class does not have a static getter named 'config'. Ignoring this definition.",
);

// port: PolymerPassErrors#POLYMER_UNQUALIFIED_BEHAVIOR
pub static POLYMER_UNQUALIFIED_BEHAVIOR: DiagnosticType = DiagnosticType::error(
    "JSC_POLYMER_UNQUALIFIED_BEHAVIOR",
    "Behaviors must be global names or qualified names that are declared as object literals or array literals of other valid Behaviors.",
);
