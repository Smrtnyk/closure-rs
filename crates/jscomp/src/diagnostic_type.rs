/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DiagnosticType.java.

use crate::{check_level::CheckLevel, platform::Platform};
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};
/// Java static final diagnostics use static instances of this type. Diagnostics
/// with computed class-init text use LazyLock<DiagnosticType> and String::leak.
#[derive(Debug)]
pub struct DiagnosticType {
    pub key: &'static str,
    pub format: &'static str,
    pub level: CheckLevel,
}
impl DiagnosticType {
    // port: DiagnosticType#error
    pub const fn error(name: &'static str, description_format: &'static str) -> Self {
        Self::make(name, CheckLevel::ERROR, description_format)
    }
    // port: DiagnosticType#warning
    pub const fn warning(name: &'static str, description_format: &'static str) -> Self {
        Self::make(name, CheckLevel::WARNING, description_format)
    }
    // port: DiagnosticType#disabled
    pub const fn disabled(name: &'static str, description_format: &'static str) -> Self {
        Self::make(name, CheckLevel::OFF, description_format)
    }
    // port: DiagnosticType#make
    pub const fn make(
        name: &'static str,
        level: CheckLevel,
        description_format: &'static str,
    ) -> Self {
        Self::new(name, level, description_format)
    }
    // port: DiagnosticType#DiagnosticType
    const fn new(key: &'static str, level: CheckLevel, format: &'static str) -> Self {
        Self { key, level, format }
    }
    // port: DiagnosticType#format
    pub fn format(&self, arguments: &[&str]) -> String {
        Platform::format_message(self.format, arguments)
    }
}
impl PartialEq for DiagnosticType {
    // port: DiagnosticType#equals
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for DiagnosticType {}
impl Hash for DiagnosticType {
    // port: DiagnosticType#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}
impl Ord for DiagnosticType {
    // port: DiagnosticType#compareTo
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.encode_utf16().cmp(other.key.encode_utf16())
    }
}
impl PartialOrd for DiagnosticType {
    // port: DiagnosticType#compareTo
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl fmt::Display for DiagnosticType {
    // port: DiagnosticType#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.format)
    }
}
