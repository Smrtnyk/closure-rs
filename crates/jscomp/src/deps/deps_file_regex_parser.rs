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
//   src/com/google/javascript/jscomp/deps/DepsFileRegexParser.java,
//   src/com/google/javascript/jscomp/deps/JsFileLineParser.java.

use super::{
    dependency_info::Require,
    js_file_line_parser::{
        JsFileLineParser, LineParser, ParseException, SharedErrorManager, guava_whitespace,
    },
    simple_dependency_info::SimpleDependencyInfo,
};
use closure_rhino::{
    java_lang::regex::{Matcher, Pattern},
    js_string::JsString,
};
use indexmap::IndexMap;
use std::{
    io::{self, Read},
    sync::Arc,
};
pub struct DepsFileRegexParser {
    pub base: JsFileLineParser,
    dep_matcher: Matcher,
    dep_args_match: Matcher,
    dep_infos: Vec<SimpleDependencyInfo>,
    path_translator: Arc<dyn Fn(&str) -> String + Send + Sync>,
}
impl DepsFileRegexParser {
    // port: DepsFileRegexParser#DepsFileRegexParser(ErrorManager)
    pub fn new(manager: SharedErrorManager) -> Self {
        Self::with_path_translator(Arc::new(str::to_owned), manager)
    }
    // port: DepsFileRegexParser#DepsFileRegexParser(Function,ErrorManager)
    pub fn with_path_translator(
        path_translator: Arc<dyn Fn(&str) -> String + Send + Sync>,
        manager: SharedErrorManager,
    ) -> Self {
        Self {
            base: JsFileLineParser::new(manager),
            dep_matcher: Pattern::compile(r"\s*goog.addDependency\((.*)\);?\s*").matcher(""),
            dep_args_match: Pattern::compile(concat!(
                r"\s*([^,]*), (\[[^\]]*\]), (\[[^\]]*\])",
                r"(?:, (true|false|\{[^{}]*\}))?\s*"
            ))
            .matcher(""),
            dep_infos: vec![],
            path_translator,
        }
    }
    // port: JsFileLineParser#setShortcutMode (inherited)
    pub fn set_shortcut_mode(&mut self, mode: bool) {
        self.base.set_shortcut_mode(mode);
    }
    // port: DepsFileRegexParser#parseFile(String,String)
    pub fn parse_file(
        &mut self,
        file_path: &str,
        contents: impl Into<JsString>,
    ) -> Vec<SimpleDependencyInfo> {
        self.dep_infos.clear();
        JsFileLineParser::do_parse(self, file_path, contents);
        self.dep_infos.clone()
    }
    // port: DepsFileRegexParser#parseFile(String)
    pub fn parse_file_from_path(
        &mut self,
        file_path: &str,
    ) -> io::Result<Vec<SimpleDependencyInfo>> {
        Ok(self.parse_file_reader(file_path, std::fs::File::open(file_path)?))
    }
    // port: DepsFileRegexParser#parseFileReader
    pub fn parse_file_reader(
        &mut self,
        file_path: &str,
        reader: impl Read,
    ) -> Vec<SimpleDependencyInfo> {
        self.dep_infos.clear();
        // port: DepsFileRegexParser#parseFileReader: Logger FINE is disabled at the default level.
        JsFileLineParser::do_parse_reader(self, file_path, reader);
        self.dep_infos.clone()
    }
    // port: DepsFileRegexParser#parseLoadFlags
    fn parse_load_flags(
        &mut self,
        flags: Option<&JsString>,
    ) -> Result<IndexMap<String, String>, ParseException> {
        if flags.is_none_or(|flags| flags == "false") {
            Ok(IndexMap::new())
        } else if flags.is_some_and(|flags| flags == "true") {
            Ok(IndexMap::from([("module".into(), "goog".into())]))
        } else {
            self.base.parse_js_string_map(flags.unwrap())
        }
    }
}
impl LineParser for DepsFileRegexParser {
    // port: JsFileLineParser#JsFileLineParser (base fields)
    fn base(&mut self) -> &mut JsFileLineParser {
        &mut self.base
    }
    // port: DepsFileRegexParser#parseLine
    fn parse_line(&mut self, line: &JsString) -> Result<bool, ParseException> {
        let mut has_dependencies = false;
        if line.index_of(&JsString::from("addDependency")) >= 0 {
            self.dep_matcher.reset(line);
            if self.dep_matcher.matches() {
                has_dependencies = true;
                let params = self.dep_matcher.group_units(1).unwrap();
                self.dep_args_match.reset(params.clone());
                if !self.dep_args_match.matches() {
                    return Err(ParseException::new(
                        &format!(
                            "Invalid arguments to goog.addDependency(). Found: {}",
                            params.to_string_lossy()
                        ),
                        true,
                    ));
                }
                let relative_path = self
                    .base
                    .parse_js_string(self.dep_args_match.group_units(1).unwrap())?;
                let path = (self.path_translator)(&relative_path);
                let mut provides = self
                    .base
                    .parse_js_string_array(self.dep_args_match.group_units(2).unwrap())?;
                let load_flags =
                    self.parse_load_flags(self.dep_args_match.group_units(4).as_ref())?;
                if load_flags.get("module").is_some_and(|m| m == "es6") {
                    provides.push(relative_path);
                }
                let requires = self
                    .base
                    .parse_js_string_array(self.dep_args_match.group_units(3).unwrap())?
                    .iter()
                    .map(|s| Require::parsed_from_deps(s))
                    .collect::<Vec<_>>();
                let dep_info = SimpleDependencyInfo::builder(&path, &self.base.file_path)
                    .set_provides(provides)
                    .set_requires(requires)
                    .set_load_flags(load_flags)
                    .build();
                // port: DepsFileRegexParser#parseLine: Logger FINE is disabled at the default level.
                self.dep_infos.push(dep_info);
            }
        }
        Ok(!self.base.shortcut_mode
            || has_dependencies
            || line.as_units().iter().copied().all(guava_whitespace))
    }
}
#[cfg(test)]
mod tests;
