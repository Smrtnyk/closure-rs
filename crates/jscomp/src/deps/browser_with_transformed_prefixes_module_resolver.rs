/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/BrowserWithTransformedPrefixesModuleResolver.java,
//   src/com/google/javascript/jscomp/deps/ModuleResolver.java.

use super::{
    module_loader::{
        ModuleLoader, ModuleResolverFactory, PathEscaper, SharedErrorHandler,
        best_match_path_ordering,
    },
    module_resolver::{ModuleResolver, ModuleResolverBase},
};
use crate::{check_level::CheckLevel, diagnostic_type::DiagnosticType};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::java_lang::string_compare_to;
use std::sync::Arc;
pub static INVALID_AMBIGUOUS_PATH: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_AMBIGUOUS_PATH",
    "The path \"{0}\" is invalid. Expected any of the following prefixes for non-relative paths: {1}.",
);
pub struct Factory {
    prefix_replacements: IndexMap<String, String>,
}
impl Factory {
    // port: BrowserWithTransformedPrefixesModuleResolver.Factory#Factory
    pub fn new(prefix_replacements: IndexMap<String, String>) -> Self {
        Self {
            prefix_replacements,
        }
    }
}
impl ModuleResolverFactory for Factory {
    // port: BrowserWithTransformedPrefixesModuleResolver.Factory#create
    fn create(
        &self,
        paths: IndexSet<String>,
        roots: Vec<String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Arc<dyn ModuleResolver> {
        Arc::new(BrowserWithTransformedPrefixesModuleResolver::new(
            paths,
            roots,
            handler,
            escaper,
            self.prefix_replacements.clone(),
        ))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrefixReplacement {
    pub prefix: String,
    pub replacement: String,
}
impl PrefixReplacement {
    // port: BrowserWithTransformedPrefixesModuleResolver.PrefixReplacement#of
    pub fn of(prefix: &str, replacement: &str) -> Self {
        Self {
            prefix: prefix.into(),
            replacement: replacement.into(),
        }
    }
}
pub struct BrowserWithTransformedPrefixesModuleResolver {
    base: ModuleResolverBase,
    prefix_replacements: IndexSet<PrefixReplacement>,
    expected_prefixes: String,
}
impl BrowserWithTransformedPrefixesModuleResolver {
    // port: BrowserWithTransformedPrefixesModuleResolver#BrowserWithTransformedPrefixesModuleResolver
    pub fn new(
        paths: IndexSet<String>,
        roots: Vec<String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
        replacements: IndexMap<String, String>,
    ) -> Self {
        let mut prefixes: Vec<_> = replacements
            .into_iter()
            .map(|(k, v)| PrefixReplacement::of(&k, &v))
            .collect();
        prefixes.sort_by(|a, b| best_match_path_ordering(&a.prefix, &b.prefix));
        let mut expected: Vec<_> = prefixes.iter().map(|r| r.prefix.clone()).collect();
        expected.sort_by(|a, b| string_compare_to(a, b).cmp(&0));
        Self {
            base: ModuleResolverBase::new(paths, roots, handler, escaper),
            prefix_replacements: prefixes.into_iter().collect(),
            expected_prefixes: expected.join(", "),
        }
    }
    // port: BrowserWithTransformedPrefixesModuleResolver#replacePathPrefixes
    fn replace_path_prefixes(&self, module_address: Option<&str>) -> Option<String> {
        let address = module_address?;
        for p in &self.prefix_replacements {
            if let Some(tail) = address.strip_prefix(&p.prefix) {
                return Some(format!("{}{tail}", p.replacement));
            }
        }
        Some(address.into())
    }
}
impl ModuleResolver for BrowserWithTransformedPrefixesModuleResolver {
    // port: ModuleResolver#ModuleResolver (base fields)
    fn base(&self) -> &ModuleResolverBase {
        &self.base
    }
    // port: BrowserWithTransformedPrefixesModuleResolver#resolveJsModule
    fn resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
        sourcename: Option<&str>,
        lineno: i32,
        colno: i32,
    ) -> Option<String> {
        let transformed = self.replace_path_prefixes(Some(module_address)).unwrap();
        let replaced = transformed != module_address;
        if !replaced && ModuleLoader::is_ambiguous_identifier(&transformed) {
            self.base.report(
                CheckLevel::WARNING,
                super::module_resolver::make_js_error(
                    sourcename,
                    lineno,
                    colno,
                    &INVALID_AMBIGUOUS_PATH,
                    &[&transformed, &self.expected_prefixes],
                ),
            );
            return None;
        }
        self.base.locate(script_address, &transformed)
    }
    // port: BrowserWithTransformedPrefixesModuleResolver#resolveJsModuleSilently
    fn resolve_js_module_silently(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        let transformed = self.replace_path_prefixes(Some(module_address)).unwrap();
        let replaced = transformed != module_address;
        if !replaced && ModuleLoader::is_ambiguous_identifier(&transformed) {
            None
        } else {
            self.base.locate(script_address, &transformed)
        }
    }
    // port: BrowserWithTransformedPrefixesModuleResolver#resolveModuleAsPath
    fn resolve_module_as_path(&self, script_address: &str, module_address: &str) -> String {
        if ModuleLoader::is_relative_identifier(module_address) {
            return self
                .base
                .resolve_module_as_path(script_address, module_address);
        }
        let transformed = self.replace_path_prefixes(Some(module_address)).unwrap();
        ModuleLoader::normalize(&transformed, &self.base.module_root_paths)
    }
}
