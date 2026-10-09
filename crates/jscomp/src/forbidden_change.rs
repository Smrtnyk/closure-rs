/*
 * Copyright 2013 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ForbiddenChange.java.

//! Port of `ForbiddenChange.java`.

use crate::code_change_handler::CodeChangeHandler;

/// A change handler that throws a runtime exception when any changes are made to the code.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForbiddenChange;

impl CodeChangeHandler for ForbiddenChange {
    // port: ForbiddenChange#reportChange
    fn report_change(&mut self) {
        panic!("Code changes forbidden");
    }
}
