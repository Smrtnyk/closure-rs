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
//   src/com/google/javascript/jscomp/transpile/Transpiler.java.

use super::transpile_result::TranspileResult;
use closure_rhino::{java_lang::uri::URI, js_string::JsString};
use std::sync::{Arc, LazyLock};
pub trait Transpiler: Send + Sync {
    // port: Transpiler#transpile
    fn transpile(&self, path: URI, code: &JsString) -> TranspileResult;
    // port: Transpiler#runtime
    fn runtime(&self) -> JsString;
}
struct NullTranspiler;
impl Transpiler for NullTranspiler {
    // port: Transpiler.NULL#transpile
    fn transpile(&self, path: URI, code: &JsString) -> TranspileResult {
        TranspileResult::new(path, code.clone(), code.clone(), "")
    }
    // port: Transpiler.NULL#runtime
    fn runtime(&self) -> JsString {
        JsString::from("")
    }
}
pub static NULL: LazyLock<Arc<dyn Transpiler>> = LazyLock::new(|| Arc::new(NullTranspiler));
