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
//   src/com/google/javascript/jscomp/deps/JsFileLineParser.java,
//   src/com/google/javascript/jscomp/deps/JsFileRegexParser.java.

use super::{
    dependency_info::{BASE, Require},
    js_file_line_parser::{
        JsFileLineParser, LineParser, ParseException, SharedErrorManager, guava_whitespace,
    },
    module_loader::{EMPTY, MODULE_CONFLICT, ModuleLoader, ModulePath},
    simple_dependency_info::SimpleDependencyInfo,
};
use crate::{check_level::CheckLevel, js_error::JSError};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_state,
    java_lang::regex::{Matcher, Pattern},
    js_string::JsString,
};
use std::{io::Read, sync::LazyLock};
// port: JsFileRegexParser#ES6_EXPORT_PATTERN
static ES6_EXPORT_PATTERN: LazyLock<Pattern> = LazyLock::new(|| Pattern::compile(r"^export\b"));
#[derive(Clone, Copy, PartialEq, Eq)]
enum ModuleType {
    NON_MODULE,
    GOOG_MODULE,
    GOOG_PROVIDE,
    ES6_MODULE,
}
pub struct JsFileRegexParser {
    pub base: JsFileLineParser,
    goog_matcher: Matcher,
    es6_matcher: Matcher,
    goog_matcher_buffer: JsString,
    provides: Vec<String>,
    requires: Vec<Require>,
    type_requires: Vec<String>,
    file_has_provides_or_requires: bool,
    has_externs_annotation: bool,
    has_no_compile_annotation: bool,
    loader: ModuleLoader,
    file: ModulePath,
    module_type: ModuleType,
    seen_load_module: bool,
    include_goog_base: bool,
}
impl JsFileRegexParser {
    const GOOG_PROVIDE_REQUIRE_PATTERN: &'static str = concat!(
        r"(?:^|;)(?:[\p{L}\p{Nl}\p{Nd}$_,:{}\s]+=)?\s*",
        r"goog\.(?<func>provide|module|require|requireType|addDependency|declareModuleId)",
        r"\s*\(\s*(?<args>.*?)\s*\)"
    );
    const ES6_MODULE_PATTERN: &'static str = concat!(
        r"^",
        r"(?:import|export)\b\s*",
        r"(?:[a-zA-Z0-9$_*,{}\s]+\bfrom\s*|)",
        r#"(?:['"]([^'"]+)['"])?"#,
        r"\s*;"
    );
    const PROVIDES_GOOG_COMMENT: &'static str = "@provideGoog";
    const EXTERNS_COMMENT: &'static str = "@externs";
    const NOCOMPILE_COMMENT: &'static str = "@nocompile";
    const BUNDLED_GOOG_MODULE_START: &'static str = "goog.loadModule(function(";
    // port: JsFileRegexParser#JsFileRegexParser
    pub fn new(error_manager: SharedErrorManager) -> Self {
        Self {
            base: JsFileLineParser::new(error_manager),
            goog_matcher: Pattern::compile(Self::GOOG_PROVIDE_REQUIRE_PATTERN).matcher(""),
            es6_matcher: Pattern::compile(Self::ES6_MODULE_PATTERN).matcher(""),
            goog_matcher_buffer: JsString::from(""),
            provides: vec![],
            requires: vec![],
            type_requires: vec![],
            file_has_provides_or_requires: false,
            has_externs_annotation: false,
            has_no_compile_annotation: false,
            loader: EMPTY.clone(),
            file: EMPTY.resolve(""),
            module_type: ModuleType::NON_MODULE,
            seen_load_module: false,
            include_goog_base: false,
        }
    }
    // port: JsFileLineParser#setShortcutMode (inherited)
    pub fn set_shortcut_mode(&mut self, mode: bool) {
        self.base.set_shortcut_mode(mode);
    }
    // port: JsFileRegexParser#setIncludeGoogBase
    pub fn set_include_goog_base(&mut self, include: bool) -> &mut Self {
        check_state!(Self::is_supported());
        self.include_goog_base = include;
        self
    }
    // port: JsFileRegexParser#setModuleLoader
    pub fn set_module_loader(&mut self, loader: ModuleLoader) -> &mut Self {
        self.loader = loader;
        self
    }
    // port: JsFileRegexParser#parseFile
    pub fn parse_file(
        &mut self,
        file_path: &str,
        closure_relative_path: &str,
        file_contents: impl Into<JsString>,
    ) -> SimpleDependencyInfo {
        self.parse_reader_with(file_path, closure_relative_path, |parser| {
            JsFileLineParser::do_parse(parser, file_path, file_contents);
        })
    }
    // port: JsFileRegexParser#parseReader
    pub fn parse_reader(
        &mut self,
        file_path: &str,
        closure_relative_path: &str,
        file_contents: impl Read,
    ) -> SimpleDependencyInfo {
        self.parse_reader_with(file_path, closure_relative_path, |parser| {
            JsFileLineParser::do_parse_reader(parser, file_path, file_contents);
        })
    }
    // port: JsFileRegexParser#parseReader (shared Reader/StringReader processing)
    fn parse_reader_with(
        &mut self,
        file_path: &str,
        closure_relative_path: &str,
        do_parse: impl FnOnce(&mut Self),
    ) -> SimpleDependencyInfo {
        self.provides.clear();
        self.requires.clear();
        self.type_requires.clear();
        self.file_has_provides_or_requires = false;
        self.has_externs_annotation = false;
        self.has_no_compile_annotation = false;
        self.file = self.loader.resolve(file_path);
        self.module_type = ModuleType::NON_MODULE;
        self.seen_load_module = false;
        self.goog_matcher_buffer = JsString::from("");
        // port: JsFileRegexParser#parseReader: Logger FINE is disabled at the default level.
        do_parse(self);
        if self.module_type == ModuleType::ES6_MODULE {
            self.provides.push(self.file.to_module_name());
        }
        let mut load_flags = IndexMap::<_, _>::default();
        match self.module_type {
            ModuleType::GOOG_MODULE => {
                load_flags.insert("module".into(), "goog".into());
            }
            ModuleType::ES6_MODULE => {
                load_flags.insert("module".into(), "es6".into());
            }
            _ => {}
        }
        SimpleDependencyInfo::builder(closure_relative_path, file_path)
            .set_provides(self.provides.clone())
            .set_requires(self.requires.clone())
            .set_type_requires(self.type_requires.clone())
            .set_load_flags(load_flags)
            .set_has_externs_annotation(self.has_externs_annotation)
            .set_has_no_compile_annotation(self.has_no_compile_annotation)
            .build()
    }
    // port: JsFileRegexParser#setModuleType
    fn set_module_type(&mut self, type_: ModuleType) -> bool {
        let provide =
            type_ == ModuleType::GOOG_PROVIDE || self.module_type == ModuleType::GOOG_PROVIDE;
        let es6_module =
            type_ == ModuleType::ES6_MODULE || self.module_type == ModuleType::ES6_MODULE;
        let goog_module =
            type_ == ModuleType::GOOG_MODULE || self.module_type == ModuleType::GOOG_MODULE;
        if goog_module && provide && self.seen_load_module {
            self.module_type = ModuleType::GOOG_PROVIDE;
            return true;
        }
        let provide_goog_module_conflict = goog_module && provide && !self.seen_load_module;
        let provide_es6_module_conflict = es6_module && provide;
        let goog_es6_module_conflict = (goog_module || self.seen_load_module) && es6_module;
        if provide_goog_module_conflict || provide_es6_module_conflict || goog_es6_module_conflict {
            let file = self.file.to_string();
            self.base.error_manager.lock().unwrap().report(
                CheckLevel::WARNING,
                JSError::make_with_source_location(&file, -1, -1, &MODULE_CONFLICT, &[&file]),
            );
            return false;
        }
        self.module_type = type_;
        true
    }
    // port: JsFileRegexParser#applyGoogMatcher
    fn apply_goog_matcher(&mut self, line: &JsString) -> Result<bool, ParseException> {
        let mut line_has_provides_or_requires = false;
        self.goog_matcher.reset(line);
        while self.goog_matcher.find() {
            line_has_provides_or_requires = true;
            if self.include_goog_base && !self.file_has_provides_or_requires {
                self.file_has_provides_or_requires = true;
                self.requires.push(BASE.clone());
            }
            let method_name = self.goog_matcher.group_named("func").unwrap();
            let first_char = method_name.as_bytes()[0];
            let is_declare_module_namespace = first_char == b'd';
            let is_module = !is_declare_module_namespace && first_char == b'm';
            let is_provide = first_char == b'p';
            let mut provides_namespace = is_provide || is_module || is_declare_module_namespace;
            let is_require = first_char == b'r';
            if is_module && !self.seen_load_module {
                provides_namespace = self.set_module_type(ModuleType::GOOG_MODULE);
            }
            if is_provide {
                provides_namespace = self.set_module_type(ModuleType::GOOG_PROVIDE);
            }
            if provides_namespace || is_require {
                let arg = self
                    .base
                    .parse_js_string(self.goog_matcher.group_named_units("args").unwrap())?;
                if is_require {
                    if method_name == "requireType" {
                        self.type_requires.push(arg);
                    } else if arg != "goog" {
                        self.requires.push(Require::goog_require_symbol(&arg));
                    }
                } else {
                    self.provides.push(arg);
                }
            }
        }
        Ok(line_has_provides_or_requires)
    }
    // port: JsFileRegexParser#isSupported
    pub fn is_supported() -> bool {
        true
    }
}
impl LineParser for JsFileRegexParser {
    // port: JsFileLineParser#JsFileLineParser (base fields)
    fn base(&mut self) -> &mut JsFileLineParser {
        &mut self.base
    }
    // port: JsFileRegexParser#parseJsDocCommentLine
    fn parse_js_doc_comment_line(&mut self, line: &JsString) -> bool {
        if self.include_goog_base && line.index_of(Self::PROVIDES_GOOG_COMMENT) >= 0 {
            self.provides.push("goog".into());
            return false;
        } else if line.index_of(Self::EXTERNS_COMMENT) >= 0 {
            self.has_externs_annotation = true;
            return false;
        } else if line.index_of(Self::NOCOMPILE_COMMENT) >= 0 {
            self.has_no_compile_annotation = true;
            return false;
        }
        true
    }
    // port: JsFileRegexParser#parseLine
    fn parse_line(&mut self, line: &JsString) -> Result<bool, ParseException> {
        let mut line_has_provides_or_requires = false;
        if line.starts_with(Self::BUNDLED_GOOG_MODULE_START) {
            self.seen_load_module = true;
        }
        let line_has_provides_or_requires_words = line.index_of("provide") >= 0
            || line.index_of("require") >= 0
            || line.index_of("module") >= 0
            || line.index_of("addDependency") >= 0
            || line.index_of("declareModuleId") >= 0;
        if !self.goog_matcher_buffer.is_empty() {
            line_has_provides_or_requires =
                self.apply_goog_matcher(&self.goog_matcher_buffer.concat(line))?;
        }
        if !line_has_provides_or_requires && line_has_provides_or_requires_words {
            line_has_provides_or_requires = self.apply_goog_matcher(line)?;
        }
        if !line_has_provides_or_requires && line_has_provides_or_requires_words {
            self.goog_matcher_buffer = line.clone();
        } else {
            self.goog_matcher_buffer = JsString::from("");
        }
        if line.starts_with("import") || line.starts_with("export") {
            self.es6_matcher.reset(line);
            while self.es6_matcher.find() {
                self.set_module_type(ModuleType::ES6_MODULE);
                line_has_provides_or_requires = true;
                if let Some(arg) = self.es6_matcher.group(1) {
                    if let Some(symbol) = arg.strip_prefix("goog:") {
                        self.requires.push(Require::goog_require_symbol(symbol));
                    } else {
                        let path = self
                            .file
                            .resolve_js_module(
                                &arg,
                                Some(&self.base.file_path),
                                self.base.line_num,
                                self.es6_matcher.start() as i32,
                            )
                            .unwrap_or_else(|| self.file.resolve_module_as_path(&arg));
                        self.requires
                            .push(Require::es6_import(&path.to_module_name(), &arg));
                    }
                }
            }
            if self.module_type != ModuleType::ES6_MODULE
                && ES6_EXPORT_PATTERN.matcher(line).looking_at()
            {
                self.set_module_type(ModuleType::ES6_MODULE);
            }
        }
        Ok(!self.base.shortcut_mode
            || line_has_provides_or_requires
            || !self.goog_matcher_buffer.is_empty()
            || line.as_units().iter().copied().all(guava_whitespace)
            || line.index_of_char(b';' as u16) == -1
            || line.index_of("goog.setTestOnly") >= 0
            || line.index_of("goog.module.declareLegacyNamespace") >= 0)
    }
}
#[cfg(test)]
mod tests;
