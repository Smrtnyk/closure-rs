/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewritePolyfills.java.

//! Injects polyfill libraries to ensure that ES6+ library functions are available. Port of
//! `RewritePolyfills.java`.
//!
//! Also runs if polyfill isolation is enabled, even if polyfill injection is disabled, in order
//! to prevent deletion of a required library function by dead code elimination.
//!
//! TODO(b/120486392): consider merging this pass with `InjectRuntimeLibraries` and
//! `InjectTranspilationRuntimeLibraries`.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_options::LanguageMode,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js::runtime_js_lib_manager::RuntimeJsLibManager,
    js_error::JSError,
    node_util::NodeUtil,
    polyfill_usage_finder::{self, Kind, PolyfillUsage, PolyfillUsageFinder, Polyfills},
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_resources::resources::resource_loader::ResourceLoader;
use closure_rhino::{ir::IR, node::NodeId, qualified_name::QualifiedName};
use indexmap::IndexSet;
use std::sync::{Arc, LazyLock, Mutex};

// port: RewritePolyfills#INSUFFICIENT_OUTPUT_VERSION_ERROR
pub static INSUFFICIENT_OUTPUT_VERSION_ERROR: DiagnosticType = DiagnosticType::disabled(
    "JSC_INSUFFICIENT_OUTPUT_VERSION",
    "Built-in ''{0}'' not supported in output version {1}",
);

// port: RewritePolyfills#JSCOMP_POLYFILL
static JSCOMP_POLYFILL: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("$jscomp.polyfill"));

// port: RewritePolyfills
pub struct RewritePolyfills {
    runtime_js_lib_manager: Arc<Mutex<RuntimeJsLibManager>>,
    polyfills: Arc<Polyfills>,
    inject_polyfills: bool,
    isolate_polyfills: bool,
    libraries: Option<IndexSet<String>>,
    inject_polyfills_newer_than: Option<LanguageMode>,
}

impl RewritePolyfills {
    /// @param injectPolyfills if true, injects $jscomp.polyfill initializations into the first
    ///     input. if false, no polyfills are injected.
    /// @param isolatePolyfills if true, adds externs for library functions used by
    ///     `IsolatePolyfills` to prevent their deletion.
    // port: RewritePolyfills#RewritePolyfills(AbstractCompiler,boolean,boolean,LanguageMode)
    pub fn new(
        compiler: &mut AbstractCompiler,
        inject_polyfills: bool,
        isolate_polyfills: bool,
        inject_polyfills_newer_than: Option<LanguageMode>,
    ) -> Self {
        Self::new_with_manager(
            compiler.get_runtime_js_lib_manager(),
            Polyfills::from_table(&ResourceLoader::load_text_resource(
                "com.google.javascript.jscomp.RewritePolyfills",
                "js/polyfills.txt",
            )),
            inject_polyfills,
            isolate_polyfills,
            inject_polyfills_newer_than,
        )
    }

    /// For unit testing, allows instantiating RewritePolyfills with a different
    /// RuntimeJsLibManager than `AbstractCompiler#getRuntimeJsLibManager`
    // port: RewritePolyfills#RewritePolyfills(AbstractCompiler,RuntimeJsLibManager,Polyfills,boolean,boolean,LanguageMode)
    pub fn new_for_testing(
        runtime_js_lib_manager: RuntimeJsLibManager,
        polyfills: Polyfills,
        inject_polyfills: bool,
        isolate_polyfills: bool,
        inject_polyfills_newer_than: Option<LanguageMode>,
    ) -> Self {
        Self::new_with_manager(
            Arc::new(Mutex::new(runtime_js_lib_manager)),
            polyfills,
            inject_polyfills,
            isolate_polyfills,
            inject_polyfills_newer_than,
        )
    }

    fn new_with_manager(
        runtime_js_lib_manager: Arc<Mutex<RuntimeJsLibManager>>,
        polyfills: Polyfills,
        inject_polyfills: bool,
        isolate_polyfills: bool,
        inject_polyfills_newer_than: Option<LanguageMode>,
    ) -> Self {
        Self {
            runtime_js_lib_manager,
            polyfills: Arc::new(polyfills),
            inject_polyfills,
            isolate_polyfills,
            libraries: None,
            inject_polyfills_newer_than,
        }
    }

    /// `runtimeJsLibManager.ensureLibraryInjected(library, forceInjection)`.
    fn ensure_library_injected(
        &mut self,
        compiler: &mut AbstractCompiler,
        library: &str,
        force_injection: bool,
    ) -> Option<NodeId> {
        self.runtime_js_lib_manager
            .lock()
            .unwrap()
            .ensure_library_injected(compiler, library, force_injection)
    }

    // port: RewritePolyfills#injectAll
    fn inject_all(
        &mut self,
        compiler: &mut AbstractCompiler,
        libraries_to_inject: &[String],
        force_injection: bool,
    ) {
        let mut last_node = None;
        for library in libraries_to_inject {
            assert!(!library.is_empty(), "unexpected empty library");
            last_node = self.ensure_library_injected(compiler, library, force_injection);
        }
        if let Some(last_node) = last_node {
            let parent = last_node.get_parent(compiler).unwrap();
            let runtime_end = last_node.get_next(compiler);
            Self::remove_unneeded_polyfills(compiler, parent, runtime_end);
            compiler.report_change_to_enclosing_scope(parent);
        }
    }

    // Remove any $jscomp.polyfill calls whose 3rd parameter (the language version
    // that already contains the library) is the same or lower than languageOut.
    // port: RewritePolyfills#removeUnneededPolyfills
    fn remove_unneeded_polyfills(
        compiler: &mut AbstractCompiler,
        parent: NodeId,
        runtime_end: Option<NodeId>,
    ) {
        let mut node = parent.get_first_child(compiler);
        // The target environment is assumed to support the features in this set.
        let output_feature_set = compiler.get_options().get_output_feature_set();
        while let Some(current) = node {
            if Some(current) == runtime_end {
                break;
            }
            // look up the next node now, because we may be removing this one.
            let next = current.get_next(compiler);
            let polyfill_supported_feature_set =
                Self::get_polyfill_supported_feature_set(compiler, current);
            if let Some(polyfill_supported_feature_set) = polyfill_supported_feature_set
                && output_feature_set.contains(polyfill_supported_feature_set)
            {
                NodeUtil::remove_child(compiler, parent, current);
                NodeUtil::mark_functions_deleted(compiler, current);
            }
            node = next;
        }
    }

    /// If the given `Node` is a polyfill definition, return the `FeatureSet` which should be
    /// considered to already include that polyfill (making it unnecessary).
    ///
    /// Otherwise, return `null`.
    // port: RewritePolyfills#getPolyfillSupportedFeatureSet
    fn get_polyfill_supported_feature_set(
        compiler: &AbstractCompiler,
        maybe_polyfill: NodeId,
    ) -> Option<FeatureSet> {
        let mut polyfill_support_feature_set = None;
        if NodeUtil::is_expr_call(compiler, maybe_polyfill) {
            let call = maybe_polyfill.get_first_child(compiler).unwrap();
            let name = call.get_first_child(compiler).unwrap();
            if JSCOMP_POLYFILL.matches(compiler, name) {
                let native_version_str = name
                    .get_next(compiler)
                    .unwrap()
                    .get_next(compiler)
                    .unwrap()
                    .get_next(compiler)
                    .unwrap()
                    .get_string(compiler)
                    .to_string();
                // Java assigns `FeatureSet.valueOf(nativeVersionStr)` here and overwrites it on
                // the next line; the call stays for its IllegalArgumentException.
                let _ = feature_set_value_of(&native_version_str);
                polyfill_support_feature_set = Some(
                    polyfill_usage_finder::get_polyfill_supported_feature_set(&native_version_str),
                );
            }
        }
        polyfill_support_feature_set
    }

    // port: RewritePolyfills#inject
    fn inject(&mut self, compiler: &mut AbstractCompiler, polyfill_usage: PolyfillUsage) {
        let polyfill = polyfill_usage.polyfill();
        let output_feature_set = compiler.get_options().get_output_feature_set();
        let features_required_by_polyfill = feature_set_value_of(&polyfill.polyfill_version);
        if polyfill.kind == Kind::STATIC
            && !output_feature_set.contains(features_required_by_polyfill)
        {
            let name = polyfill_usage.name().to_string();
            let error = JSError::make(
                compiler,
                polyfill_usage.node(),
                &INSUFFICIENT_OUTPUT_VERSION_ERROR,
                &[&name, output_feature_set.version()],
            );
            compiler.report(error);
        }

        // The question we want to ask here is:
        // "Does the target platform already have the symbol this polyfill provides?"
        // We approximate it by asking instead:
        // "Does the target platform support all of the features that existed in the language
        // version that introduced this symbol?"
        if !output_feature_set.contains(feature_set_value_of(&polyfill.native_version))
            && !polyfill.library.is_empty()
        {
            self.libraries
                .as_mut()
                .unwrap()
                .insert(polyfill.library.clone());
        }
    }
}

/// `FeatureSet.valueOf(String)`, which throws IllegalArgumentException for an unknown name.
fn feature_set_value_of(name: &str) -> FeatureSet {
    FeatureSet::value_of(name).unwrap_or_else(|e| panic!("{e}"))
}

impl CompilerPass for RewritePolyfills {
    // port: RewritePolyfills#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        if self.isolate_polyfills {
            // Polyfill isolation requires a pass to run near the end of optimizations. That pass
            // may call into a library method injected in this pass. Adding an externs declaration
            // of that library method prevents it from being dead-code-elimiated before polyfill
            // isolation runs.
            let name = IR::name(compiler, "$jscomp$lookupPolyfilledValue");
            let jscomp_lookup_method_decl = IR::var(compiler, name);
            let synthesized_externs_ast_root = compiler.get_synthesized_externs_root();
            jscomp_lookup_method_decl.srcref_tree(compiler, synthesized_externs_ast_root);
            synthesized_externs_ast_root.add_child_to_back(compiler, jscomp_lookup_method_decl);
            compiler.report_change_to_enclosing_scope(jscomp_lookup_method_decl);
        }

        if !self.inject_polyfills && self.inject_polyfills_newer_than.is_none() {
            // Nothing left to do. Probably this pass only needed to run because
            // --isolate_polyfills is enabled but not --rewrite_polyfills.
            return;
        }
        if self.inject_polyfills {
            self.libraries = Some(IndexSet::new());
            let finder = PolyfillUsageFinder::new(Arc::clone(&self.polyfills));
            finder.traverse_excluding_guarded(compiler, root, &mut |compiler, usage| {
                self.inject(compiler, usage)
            });
        }

        let libraries_to_inject: Vec<String> =
            if let Some(inject_polyfills_newer_than) = self.inject_polyfills_newer_than {
                let polyfills_to_inject = self
                    .polyfills
                    .get_polyfills_newer_than(inject_polyfills_newer_than);
                polyfills_to_inject
                    .iter()
                    // Skip polyfills that have no associated library. This is true for language
                    // features like `Proxy` and `String.raw` that have no associated polyfill,
                    // hence there's
                    // nothing to inject here.
                    .filter(|p| !p.library.is_empty())
                    .map(|p| p.library.clone())
                    .collect()
            } else {
                self.libraries.as_ref().unwrap().iter().cloned().collect()
            };

        self.inject_all(
            compiler,
            &libraries_to_inject,
            /* forceInjection= */ self.inject_polyfills_newer_than.is_some(),
        );
    }
}
