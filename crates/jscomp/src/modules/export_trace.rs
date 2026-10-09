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
//   src/com/google/javascript/jscomp/modules/ExportTrace.java.

//! Simple class to keep track of which modules and exports have been visited when resolving
//! exports.
use crate::modules::unresolved_module::UnresolvedModuleId;
use closure_rhino::js_string::JsString;

/// Simple class to keep track of which modules and exports have been visited when resolving
/// exports. It is invalid to visit the same (module, name) pair more than once when resolving an
/// export (invalid cycle).
///
/// Used for its equals / hashCode in a set; `UnresolvedModule` has reference equality, which is
/// handle equality here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExportTrace {
    module: UnresolvedModuleId,
    export_name: JsString,
}

impl ExportTrace {
    // port: ExportTrace#create
    pub fn create(module: UnresolvedModuleId, export_name: JsString) -> Self {
        Self {
            module,
            export_name,
        }
    }

    // port: ExportTrace#module
    pub fn module(&self) -> UnresolvedModuleId {
        self.module
    }

    // port: ExportTrace#exportName
    pub fn export_name(&self) -> &JsString {
        &self.export_name
    }
}
