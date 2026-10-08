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
//   src/com/google/javascript/jscomp/DiagnosticGroup.java.

use crate::{diagnostic_type::DiagnosticType, js_error::JSError};
use indexmap::{IndexMap, IndexSet};
use std::{
    fmt,
    sync::{Arc, LazyLock, Mutex},
};
#[derive(Debug)]
pub struct DiagnosticGroup {
    types: IndexSet<&'static DiagnosticType>,
    name: Option<String>,
}
static SINGLETONS: LazyLock<Mutex<IndexMap<&'static DiagnosticType, Arc<DiagnosticGroup>>>> =
    LazyLock::new(|| Mutex::new(IndexMap::new()));
impl DiagnosticGroup {
    // port: DiagnosticGroup#DiagnosticGroup(DiagnosticType...)
    pub fn new(types: &[&'static DiagnosticType]) -> Self {
        Self {
            types: types.iter().copied().collect(),
            name: None,
        }
    }
    // port: DiagnosticGroup#DiagnosticGroup(String,DiagnosticType...)
    pub fn new_named(name: &str, types: &[&'static DiagnosticType]) -> Self {
        Self {
            types: types.iter().copied().collect(),
            name: Some(name.into()),
        }
    }
    // port: DiagnosticGroup#DiagnosticGroup(DiagnosticGroup...)
    pub fn new_from_groups(groups: &[Arc<Self>]) -> Self {
        Self {
            types: groups
                .iter()
                .flat_map(|g| g.types.iter().copied())
                .collect(),
            name: None,
        }
    }
    // port: DiagnosticGroup#DiagnosticGroup(String,DiagnosticGroup...)
    pub fn new_named_from_groups(name: &str, groups: &[Arc<Self>]) -> Self {
        let mut result = Self::new_from_groups(groups);
        result.name = Some(name.into());
        result
    }
    // port: DiagnosticGroup#forType
    pub fn for_type(type_: &'static DiagnosticType) -> Arc<Self> {
        SINGLETONS
            .lock()
            .unwrap()
            .entry(type_)
            .or_insert_with(|| Arc::new(Self::new(&[type_])))
            .clone()
    }
    // port: DiagnosticGroup#matches(JSError)
    pub fn matches(&self, error: &JSError) -> bool {
        self.matches_type(error.get_type())
    }
    // port: DiagnosticGroup#matches(DiagnosticType)
    pub fn matches_type(&self, type_: &DiagnosticType) -> bool {
        self.types.contains(type_)
    }
    // port: DiagnosticGroup#getTypes
    pub fn get_types(&self) -> &IndexSet<&'static DiagnosticType> {
        &self.types
    }
    // port: DiagnosticGroup#getName
    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}
impl fmt::Display for DiagnosticGroup {
    // port: DiagnosticGroup#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "DiagnosticGroup<{name}>")
        }
        // The object's address stands in for JVM identityHashCode; this does not
        // reach compiler output and preserves identity for the object's lifetime.
        else {
            write!(
                f,
                "com.google.javascript.jscomp.DiagnosticGroup@{:x}",
                self as *const Self as usize
            )
        }
    }
}
