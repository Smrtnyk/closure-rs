/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/MessageFormatter.java.

use crate::js_error::JSError;
use closure_rhino::node::Ast;
pub trait MessageFormatter: Send + Sync {
    // port: MessageFormatter#formatError
    fn format_error(&self, ast: &Ast, error: &JSError) -> String;
    // port: MessageFormatter#formatWarning
    fn format_warning(&self, ast: &Ast, warning: &JSError) -> String;
}
