/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/LazyParsedDependencyInfo.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_input::CompilerInput,
    deps::{
        dependency_info::{DependencyInfo, Require},
        simple_dependency_info::SimpleDependencyInfo,
    },
    js_error::JSError,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::fast_hash::IndexMap;

/// A DependencyInfo class that determines load flags by parsing the AST just-in-time.
///
/// The Java object keeps its AbstractCompiler; Rust callers pass it to `get_load_flags`, the one
/// method that uses it. CompilerInput (which folded isEs6Module/isGoogModule/getLoadFlags into its
/// own dependency state) runs its lazy load flags through this class.
pub struct LazyParsedDependencyInfo {
    delegate: SimpleDependencyInfo,
    ast: Option<CompilerInput>,
    load_flags: Option<IndexMap<String, String>>,
}
impl LazyParsedDependencyInfo {
    // port: LazyParsedDependencyInfo#LazyParsedDependencyInfo
    pub fn new(delegate: SimpleDependencyInfo, ast: CompilerInput) -> Self {
        Self {
            delegate,
            ast: Some(ast),
            load_flags: None,
        }
    }

    // port: LazyParsedDependencyInfo#isEs6Module
    pub fn is_es6_module(&self) -> bool {
        // Instead of doing a full parse to read all load flags, just ask the delegate, which at
        // least has this much info
        self.delegate.is_es6_module()
    }

    // port: LazyParsedDependencyInfo#isGoogModule
    pub fn is_goog_module(&self) -> bool {
        // Instead of doing a full parse to read all load flags, just ask the delegate, which at
        // least has this much info
        self.delegate.is_goog_module()
    }

    // port: LazyParsedDependencyInfo#getLoadFlags
    pub fn get_load_flags(&mut self, compiler: &mut AbstractCompiler) -> &IndexMap<String, String> {
        if self.load_flags.is_none() {
            let ast = self
                .ast
                .as_ref()
                .expect("ast is cleared only after loadFlags is set");
            // TreeMap: sorted on insertion below.
            let mut load_flags_builder: IndexMap<String, String> = IndexMap::<_, _>::default();
            let had_source_in_memory = ast.get_source_file().has_source_in_memory();
            load_flags_builder.extend(
                self.delegate
                    .get_load_flags()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone())),
            );
            let features = ast
                .get_features(compiler)
                .expect("FeatureSet missing after full parse");
            if features.has(Feature::MODULES) {
                let previous_module = load_flags_builder.get("module");
                if previous_module.is_some_and(|previous_module| previous_module != "es6") {
                    compiler.report(JSError::make_with_source_location(
                        self.get_name(),
                        /* lineno= */ -1,
                        /* charno= */ -1,
                        &crate::deps::module_loader::MODULE_CONFLICT,
                        &[self.get_name()],
                    ));
                }
                load_flags_builder.insert("module".into(), "es6".into());
            }
            let version = features.version();
            if version != "es3" {
                load_flags_builder.insert("lang".into(), version.into());
            }
            self.load_flags = Some(crate::compiler_input::sorted_flags(load_flags_builder));

            // Don't preserve the source in memory if it was loaded only for getLoadFlags().
            if !had_source_in_memory {
                ast.get_source_file().clear_cached_source();
            }
            // Don't preserve the full AST longer than necessary.  It can consume a lot of memory.
            self.ast = None;
        }
        self.load_flags.as_ref().unwrap()
    }

    // port: LazyParsedDependencyInfo#getName
    pub fn get_name(&self) -> &str {
        self.delegate.get_name()
    }

    // port: LazyParsedDependencyInfo#getPathRelativeToClosureBase
    pub fn get_path_relative_to_closure_base(&self) -> &str {
        self.delegate.get_path_relative_to_closure_base()
    }

    // port: LazyParsedDependencyInfo#getRequires
    pub fn get_requires(&self) -> &[Require] {
        self.delegate.get_requires()
    }

    // port: LazyParsedDependencyInfo#getTypeRequires
    pub fn get_type_requires(&self) -> &[String] {
        self.delegate.get_type_requires()
    }

    // port: LazyParsedDependencyInfo#getProvides
    pub fn get_provides(&self) -> &[String] {
        self.delegate.get_provides()
    }

    // port: LazyParsedDependencyInfo#getHasExternsAnnotation
    pub fn get_has_externs_annotation(&self) -> bool {
        self.delegate.get_has_externs_annotation()
    }

    // port: LazyParsedDependencyInfo#getHasNoCompileAnnotation
    pub fn get_has_no_compile_annotation(&self) -> bool {
        self.delegate.get_has_no_compile_annotation()
    }
}
