/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/js/RuntimeJsLibManager.java.

//! Port of the parts of `com.google.javascript.jscomp.js.RuntimeJsLibManager` that need no AST:
//! `RUNTIME_LIB_DIR` and the nested `FieldsTable`. The manager itself (it injects runtime
//! libraries into the AST) is ported in closure-jscomp on top of these.

use std::sync::LazyLock;

use indexmap::IndexMap;

use crate::guava;
use crate::guava::ImmutableMapBuilder;
use crate::resources::resource_loader::ResourceLoader;

const RUNTIME_JS_LIB_MANAGER: &str = "com.google.javascript.jscomp.js.RuntimeJsLibManager";

pub const RUNTIME_LIB_DIR: &str = "src/com/google/javascript/jscomp/js/";

/// Holds the names of all possible $jscomp fields & the file in which they're defined.
/// For example, "inherits" -> "es6/inherits".
/// (This doesn't include nested properties - so no "$jscomp.a.b.c")
// port: RuntimeJsLibManager.FieldsTable#INSTANCE
static INSTANCE: LazyLock<IndexMap<String, String>> = LazyLock::new(FieldsTable::load_fields);

// port: RuntimeJsLibManager.FieldsTable
pub struct FieldsTable;

impl FieldsTable {
    /// `FieldsTable.INSTANCE`, loaded on first use.
    // port: RuntimeJsLibManager.FieldsTable#INSTANCE
    pub fn instance() -> &'static IndexMap<String, String> {
        &INSTANCE
    }

    // port: RuntimeJsLibManager.FieldsTable#loadFields
    fn load_fields() -> IndexMap<String, String> {
        let transpilation_libs_txt =
            ResourceLoader::load_text_resource(RUNTIME_JS_LIB_MANAGER, "transpilation_libs.txt");
        Self::load_fields_from(&transpilation_libs_txt)
    }

    /// The body of `loadFields` after the resource is read; separate only so that the tests can
    /// feed it other table texts.
    // port: RuntimeJsLibManager.FieldsTable#loadFields
    pub fn load_fields_from(transpilation_libs_txt: &str) -> IndexMap<String, String> {
        let mut all_fields = ImmutableMapBuilder::new();
        for line in guava::split_omit_empty_strings(transpilation_libs_txt, '\n') {
            let tokens = guava::split_limit_to_list(line, ',', 2);
            assert!(tokens.len() == 2, "[{}]", tokens.join(", "));
            let field_name = tokens[0];
            let resource_path = tokens[1];
            all_fields.put(field_name.to_string(), resource_path.to_string());
        }
        all_fields.build_or_throw()
    }
}
