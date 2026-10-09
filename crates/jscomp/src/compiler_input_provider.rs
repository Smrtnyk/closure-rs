/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/CompilerInputProvider.java.

use crate::compiler_input::CompilerInput;
use closure_rhino::input_id::InputId;
pub trait CompilerInputProvider {
    // port: CompilerInputProvider#getInput
    fn get_input(&self, input_id: &InputId) -> Option<&CompilerInput>;
}
impl CompilerInputProvider for crate::Compiler {
    // port: Compiler#getInput
    fn get_input(&self, input_id: &InputId) -> Option<&CompilerInput> {
        self.get_input(input_id)
    }
}
