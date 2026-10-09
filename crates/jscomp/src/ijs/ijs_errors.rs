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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ijs/IjsErrors.java.

//! Port of `com.google.javascript.jscomp.ijs.IjsErrors`: errors related to `.i.js` files.

use crate::diagnostic_type::DiagnosticType;

/// Java `IjsErrors` is a constants holder with a private constructor.
pub struct IjsErrors;

// port: IjsErrors#BAD_IJS_FILE_NAME
pub static BAD_IJS_FILE_NAME: DiagnosticType = DiagnosticType::error(
    "JSC_BAD_IJS_FILE_NAME",
    "--ijs file name must end in .i.js: {0}",
);

// port: IjsErrors#CONFLICTING_IJS_FILE
pub static CONFLICTING_IJS_FILE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONFLICTING_IJS_FILE",
    concat!(
        "Found --ijs file in a conflict with --js or --weakdep file. This usually means the same",
        " file appears both in the srcs and the transitive deps of a build target.\n",
        "File 1: {0}\n",
        "File 2: {1}"
    ),
);
