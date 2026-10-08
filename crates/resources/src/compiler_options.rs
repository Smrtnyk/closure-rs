/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerOptions.java.

//! Port of `CompilerOptions.Environment` (the rest of `CompilerOptions` lives in closure-jscomp,
//! which re-exports this enum).

use std::fmt;

/// port: CompilerOptions.Environment
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Environment {
    /// Hand crafted externs that have traditionally been the default externs.
    BROWSER,

    /// Only language externs are loaded.
    CUSTOM,
}

impl Environment {
    /// Java's `Environment.values()`, in declaration order.
    // port: CompilerOptions.Environment#values
    pub fn values() -> [Environment; 2] {
        [Environment::BROWSER, Environment::CUSTOM]
    }

    // port: CompilerOptions.Environment#name
    pub fn name(self) -> &'static str {
        match self {
            Environment::BROWSER => "BROWSER",
            Environment::CUSTOM => "CUSTOM",
        }
    }
}

impl fmt::Display for Environment {
    // port: CompilerOptions.Environment#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
