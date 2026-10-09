/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/RewriteJsonToModule.java.

//! Rewrites a JSON file to be a module export. So that the JSON file parses correctly, it is
//! wrapped in an EXPR_RESULT. The pass makes only basic checks that the file provided is valid
//! JSON. It is not a full JSON validator.
//!
//! Looks for JSON files named "package.json" so that the "main" property can be used as an alias
//! in module name resolution. See https://docs.npmjs.com/files/package.json#main
use crate::{
    compiler::Compiler,
    compiler_pass::CompilerPass,
    deps::module_loader::ModuleLoader,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    check_state, ir::IR, js_string::JsString, jsdoc_info::JSDocInfo, node::NodeId,
    static_source_file::StaticSourceFile, token::Token,
};

// port: RewriteJsonToModule#JSON_UNEXPECTED_TOKEN
pub static JSON_UNEXPECTED_TOKEN: DiagnosticType =
    DiagnosticType::error("JSC_JSON_UNEXPECTED_TOKEN", "Unexpected JSON token");

#[derive(Default)]
pub struct RewriteJsonToModule {
    package_json_main_entries: IndexMap<String, String>,
}

impl RewriteJsonToModule {
    /// Creates a new RewriteJsonToModule instance which can be used to rewrite JSON files to
    /// modules.
    // port: RewriteJsonToModule#RewriteJsonToModule
    pub fn new() -> Self {
        Self {
            package_json_main_entries: IndexMap::<_, _>::default(),
        }
    }

    // port: RewriteJsonToModule#getPackageJsonMainEntries
    pub fn get_package_json_main_entries(&self) -> IndexMap<String, String> {
        self.package_json_main_entries.clone()
    }

    /// For script nodes of JSON objects, add a module variable assignment so the result is
    /// exported.
    ///
    /// If the file path ends with "/package.json", look for main entries in their specified
    /// order in the object literal and track them as module aliases. Main entries default to
    /// "main" and can be overridden with the `--package_json_entry_names` option.
    // port: RewriteJsonToModule#visitScript
    fn visit_script(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !n.has_one_child(t) || !n.get_first_child(t).unwrap().is_expr_result(t) {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(compiler, n, &JSON_UNEXPECTED_TOKEN, &[]));
            return;
        }

        let mut jsdoc = JSDocInfo::builder();
        jsdoc.record_file_overview("Suppresses undefined var goog error");
        jsdoc.record_suppression("undefinedVars");
        let info = jsdoc.build();
        n.set_jsdoc_info(t, info);

        let json_object = n.get_first_first_child(t).unwrap().detach(t);
        n.remove_first_child(t);

        let input = t.get_input().unwrap().clone();
        let module_name = input.get_path(t.get_compiler()).to_module_name();

        let name = IR::name(t, module_name.as_str()).srcref(t, json_object);
        let var = IR::var_with_value(t, name, json_object).srcref(t, json_object);
        n.add_child_to_front(t, var);

        let goog = IR::name(t, "goog");
        let provide = IR::getprop(t, goog, "provide");
        let module_name_string = IR::string(t, module_name.as_str());
        let call = IR::call(t, provide, &[module_name_string]);
        let expr_result = IR::expr_result(t, call).srcref_tree_if_missing(t, n);
        n.add_child_to_front(t, expr_result);

        let input_path = input.get_source_file().get_name().to_string();
        if input_path.ends_with("/package.json") && json_object.is_object_lit(t) {
            let possible_main_entries = t
                .get_compiler()
                .get_options()
                .get_package_json_entry_names()
                .clone();

            for entry_name in &possible_main_entries {
                let entry = NodeUtil::get_first_prop_matching_key(
                    t,
                    json_object,
                    &JsString::from(entry_name.as_str()),
                );

                if let Some(entry) = entry
                    && (entry.is_string_lit(t) || entry.is_object_lit(t))
                {
                    let dir_name =
                        input_path[..input_path.len() - "package.json".len()].to_string();

                    if entry.is_string_lit(t) {
                        self.package_json_main_entries.insert(
                            input_path.clone(),
                            format!("{dir_name}{}", entry.get_string(t).to_string_lossy()),
                        );
                        break;
                    } else if entry.is_object_lit(t) {
                        check_state!(entry_name == "browser", "%s", entry_name);

                        // don't break if we're processing a browser field that is an object
                        // literal because one of its entries may override the package.json
                        // main, which we will get in the next iteration.
                        self.process_browser_field_advanced_usage(t, &dir_name, entry);
                    }
                }
            }
        }

        t.report_code_change();
    }

    /// For browser field entries in package.json files that are used in an advanced manner
    /// (https://github.com/defunctzombie/package-browser-field-spec/#replace-specific-files---advanced),
    /// track the entries in that object literal as module file replacements.
    // port: RewriteJsonToModule#processBrowserFieldAdvancedUsage
    fn process_browser_field_advanced_usage(
        &mut self,
        t: &mut NodeTraversal<'_>,
        dir_name: &str,
        entry: NodeId,
    ) {
        let mut child = entry.get_first_child(t);
        while let Some(c) = child {
            let value = c.get_first_child(t).unwrap();

            check_state!(c.is_string_key(t) && (value.is_string_lit(t) || value.is_false(t)));

            let mut path = c.get_string(t).to_string_lossy();

            if path.starts_with(ModuleLoader::DEFAULT_FILENAME_PREFIX) {
                path = path[ModuleLoader::DEFAULT_FILENAME_PREFIX.len()..].to_string();
            }

            let replacement = if value.is_string_lit(t) {
                format!("{dir_name}{}", value.get_string(t).to_string_lossy())
            } else {
                ModuleLoader::JSC_BROWSER_SKIPLISTED_MARKER.to_string()
            };

            self.package_json_main_entries
                .insert(format!("{dir_name}{path}"), replacement);
            child = c.get_next(t);
        }
    }
}

impl CompilerPass for RewriteJsonToModule {
    /// Module rewriting is done a on per-file basis prior to main compilation. The root node for
    /// each file is a SCRIPT - not the typical jsRoot of other passes.
    // port: RewriteJsonToModule#process
    fn process(&mut self, compiler: &mut Compiler, _externs: NodeId, root: NodeId) {
        check_state!(root.is_script(compiler));
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for RewriteJsonToModule {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RewriteJsonToModule#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::SCRIPT => {
                if !n.has_one_child(t) {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(compiler, n, &JSON_UNEXPECTED_TOKEN, &[]));
                } else {
                    self.visit_script(t, n);
                }
                return;
            }
            Token::OBJECTLIT
            | Token::ARRAYLIT
            | Token::NUMBER
            | Token::TRUE
            | Token::FALSE
            | Token::NULL
            | Token::STRINGLIT => {}
            Token::STRING_KEY => {
                if !n.is_quoted_string_key(t) || !n.has_one_child(t) {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(compiler, n, &JSON_UNEXPECTED_TOKEN, &[]));
                }
            }
            Token::EXPR_RESULT => {
                if !parent.unwrap().is_script(t) {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(compiler, n, &JSON_UNEXPECTED_TOKEN, &[]));
                }
            }
            _ => {
                let compiler = t.get_compiler();
                compiler.report(JSError::make(compiler, n, &JSON_UNEXPECTED_TOKEN, &[]));
            }
        }

        if n.get_lineno(t) == 1 {
            // We wrapped the expression in parens so our first-line columns are off by one.
            // We need to correct for this.
            let charno = n.get_charno(t);
            n.set_lineno_charno(t, 1, charno - 1);
            t.report_code_change();
        }
    }
}
