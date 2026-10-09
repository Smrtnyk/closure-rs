/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/ParserConfiguration.java.

//! Port of `com.google.javascript.jscomp.parsing.ParserConfiguration`.

use std::sync::LazyLock;

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::java_lang::properties;

/// The resource bundle `com.google.javascript.jscomp.parsing.ParserConfig` (the jar's copy is
/// byte-identical to the source file embedded here).
static PARSER_CONFIG: LazyLock<IndexMap<String, String>> =
    LazyLock::new(|| properties::load(include_str!("ParserConfig.properties")));

pub struct ParserConfiguration;

impl ParserConfiguration {
    // port: ParserConfiguration#getString
    pub fn get_string(name: &str) -> String {
        match PARSER_CONFIG.get(name) {
            Some(value) => value.clone(),
            // ResourceBundle#getString throws MissingResourceException.
            None => panic!(
                "Can't find resource for bundle java.util.PropertyResourceBundle, key {name}"
            ),
        }
    }

    // port: ParserConfiguration#ParserConfiguration
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
}
