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
//   src/com/google/javascript/jscomp/modules/GoogEsImports.java.

//! Utilties for ES imports for Closure files, for example `import array from 'goog:goog.array';`.
use closure_rhino::js_string::JsString;

/// Utilties for ES imports for Closure files.
pub struct GoogEsImports;

impl GoogEsImports {
    // port: GoogEsImports#GOOG_IMPORT_PREFIX
    const GOOG_IMPORT_PREFIX: &'static str = "goog:";

    // port: GoogEsImports#isGoogImportSpecifier
    pub fn is_goog_import_specifier(module_specifier: &JsString) -> bool {
        module_specifier.starts_with(JsString::from(Self::GOOG_IMPORT_PREFIX))
    }

    // port: GoogEsImports#getClosureIdFromGoogImportSpecifier
    pub fn get_closure_id_from_goog_import_specifier(module_specifier: &JsString) -> JsString {
        module_specifier.substring_from(Self::GOOG_IMPORT_PREFIX.len())
    }
}
