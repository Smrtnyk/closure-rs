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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/RenamingToken.java.

// port: RenamingToken
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenamingToken {
    INCONSISTENT,
    STABLE,
    DISABLE,
}
impl RenamingToken {
    pub const VALUES: &'static [Self] = &[Self::INCONSISTENT, Self::STABLE, Self::DISABLE];
    // port: RenamingToken#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl std::fmt::Display for RenamingToken {
    // port: RenamingToken#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl crate::renaming_map::RenamingMap for RenamingToken {
    // port: RenamingToken#get
    fn get(
        &self,
        _value: &closure_rhino::js_string::JsString,
    ) -> Option<closure_rhino::js_string::JsString> {
        None
    }
}
