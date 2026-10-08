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
//   src/com/google/javascript/jscomp/PropertyRenamingPolicy.java.

// port: PropertyRenamingPolicy
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyRenamingPolicy {
    OFF,
    ALL_UNQUOTED,
}
impl PropertyRenamingPolicy {
    pub const VALUES: &'static [Self] = &[Self::OFF, Self::ALL_UNQUOTED];
    // port: PropertyRenamingPolicy#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.as_str() == name)
    }
    // port: PropertyRenamingPolicy#toString
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OFF => "OFF",
            Self::ALL_UNQUOTED => "ALL_UNQUOTED",
        }
    }
}
impl std::fmt::Display for PropertyRenamingPolicy {
    // port: PropertyRenamingPolicy#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
