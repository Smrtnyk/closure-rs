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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/transpile/BaseTranspiler.java.

//! Basic Transpiler implementation for outputting ES5 code.
use super::{transpile_result::TranspileResult, transpiler::Transpiler};
use crate::{
    bundle::transpilation_exception::TranspilationException,
    check_level::CheckLevel,
    compiler::Compiler,
    compiler_options::{CompilerOptions, Es6ModuleTranspilation, LanguageMode},
    deps::module_loader::{PathEscaper, ResolutionMode},
    diagnostic_groups,
    property_renaming_policy::PropertyRenamingPolicy,
    source_file::SourceFile,
    source_map::{LocationMapping, PrefixLocationMapping},
    variable_renaming_policy::VariableRenamingPolicy,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{java_lang::uri::URI, js_string::JsString};
use std::{
    fmt,
    sync::{Arc, LazyLock},
};

/// Basic Transpiler implementation for outputting ES5 code.
pub struct BaseTranspiler {
    compiler_supplier: Arc<dyn CompilerSupplierApi>,
    runtime_library_name: String,
}

impl BaseTranspiler {
    // port: BaseTranspiler#BaseTranspiler
    pub fn new(
        compiler_supplier: Arc<dyn CompilerSupplierApi>,
        runtime_library_name: impl Into<String>,
    ) -> Self {
        Self {
            compiler_supplier,
            runtime_library_name: runtime_library_name.into(),
        }
    }

    // port: BaseTranspiler#to(FeatureSet, String)
    pub fn to_with_runtime(feature_set: FeatureSet, runtime: &str) -> BaseTranspiler {
        BaseTranspiler::new(
            Arc::new(CompilerSupplier::with_output_feature_set(feature_set)),
            runtime,
        )
    }

    // port: BaseTranspiler#to(FeatureSet)
    pub fn to(feature_set: FeatureSet) -> BaseTranspiler {
        Self::to_with_runtime(feature_set, "es6_runtime")
    }
}

impl Transpiler for BaseTranspiler {
    // port: BaseTranspiler#transpile
    fn transpile(&self, path: URI, code: &JsString) -> TranspileResult {
        let result = self.compiler_supplier.compile(&path, code);
        if !result.transpiled {
            return TranspileResult::new(path, code.clone(), code.clone(), "");
        }
        TranspileResult::new(path, code.clone(), result.source, result.source_map)
    }

    // port: BaseTranspiler#runtime
    fn runtime(&self) -> JsString {
        let mut sb = JsString::from("");
        if !self.runtime_library_name.is_empty() {
            sb = sb.concat(&self.compiler_supplier.runtime(&self.runtime_library_name));
        }
        sb = sb.concat(&self.compiler_supplier.runtime("modules"));
        sb
    }
}

// port: BaseTranspiler#LATEST_TRANSPILER
pub static LATEST_TRANSPILER: LazyLock<Arc<BaseTranspiler>> =
    LazyLock::new(|| Arc::new(BaseTranspiler::to_with_runtime(FeatureSet::latest(), "")));

// port: BaseTranspiler#ES5_TRANSPILER
pub static ES5_TRANSPILER: LazyLock<Arc<BaseTranspiler>> = LazyLock::new(|| {
    Arc::new(BaseTranspiler::to(
        LanguageMode::ECMASCRIPT5.to_feature_set(),
    ))
});

/// The overridable public methods of `BaseTranspiler.CompilerSupplier` (a Java class that callers
/// and tests subclass or mock). `BaseTranspiler` calls its supplier through this trait;
/// `CompilerSupplier` is the Java class's own implementation.
pub trait CompilerSupplierApi: Send + Sync {
    // port: BaseTranspiler.CompilerSupplier#compile
    fn compile(&self, path: &URI, code: &JsString) -> CompileResult;
    // port: BaseTranspiler.CompilerSupplier#runtime
    fn runtime(&self, library: &str) -> JsString;
}

/// Wraps the Compiler into a more relevant interface, making it easy to test the Transpiler
/// without depending on implementation details of the Compiler itself. Also works around the fact
/// that the Compiler is not thread-safe (since we may do multiple transpiles concurrently), so we
/// supply a fresh instance each time when we're in single-file mode.
#[derive(Clone, Debug)]
pub struct CompilerSupplier {
    pub module_resolution: ResolutionMode,
    pub module_roots: Vec<String>,
    pub prefix_replacements: IndexMap<String, String>,
    pub output_feature_set: FeatureSet,
}

impl Default for CompilerSupplier {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerSupplier {
    // port: BaseTranspiler.CompilerSupplier#CompilerSupplier()
    pub fn new() -> Self {
        Self::with_output_feature_set(LanguageMode::ECMASCRIPT5.to_feature_set())
    }

    // port: BaseTranspiler.CompilerSupplier#CompilerSupplier(FeatureSet)
    pub fn with_output_feature_set(output_feature_set: FeatureSet) -> Self {
        // Use the default resolution mode
        Self::with_module_options(
            output_feature_set,
            CompilerOptions::new().get_module_resolution_mode(),
            Vec::new(),
            IndexMap::<_, _>::default(),
        )
    }

    /// Accepts commonly overridden options for ES6 modules to avoid needed to subclass.
    ///
    /// @param moduleResolution module resolution for resolving import paths
    /// @param prefixReplacements prefix replacements for when moduleResolution is {@link
    ///     ModuleLoader.ResolutionMode#BROWSER_WITH_TRANSFORMED_PREFIXES}
    // port: BaseTranspiler.CompilerSupplier#CompilerSupplier(FeatureSet, ResolutionMode, ImmutableList, ImmutableMap)
    pub fn with_module_options(
        output_feature_set: FeatureSet,
        module_resolution: ResolutionMode,
        module_roots: Vec<String>,
        prefix_replacements: IndexMap<String, String>,
    ) -> Self {
        Self {
            output_feature_set,
            module_resolution,
            module_roots,
            prefix_replacements,
        }
    }

    // port: BaseTranspiler.CompilerSupplier#compiler
    pub fn compiler(&self) -> Compiler {
        Compiler::new()
    }

    // port: BaseTranspiler.CompilerSupplier#options
    pub fn options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        self.set_options(&mut options);
        options
    }

    /// @LegacySetFeatureSetCaller
    // port: BaseTranspiler.CompilerSupplier#setOptions
    pub fn set_options(&self, options: &mut CompilerOptions) {
        options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
        options.legacy_set_output_feature_set(self.output_feature_set.without(Feature::MODULES));
        options.set_emit_use_strict(false);
        options.set_quote_keyword_properties(true);
        options.set_skip_non_transpilation_passes(true);
        options.set_variable_renaming(VariableRenamingPolicy::OFF);
        options.set_property_renaming(PropertyRenamingPolicy::OFF);
        options.set_wrap_goog_modules_for_whitespace_only(false);
        options.set_pretty_print(true);
        options.set_warning_level(
            diagnostic_groups::NON_STANDARD_JSDOC.clone(),
            CheckLevel::OFF,
        );
        options.set_es6_module_transpilation(Es6ModuleTranspilation::TO_COMMON_JS_LIKE_MODULES);
        options.set_module_resolution_mode(self.module_resolution);
        options.set_module_roots(self.module_roots.clone());
        options.set_browser_resolver_prefix_replacements(self.prefix_replacements.clone());

        // Transpiler often used in isolation, so references to other files will never exist.
        options.set_warning_level(diagnostic_groups::MODULE_LOAD.clone(), CheckLevel::OFF);

        // Don't escape module paths when bundling in the event paths are URLs.
        options.set_path_escaper(PathEscaper::CANONICALIZE_ONLY);
        options.set_parse_inline_source_maps(true);
        options.set_apply_input_source_maps(true);
        options.set_source_map_output_path("/dev/null".into());
        options.set_source_map_include_sources_content(true);
        // Make sourcemaps use absolute paths, so that the path is not duplicated if a build tool adds
        // a sourceurl. Exception: if the location has a scheme (like http:) then leave the path
        // intact. This makes this usable from web servers.
        options.set_source_map_location_mappings(vec![Arc::new(AbsoluteLocationMapping)]);
    }

    // port: BaseTranspiler.CompilerSupplier#createTrivialExterns
    pub fn create_trivial_externs(&self) -> Arc<SourceFile> {
        Arc::new(SourceFile::from_code("externs.js", "function Symbol() {}"))
    }

    // port: BaseTranspiler.CompilerSupplier#createEmptySource
    pub fn create_empty_source(&self) -> Arc<SourceFile> {
        Arc::new(SourceFile::from_code("empty.js", ""))
    }
}

impl CompilerSupplierApi for CompilerSupplier {
    /// Throws `TranspilationException` (with `std::panic::panic_any`) when the compile reports
    /// errors.
    // port: BaseTranspiler.CompilerSupplier#compile
    fn compile(&self, path: &URI, code: &JsString) -> CompileResult {
        let mut compiler = self.compiler();
        let result = compiler.compile_single(
            self.create_trivial_externs(),
            Arc::new(SourceFile::from_code(&path.to_string(), code.clone())),
            self.options(),
        );
        let source = compiler.to_source();
        let mut source_map = String::new();
        if let Some(result_source_map) = &result.source_map {
            // impossible, and not a big deal even if it did happen.
            let _ = result_source_map
                .lock()
                .unwrap()
                .append_to(&mut source_map, path.to_string());
        }
        let transpiled = result.transpiled_files;
        if !result.errors.is_empty() {
            std::panic::panic_any(TranspilationException::new(
                Some(compiler.get_source_excerpt_provider()),
                &compiler,
                result.errors,
                result.warnings,
            ));
        }
        CompileResult::new(
            source,
            transpiled,
            if transpiled {
                JsString::from(source_map)
            } else {
                JsString::from("")
            },
        )
    }

    // port: BaseTranspiler.CompilerSupplier#runtime
    fn runtime(&self, library: &str) -> JsString {
        let mut compiler = self.compiler();
        let mut options = self.options();
        options.set_force_library_injection(vec![library.to_owned()]);
        compiler.compile_single(
            self.create_trivial_externs(),
            self.create_empty_source(),
            options,
        );
        compiler.to_source()
    }
}

/// The `LocationMapping` lambda of `CompilerSupplier#setOptions`.
struct AbsoluteLocationMapping;

impl LocationMapping for AbsoluteLocationMapping {
    // port: BaseTranspiler.CompilerSupplier#setOptions (location mapping lambda)
    fn map(&self, location: &JsString) -> Option<JsString> {
        if let Ok(uri) = URI::new(&location.to_string_lossy())
            && uri.get_scheme().is_some()
        {
            return Some(location.clone());
        }
        // Swallow, return the absolute version below.
        PrefixLocationMapping::new("", "/").map(location)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl fmt::Display for AbsoluteLocationMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BaseTranspiler$CompilerSupplier$$Lambda")
    }
}

/// The source together with the additional compilation results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileResult {
    pub source: JsString,
    pub transpiled: bool,
    pub source_map: JsString,
}

impl CompileResult {
    // port: BaseTranspiler.CompileResult#CompileResult
    pub fn new(
        source: impl Into<JsString>,
        transpiled: bool,
        source_map: impl Into<JsString>,
    ) -> Self {
        Self {
            source: source.into(),
            transpiled,
            source_map: source_map.into(),
        }
    }
}
