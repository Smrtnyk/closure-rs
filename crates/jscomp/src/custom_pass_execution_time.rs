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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CustomPassExecutionTime.java.

// port: CustomPassExecutionTime
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CustomPassExecutionTime {
    BEFORE_CHECKS,
    BEFORE_OPTIMIZATIONS,
    BEFORE_OPTIMIZATION_LOOP,
    AFTER_OPTIMIZATION_LOOP,
}
impl CustomPassExecutionTime {
    pub const VALUES: &'static [Self] = &[
        Self::BEFORE_CHECKS,
        Self::BEFORE_OPTIMIZATIONS,
        Self::BEFORE_OPTIMIZATION_LOOP,
        Self::AFTER_OPTIMIZATION_LOOP,
    ];
    // port: CustomPassExecutionTime#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.as_str() == name)
    }
    // port: CustomPassExecutionTime#toString
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BEFORE_CHECKS => "BEFORE_CHECKS",
            Self::BEFORE_OPTIMIZATIONS => "BEFORE_OPTIMIZATIONS",
            Self::BEFORE_OPTIMIZATION_LOOP => "BEFORE_OPTIMIZATION_LOOP",
            Self::AFTER_OPTIMIZATION_LOOP => "AFTER_OPTIMIZATION_LOOP",
        }
    }
}
impl std::fmt::Display for CustomPassExecutionTime {
    // port: CustomPassExecutionTime#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
