/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/DependencyInfo.java,
//   test/com/google/javascript/jscomp/deps/DependencyInfoTest.java.

use closure_rhino::check_state;
use indexmap::IndexMap;
use std::{fmt, sync::LazyLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    GOOG_REQUIRE_SYMBOL,
    ES6_IMPORT,
    PARSED_FROM_DEPS,
    COMMON_JS,
    COMPILER_CHUNK,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Require {
    symbol: String,
    raw_text: String,
    type_: Type,
}
impl Require {
    pub const BASE: &'static LazyLock<Require> = &BASE;
    // port: DependencyInfo.Require#asSymbolList
    pub fn as_symbol_list<'a>(requires: impl IntoIterator<Item = &'a Require>) -> Vec<String> {
        requires
            .into_iter()
            .map(|r| r.get_symbol().to_owned())
            .collect()
    }
    // port: DependencyInfo.Require#googRequireSymbol
    pub fn goog_require_symbol(symbol: &str) -> Self {
        Self::builder()
            .set_raw_text(symbol)
            .set_symbol(symbol)
            .set_type(Type::GOOG_REQUIRE_SYMBOL)
            .build()
    }
    // port: DependencyInfo.Require#es6Import
    pub fn es6_import(symbol: &str, raw_path: &str) -> Self {
        Self::builder()
            .set_raw_text(raw_path)
            .set_symbol(symbol)
            .set_type(Type::ES6_IMPORT)
            .build()
    }
    // port: DependencyInfo.Require#commonJs
    pub fn common_js(symbol: &str, raw_path: &str) -> Self {
        Self::builder()
            .set_raw_text(raw_path)
            .set_symbol(symbol)
            .set_type(Type::COMMON_JS)
            .build()
    }
    // port: DependencyInfo.Require#compilerChunk
    pub fn compiler_chunk(symbol: &str) -> Self {
        Self::builder()
            .set_raw_text(symbol)
            .set_symbol(symbol)
            .set_type(Type::COMPILER_CHUNK)
            .build()
    }
    // port: DependencyInfo.Require#parsedFromDeps
    pub fn parsed_from_deps(symbol: &str) -> Self {
        Self::builder()
            .set_raw_text(symbol)
            .set_symbol(symbol)
            .set_type(Type::PARSED_FROM_DEPS)
            .build()
    }
    // port: DependencyInfo.Require#builder
    pub fn builder() -> Builder {
        Builder::default()
    }
    // port: DependencyInfo.Require#toBuilder
    pub fn to_builder(&self) -> Builder {
        Self::builder()
            .set_symbol(&self.symbol)
            .set_raw_text(&self.raw_text)
            .set_type(self.type_)
    }
    // port: DependencyInfo.Require#withSymbol
    pub fn with_symbol(&self, symbol: &str) -> Self {
        self.to_builder().set_symbol(symbol).build()
    }
    // port: DependencyInfo.Require#getSymbol
    pub fn get_symbol(&self) -> &str {
        &self.symbol
    }
    // port: DependencyInfo.Require#getRawText
    pub fn get_raw_text(&self) -> &str {
        &self.raw_text
    }
    // port: DependencyInfo.Require#getType
    pub fn get_type(&self) -> Type {
        self.type_
    }
}
pub static BASE: LazyLock<Require> = LazyLock::new(|| Require::goog_require_symbol("goog"));
#[derive(Default)]
pub struct Builder {
    symbol: Option<String>,
    raw_text: Option<String>,
    type_: Option<Type>,
}
impl Builder {
    // port: DependencyInfo.Require.Builder#setType
    pub fn set_type(mut self, value: Type) -> Self {
        self.type_ = Some(value);
        self
    }
    // port: DependencyInfo.Require.Builder#setRawText
    pub fn set_raw_text(mut self, value: &str) -> Self {
        self.raw_text = Some(value.into());
        self
    }
    // port: DependencyInfo.Require.Builder#setSymbol
    pub fn set_symbol(mut self, value: &str) -> Self {
        self.symbol = Some(value.into());
        self
    }
    // port: DependencyInfo.Require.Builder#build
    pub fn build(self) -> Require {
        let mut missing = String::new();
        if self.symbol.is_none() {
            missing.push_str(" symbol");
        }
        if self.raw_text.is_none() {
            missing.push_str(" rawText");
        }
        if self.type_.is_none() {
            missing.push_str(" type");
        }
        check_state!(
            missing.is_empty(),
            "Missing required properties:%s",
            missing
        );
        Require {
            symbol: self.symbol.unwrap(),
            raw_text: self.raw_text.unwrap(),
            type_: self.type_.unwrap(),
        }
    }
}
impl fmt::Display for Require {
    // port: AutoValue_DependencyInfo_Require#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Require{{symbol={}, rawText={}, type={:?}}}",
            self.symbol, self.raw_text, self.type_
        )
    }
}
pub trait DependencyInfo: Send + Sync {
    // port: DependencyInfo#getName
    fn get_name(&self) -> &str;
    // port: DependencyInfo#getPathRelativeToClosureBase
    fn get_path_relative_to_closure_base(&self) -> &str;
    // port: DependencyInfo#getProvides
    fn get_provides(&self) -> &[String];
    // port: DependencyInfo#getRequires
    fn get_requires(&self) -> &[Require];
    // port: DependencyInfo#getRequiredSymbols
    fn get_required_symbols(&self) -> Vec<String> {
        Require::as_symbol_list(self.get_requires())
    }
    // port: DependencyInfo#getTypeRequires
    fn get_type_requires(&self) -> &[String];
    // port: DependencyInfo#getLoadFlags
    fn get_load_flags(&self) -> &IndexMap<String, String>;
    // port: DependencyInfo#isEs6Module
    fn is_es6_module(&self) -> bool {
        self.get_load_flags()
            .get("module")
            .is_some_and(|s| s == "es6")
    }
    // port: DependencyInfo#isGoogModule
    fn is_goog_module(&self) -> bool {
        self.get_load_flags()
            .get("module")
            .is_some_and(|s| s == "goog")
    }
    // port: DependencyInfo#getHasExternsAnnotation
    fn get_has_externs_annotation(&self) -> bool;
    // port: DependencyInfo#getHasNoCompileAnnotation
    fn get_has_no_compile_annotation(&self) -> bool;
}
pub struct Util;
impl Util {
    // port: DependencyInfo.Util#writeAddDependency
    pub fn write_add_dependency(
        out: &mut dyn fmt::Write,
        info: &dyn DependencyInfo,
    ) -> fmt::Result {
        write!(
            out,
            "goog.addDependency('{}', ",
            info.get_path_relative_to_closure_base()
        )?;
        Self::write_js_array(out, info.get_provides())?;
        out.write_str(", ")?;
        Self::write_js_array(out, &Require::as_symbol_list(info.get_requires()))?;
        if !info.get_load_flags().is_empty() {
            out.write_str(", ")?;
            Self::write_js_object(out, info.get_load_flags())?;
        }
        out.write_str(");\n")
    }
    // port: DependencyInfo.Util#writeJsObject
    fn write_js_object(out: &mut dyn fmt::Write, map: &IndexMap<String, String>) -> fmt::Result {
        let entries: Vec<String> = map
            .iter()
            .map(|(k, v)| format!("'{}': '{}'", k.replace('\'', "\\'"), v.replace('\'', "\\'")))
            .collect();
        write!(out, "{{{}}}", entries.join(", "))
    }
    // port: DependencyInfo.Util#writeJsArray
    fn write_js_array(out: &mut dyn fmt::Write, values: &[String]) -> fmt::Result {
        let quoted: Vec<String> = values
            .iter()
            .map(|s| format!("'{}'", s.replace('\'', "\\'")))
            .collect();
        write!(out, "[{}]", quoted.join(", "))
    }
}
#[cfg(test)]
mod tests {
    use super::super::simple_dependency_info::SimpleDependencyInfo;
    use super::*;
    // port: DependencyInfoTest#testWriteAddDependency
    #[test]
    fn test_write_add_dependency() {
        let mut sb = String::new();
        Util::write_add_dependency(
            &mut sb,
            &SimpleDependencyInfo::builder("../some/relative/path.js", "/unused/absolute/path.js")
                .set_provides(["provided.symbol", "other.provide"])
                .set_requires([
                    Require::goog_require_symbol("required.symbol"),
                    Require::goog_require_symbol("other.require"),
                ])
                .set_load_flags(IndexMap::from([
                    ("module".into(), "goog".into()),
                    ("lang".into(), "es6".into()),
                ]))
                .build(),
        )
        .unwrap();
        assert_eq!(
            sb,
            "goog.addDependency('../some/relative/path.js', ['provided.symbol', 'other.provide'], ['required.symbol', 'other.require'], {'module': 'goog', 'lang': 'es6'});\n"
        );
    }
    // port: DependencyInfoTest#testWriteAddDependency_emptyArguments
    #[test]
    fn test_write_add_dependency_empty_arguments() {
        let mut sb = String::new();
        Util::write_add_dependency(
            &mut sb,
            &SimpleDependencyInfo::builder("path.js", "unused.js").build(),
        )
        .unwrap();
        assert_eq!(sb, "goog.addDependency('path.js', [], []);\n");
    }
}
