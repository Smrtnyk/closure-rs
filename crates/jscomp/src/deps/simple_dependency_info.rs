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
//   src/com/google/javascript/jscomp/deps/SimpleDependencyInfo.java.

use super::dependency_info::{DependencyInfo, Require};
use indexmap::IndexMap;
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::LazyLock,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleDependencyInfo {
    name: String,
    path_relative_to_closure_base: String,
    provides: Vec<String>,
    requires: Vec<Require>,
    type_requires: Vec<String>,
    load_flags: IndexMap<String, String>,
    has_externs_annotation: bool,
    has_no_compile_annotation: bool,
}
impl SimpleDependencyInfo {
    pub const EMPTY: &'static LazyLock<Self> = &EMPTY;
    // port: SimpleDependencyInfo#builder
    pub fn builder(src_path_relative_to_closure: &str, path_of_defining_file: &str) -> Builder {
        Builder {
            info: Self {
                name: path_of_defining_file.into(),
                path_relative_to_closure_base: src_path_relative_to_closure.into(),
                provides: vec![],
                requires: vec![],
                type_requires: vec![],
                load_flags: IndexMap::new(),
                has_externs_annotation: false,
                has_no_compile_annotation: false,
            },
        }
    }
}
pub static EMPTY: LazyLock<SimpleDependencyInfo> =
    LazyLock::new(|| SimpleDependencyInfo::builder("", "").build());
#[derive(Clone)]
pub struct Builder {
    info: SimpleDependencyInfo,
}
impl Builder {
    // port: SimpleDependencyInfo.Builder#from
    pub fn from(copy: &dyn DependencyInfo) -> Self {
        SimpleDependencyInfo::builder(copy.get_path_relative_to_closure_base(), copy.get_name())
            .set_provides(copy.get_provides().iter().cloned())
            .set_requires(copy.get_requires().iter().cloned())
            .set_type_requires(copy.get_type_requires().iter().cloned())
            .set_load_flags(copy.get_load_flags().clone())
            .set_has_externs_annotation(copy.get_has_externs_annotation())
            .set_has_no_compile_annotation(copy.get_has_no_compile_annotation())
    }
    // port: SimpleDependencyInfo.Builder#setName
    pub fn set_name(mut self, name: &str) -> Self {
        self.info.name = name.into();
        self
    }
    // port: SimpleDependencyInfo.Builder#setPathRelativeToClosureBase
    pub fn set_path_relative_to_closure_base(mut self, path: &str) -> Self {
        self.info.path_relative_to_closure_base = path.into();
        self
    }
    // port: SimpleDependencyInfo.Builder#setProvides(Collection<String>), setProvides(String...)
    pub fn set_provides(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.info.provides = values.into_iter().map(Into::into).collect();
        self
    }
    // port: SimpleDependencyInfo.Builder#setRequires(Collection<Require>), setRequires(Require...)
    pub fn set_requires(mut self, values: impl IntoIterator<Item = Require>) -> Self {
        self.info.requires = values.into_iter().collect();
        self
    }
    // port: SimpleDependencyInfo.Builder#setTypeRequires(Collection<String>), setTypeRequires(String...)
    pub fn set_type_requires(
        mut self,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.info.type_requires = values.into_iter().map(Into::into).collect();
        self
    }
    // port: SimpleDependencyInfo.Builder#setLoadFlags
    pub fn set_load_flags(mut self, values: IndexMap<String, String>) -> Self {
        self.info.load_flags = values;
        self
    }
    // port: SimpleDependencyInfo.Builder#setHasExternsAnnotation
    pub fn set_has_externs_annotation(mut self, value: bool) -> Self {
        self.info.has_externs_annotation = value;
        self
    }
    // port: SimpleDependencyInfo.Builder#setHasNoCompileAnnotation
    pub fn set_has_no_compile_annotation(mut self, value: bool) -> Self {
        self.info.has_no_compile_annotation = value;
        self
    }
    // port: SimpleDependencyInfo.Builder#setGoogModule
    pub fn set_goog_module(self, is_module: bool) -> Self {
        self.set_load_flags(if is_module {
            IndexMap::from([("module".into(), "goog".into())])
        } else {
            IndexMap::new()
        })
    }
    // port: SimpleDependencyInfo.Builder#build
    pub fn build(self) -> SimpleDependencyInfo {
        self.info
    }
}
impl DependencyInfo for SimpleDependencyInfo {
    // port: SimpleDependencyInfo#getName
    fn get_name(&self) -> &str {
        &self.name
    }
    // port: SimpleDependencyInfo#getPathRelativeToClosureBase
    fn get_path_relative_to_closure_base(&self) -> &str {
        &self.path_relative_to_closure_base
    }
    // port: SimpleDependencyInfo#getProvides
    fn get_provides(&self) -> &[String] {
        &self.provides
    }
    // port: SimpleDependencyInfo#getRequires
    fn get_requires(&self) -> &[Require] {
        &self.requires
    }
    // port: SimpleDependencyInfo#getTypeRequires
    fn get_type_requires(&self) -> &[String] {
        &self.type_requires
    }
    // port: SimpleDependencyInfo#getLoadFlags
    fn get_load_flags(&self) -> &IndexMap<String, String> {
        &self.load_flags
    }
    // port: SimpleDependencyInfo#getHasExternsAnnotation
    fn get_has_externs_annotation(&self) -> bool {
        self.has_externs_annotation
    }
    // port: SimpleDependencyInfo#getHasNoCompileAnnotation
    fn get_has_no_compile_annotation(&self) -> bool {
        self.has_no_compile_annotation
    }
}
impl Hash for SimpleDependencyInfo {
    // port: AutoValue_SimpleDependencyInfo#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.path_relative_to_closure_base.hash(state);
        self.provides.hash(state);
        self.requires.hash(state);
        self.type_requires.hash(state);
        // Java Map equality ignores insertion order.
        let mut flags: Vec<_> = self.load_flags.iter().collect();
        flags.sort();
        flags.hash(state);
        self.has_externs_annotation.hash(state);
        self.has_no_compile_annotation.hash(state);
    }
}
impl fmt::Display for SimpleDependencyInfo {
    // port: AutoValue_SimpleDependencyInfo#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flags: Vec<_> = self
            .load_flags
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let requires: Vec<_> = self.requires.iter().map(ToString::to_string).collect();
        write!(
            f,
            "SimpleDependencyInfo{{name={}, pathRelativeToClosureBase={}, provides=[{}], requires=[{}], typeRequires=[{}], loadFlags={{{}}}, hasExternsAnnotation={}, hasNoCompileAnnotation={}}}",
            self.name,
            self.path_relative_to_closure_base,
            self.provides.join(", "),
            requires.join(", "),
            self.type_requires.join(", "),
            flags.join(", "),
            self.has_externs_annotation,
            self.has_no_compile_annotation
        )
    }
}
