/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/SourceMapSupplier.java.

use closure_rhino::js_string::JsString;
pub trait SourceMapSupplier {
    // port: SourceMapSupplier#getSourceMap
    fn get_source_map(&self, url: &JsString) -> Result<Option<JsString>, std::io::Error>;
}
impl<F: Fn(&JsString) -> Result<Option<JsString>, std::io::Error>> SourceMapSupplier for F {
    // Rust closure adapter.
    fn get_source_map(&self, url: &JsString) -> Result<Option<JsString>, std::io::Error> {
        self(url)
    }
}
