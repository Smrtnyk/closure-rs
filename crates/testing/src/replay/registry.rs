/*
 * Copyright 2026 The closure-rs Authors.
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
/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! Exact Java signatures resolved while the Java corpus replay ran. No Rust overload guesses.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, Object, invoke_lambda},
    throwable::{Throwable, assert_that},
};
use closure_rhino::fx_hash::IndexMap;
use std::{cell::RefCell, path::Path, rc::Rc};
pub type BorrowedEntry =
    fn(&mut Ctx, Vec<DslValue>, &mut crate::jscomp_api::Compiler) -> Result<DslValue, Throwable>;
pub type Entry = fn(&mut Ctx, Vec<DslValue>) -> Result<DslValue, Throwable>;
#[derive(Clone, Debug)]
pub struct Resolution {
    pub signature: String,
    pub parameters: Vec<String>,
    pub widened: bool,
}
#[derive(Clone)]
pub struct Registry {
    resolutions: Rc<IndexMap<(String, String, usize), Resolution>>,
    entries: Rc<IndexMap<String, Entry>>,
    borrowed_entries: Rc<IndexMap<String, BorrowedEntry>>,
}
impl Registry {
    // port: ReplayDsl#pick (resolved corpus TSV)
    pub fn from_tsv(tsv: &str) -> Result<Self, Throwable> {
        let mut resolutions = IndexMap::<_, _>::default();
        for (index, line) in tsv.lines().enumerate().skip(1) {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<_> = line.split('\t').collect();
            if parts.len() < 4 {
                return Err(bad(&format!("signature TSV row {}", index + 1)));
            }
            let parameters = parts[3]
                .split_once('(')
                .and_then(|(_, p)| p.strip_suffix(')'))
                .ok_or_else(|| bad("invalid resolved signature"))?;
            let parameters = if parameters.is_empty() {
                vec![]
            } else {
                parameters.split(',').map(String::from).collect::<Vec<_>>()
            };
            let key = (parts[0].into(), parts[1].into(), parameters.len());
            let r = Resolution {
                signature: format!("{}#{}", parts[2], parts[3]),
                parameters,
                widened: parts.get(4).is_some_and(|s| *s == "widened"),
            };
            if resolutions.insert(key, r).is_some() {
                return Err(bad("duplicate signature lookup/arity"));
            }
        }
        let mut out = Self {
            resolutions: Rc::new(resolutions),
            entries: Rc::new(IndexMap::<_, _>::default()),
            borrowed_entries: Rc::new(IndexMap::<_, _>::default()),
        };
        out.register_value_helpers();
        crate::replay::replace_messages_helpers::register(&mut out);
        crate::replay::api_passes_helpers::register(&mut out);
        out.register(
            "com.google.javascript.jscomp.MakeDeclaredNamesUniqueTest_Helpers#<init>()",
            crate::replay::make_declared_names_unique_helpers::holder,
        );
        out.register(
            "com.google.javascript.jscomp.MakeDeclaredNamesUniqueTest_Helpers$Processor#<init>(com.google.javascript.jscomp.MakeDeclaredNamesUniqueTest_Helpers,com.google.javascript.jscomp.Compiler)",
            crate::replay::make_declared_names_unique_helpers::processor,
        );
        out.register(
            "com.google.javascript.jscomp.RewriteGoogJsImportsTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.Compiler)",
            crate::replay::rewrite_goog_js_imports_helpers::rewrite_goog_js_imports_test_get_processor_pass,
        );
        out.register(
            "com.google.javascript.jscomp.CheckGoogJsImportTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.Compiler)",
            crate::replay::rewrite_goog_js_imports_helpers::check_goog_js_import_test_get_processor_pass,
        );
        out.register(
            "com.google.javascript.jscomp.RenameVarsTestHelpers#<init>()",
            crate::replay::native_finalize_rename::rename_vars_test_helpers,
        );
        out.register(
            "com.google.javascript.jscomp.RewriteDynamicImportsTest_Helpers#<init>()",
            crate::replay::rewrite_dynamic_imports_test_helpers::holder,
        );
        out.register(
            "com.google.javascript.jscomp.RewriteDynamicImportsTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.RewriteDynamicImportsTest_Helpers)",
            crate::replay::rewrite_dynamic_imports_test_helpers::get_processor_init,
        );
        out.register(
            "com.google.javascript.jscomp.RewriteDynamicImportsTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
            crate::replay::rewrite_dynamic_imports_test_helpers::get_processor,
        );
        out.register(
            "com.google.javascript.jscomp.NormalizeTest_Helpers$RenamingConstantPropertiesTester#<init>()",
            crate::replay::normalize_test_helpers::tester,
        );
        out.register(
            "com.google.javascript.jscomp.NormalizeTest_Helpers$RenamingConstantPropertiesTester#getProcessor(com.google.javascript.jscomp.Compiler)",
            crate::replay::normalize_test_helpers::get_processor,
        );
        crate::replay::transpile_es2015_natives::register(&mut out);
        crate::replay::transpile_infra_natives::register(&mut out);
        crate::replay::transpile_misc_natives::register(&mut out);
        out.register(
            "com.google.javascript.jscomp.InlineFunctionsTest_Helpers#<init>()",
            crate::replay::inline_functions_test_helpers::holder,
        );
        out.register(
            "com.google.javascript.jscomp.InlineFunctions#<init>(com.google.javascript.jscomp.AbstractCompiler,java.util.function.Supplier,com.google.javascript.jscomp.CompilerOptions$Reach,boolean,boolean,int)",
            crate::replay::inline_functions_test_helpers::inline_functions,
        );
        out.register(
            "com.google.javascript.jscomp.TypedScopeCreatorTest_Helpers#<init>()",
            crate::replay::typed_scope_creator_test_helpers::holder,
        );
        out.register(
            "com.google.javascript.jscomp.TypedScopeCreatorTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)",
            crate::replay::typed_scope_creator_test_helpers::get_processor,
        );
        out.register(
            "com.google.javascript.jscomp.ScopedAliasesTest_Helpers$TypeVerifyingPass#<init>(com.google.javascript.jscomp.Compiler)",
            crate::replay::scoped_aliases_test_helpers::type_verifying_pass,
        );
        out.register(
            "com.google.javascript.jscomp.ScopedAliasesTest_Helpers$TypeVerifyingPass#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
            crate::replay::scoped_aliases_test_helpers::process,
        );
        {
            use crate::replay::optimize_calls_helpers as oc;
            for (signature, entry) in [
                (
                    "com.google.javascript.jscomp.OptimizeCalls#builder()",
                    oc::builder as Entry,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeCalls$Builder#setCompiler(com.google.javascript.jscomp.AbstractCompiler)",
                    oc::set_compiler,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeCalls$Builder#setConsiderExterns(boolean)",
                    oc::set_consider_externs,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeCalls$Builder#addPass(com.google.javascript.jscomp.OptimizeCalls$CallGraphCompilerPass)",
                    oc::add_pass,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeCalls$Builder#build()",
                    oc::build,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeCallsTest_Helpers$ReferencesCapturingPass#<init>()",
                    oc::references_capturing_pass,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifier$Driver#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oc::pure_function_identifier_driver,
                ),
            ] {
                out.register(signature, entry);
            }
        }
        {
            use crate::replay::pure_function_identifier_helpers as pfi;
            for (signature, entry) in [
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers#<init>()",
                    pfi::holder as Entry,
                ),
                (
                    "com.google.javascript.jscomp.CompilerTestCase#postcondition(com.google.javascript.jscomp.CompilerTestCase$Postcondition)",
                    pfi::postcondition,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$NoSideEffectCallEnumerator#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers,com.google.javascript.jscomp.Compiler,com.google.javascript.jscomp.CompilerPass)",
                    pfi::no_side_effect_call_enumerator,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$AssertPureCallsMarkedPostcondition#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers,java.util.List,com.google.javascript.jscomp.CompilerTestCase$Postcondition)",
                    pfi::assert_pure_calls_marked_postcondition,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testFunctionProperties1#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_function_properties1,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testTaggedTemplatelit_propagatesCalleeSideEffects#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_tagged_templatelit_propagates_callee_side_effects,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache_withKeyFn#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache_with_key_fn,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache_anonymousFn#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache_anonymous_fn,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache_anonymousFn_hasSideEffects#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache_anonymous_fn_has_side_effects,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache_hasSideEffects#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache_has_side_effects,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache_withKeyFn_hasSideEffects#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache_with_key_fn_has_side_effects,
                ),
                (
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$Post_testCallCache_propagatesSideEffects#<init>(com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers)",
                    pfi::post_test_call_cache_propagates_side_effects,
                ),
            ] {
                out.register(signature, entry);
            }
        }
        {
            use crate::replay::optimize_calls_task_helpers as oct;
            for (signature, entry) in [
                (
                    "com.google.javascript.jscomp.OptimizeReturns#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oct::optimize_returns as Entry,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeParameters#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oct::optimize_parameters,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeConstructors#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oct::optimize_constructors,
                ),
                (
                    "com.google.javascript.jscomp.DevirtualizeMethods#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oct::devirtualize_methods,
                ),
                (
                    "com.google.javascript.jscomp.InlineObjectLiterals#<init>(com.google.javascript.jscomp.AbstractCompiler,java.util.function.Supplier)",
                    oct::inline_object_literals,
                ),
                (
                    "com.google.javascript.jscomp.InlineProperties#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oct::inline_properties,
                ),
                (
                    "com.google.javascript.jscomp.InlineSimpleMethods#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                    oct::inline_simple_methods,
                ),
                (
                    "com.google.javascript.jscomp.InlinePropertiesTest_Helpers$Anon1#<init>(com.google.javascript.jscomp.CompilerPass,com.google.javascript.jscomp.CompilerPass)",
                    oct::inline_properties_anon1,
                ),
                (
                    "com.google.javascript.jscomp.OptimizeCallsIntegrationTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.Compiler)",
                    oct::optimize_calls_integration_get_processor_pass,
                ),
            ] {
                out.register(signature, entry);
            }
        }
        out.register(
            "com.google.javascript.jscomp.InlineVariables#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.InlineVariables$Mode)",
            crate::replay::inline_variables_helpers::inline_variables,
        );
        out.register(
            "com.google.javascript.jscomp.FlowSensitiveInlineVariables#<init>(com.google.javascript.jscomp.AbstractCompiler)",
            crate::replay::inline_variables_helpers::flow_sensitive_inline_variables,
        );

        crate::replay::inline_and_collapse_properties_helpers::register(&mut out);
        for (signature, entry) in crate::replay::native_registry::HELPER_HOLDERS {
            out.register(signature, entry);
        }
        out.register_with_compiler(
            "java.lang.Iterable#forEach(java.util.function.Consumer)",
            for_each_with_compiler,
        );
        {
            use crate::replay::var_checks_helpers as v;
            out.register(
                "com.google.javascript.jscomp.VariableReferenceCheck#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                v::variable_reference_check,
            );
            out.register(
                "com.google.javascript.jscomp.StrictModeCheck#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.CheckLevel)",
                v::strict_mode_check,
            );
            out.register(
                "com.google.javascript.jscomp.DeclaredGlobalExternsOnWindow#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                v::declared_global_externs_on_window,
            );
            out.register_with_compiler(
                "com.google.javascript.jscomp.ConstCheck#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                v::const_check_with_compiler,
            );
            out.register_with_compiler(
                "com.google.javascript.jscomp.ConstCheck#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
                v::process_with_compiler,
            );
            out.register_with_compiler(
                "com.google.javascript.jscomp.InferConsts#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                v::infer_consts_with_compiler,
            );
            out.register_with_compiler(
                "com.google.javascript.jscomp.InferConsts#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
                v::process_with_compiler,
            );
            out.register(
                "com.google.javascript.jscomp.VarCheckTest_Helpers#<init>()",
                v::var_check_test_helpers,
            );
            out.register(
                "com.google.javascript.jscomp.VarCheckTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.VarCheckTest_Helpers,com.google.javascript.jscomp.Compiler)",
                v::var_check_get_processor_pass,
            );
            out.register(
                "com.google.javascript.jscomp.ValidityCheckTest_Helpers#<init>()",
                v::validity_check_test_helpers,
            );
            out.register(
                "com.google.javascript.jscomp.ValidityCheckTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.ValidityCheckTest_Helpers,com.google.javascript.jscomp.Compiler,java.lang.String)",
                v::validity_check_get_processor,
            );
            out.register(
                "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExternsTest_Helpers#<init>()",
                v::remove_unnecessary_synthetic_externs_test_helpers,
            );
            out.register(
                "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExternsTest_Helpers#setUp()",
                v::synthetic_externs_set_up,
            );
            for (method, entry) in v::SYNTHETIC_EXTERNS_PROLOGUES {
                out.register(
                    &format!(
                        "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExternsTest_Helpers#{method}()"
                    ),
                    *entry,
                );
            }
            out.register_with_compiler(
                "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExterns#<init>(com.google.javascript.jscomp.AbstractCompiler)",
                v::remove_unnecessary_synthetic_externs,
            );
            out.register_with_compiler(
                "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExterns#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
                v::process_with_compiler,
            );
        }
        out.register_with_compiler(
            "com.google.javascript.jscomp.Denormalize#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
            crate::replay::native_finalize_rename::denormalize_process,
        );
        Ok(out)
    }
    // port: ReplayMain#main (--sig-out manifest)
    pub fn load(corpus: &Path) -> Result<Self, Throwable> {
        Self::from_tsv(
            &std::fs::read_to_string(corpus.join("replay_signatures.tsv"))
                .map_err(|e| bad(&e.to_string()))?,
        )
    }
    // port: ReplayDsl#pick
    pub fn resolve(
        &self,
        descriptor: &str,
        lookup: &str,
        arity: usize,
    ) -> Result<&Resolution, Throwable> {
        self.resolutions
            .get(&(descriptor.into(), lookup.into(), arity))
            .ok_or_else(|| bad(&format!("no TSV row for ({descriptor}, {lookup}, {arity})")))
    }
    // port: ReplayDsl#helper (non-static inner constructor resolution)
    pub fn has_resolution(&self, descriptor: &str, lookup: &str, arity: usize) -> bool {
        self.resolutions
            .contains_key(&(descriptor.into(), lookup.into(), arity))
    }
    // port: ReplayDsl#invoke (native method registration)
    pub fn register(&mut self, signature: &str, entry: Entry) {
        Rc::make_mut(&mut self.entries).insert(signature.into(), entry);
    }
    // port: ReplayDsl#invoke (constructor while the actual compiler is borrowed by a factory)
    pub fn register_with_compiler(&mut self, signature: &str, entry: BorrowedEntry) {
        Rc::make_mut(&mut self.borrowed_entries).insert(signature.into(), entry);
    }
    // port: ReplayDsl#invoke (borrowed native dispatch)
    pub fn borrowed_entry(&self, signature: &str) -> Option<BorrowedEntry> {
        self.borrowed_entries.get(signature).copied()
    }
    // port: ReplayDsl#invoke
    pub fn entry(&self, signature: &str) -> Option<Entry> {
        self.entries.get(signature).copied()
    }
    // port: ReplayDsl#invoke (ported JDK and Guava value methods)
    fn register_value_helpers(&mut self) {
        let signatures = self
            .resolutions
            .values()
            .map(|r| r.signature.clone())
            .collect::<Vec<_>>();
        for signature in signatures {
            let f: Option<Entry> = if signature
                .starts_with("com.google.common.collect.ImmutableSet#of(")
            {
                Some(immutable_set)
            } else if signature.starts_with("com.google.common.collect.ImmutableList#of(") {
                Some(immutable_list)
            } else if signature.starts_with("com.google.common.collect.ImmutableMap#of(") {
                Some(immutable_map)
            } else {
                match signature.as_str() {
                    "java.lang.String#charAt(int)" => Some(char_at),
                    "java.lang.Class#forName(java.lang.String)" => Some(class_for_name),
                    "java.util.ArrayList#get(int)" => Some(list_get),
                    "java.util.HashMap#<init>(java.util.Map)" => Some(map_copy),
                    "com.google.common.base.Joiner#on(java.lang.String)" => Some(joiner_on),
                    "com.google.common.base.Joiner#join(java.lang.Iterable)" => Some(joiner_join),
                    "com.google.common.base.Preconditions#checkNotNull(java.lang.Object)" => {
                        Some(check_not_null)
                    }
                    "com.google.common.collect.RegularImmutableSet#contains(java.lang.Object)" => {
                        Some(set_contains)
                    }
                    "java.lang.Iterable#forEach(java.util.function.Consumer)" => Some(for_each),
                    "com.google.common.collect.ImmutableMap$Builder#<init>()" => Some(map_builder),
                    "com.google.common.collect.ImmutableMap$Builder#put(java.lang.Object,java.lang.Object)" => {
                        Some(map_builder_put)
                    }
                    "com.google.common.collect.ImmutableMap$Builder#buildOrThrow()" => {
                        Some(map_builder_build)
                    }
                    "com.google.common.truth.Truth#assertThat(java.lang.Boolean)" => {
                        Some(boolean_subject)
                    }
                    "com.google.common.truth.BooleanSubject#isTrue()" => Some(is_true),
                    "com.google.javascript.jscomp.CompilerTestCase#makePassFactory(java.lang.String,java.util.function.Function)" => {
                        Some(make_pass_factory)
                    }
                    _ => None,
                }
            };
            if let Some(f) = f
                .or_else(|| crate::replay::native_registry::entry(&signature))
                .or_else(|| crate::replay::native_finalize_rename::entry(&signature))
                .or_else(|| crate::replay::disambiguate_test_helpers::entry(&signature))
                .or_else(|| crate::replay::api_passes_helpers::entry(&signature))
                .or_else(|| crate::replay::extern_exports_pass_test_helpers::entry(&signature))
                .or_else(|| crate::replay::native_peephole::entry(&signature))
                .or_else(|| crate::replay::polyfills::entry(&signature))
                .or_else(|| crate::replay::remove_unused_code_helpers::entry(&signature))
                .or_else(|| crate::replay::native_closure_primitives::entry(&signature))
                .or_else(|| crate::replay::native_closure_rewrite_module::entry(&signature))
                .or_else(|| crate::replay::native_conformance::entry(&signature))
                .or_else(|| crate::replay::native_es_modules::entry(&signature))
                .or_else(|| crate::replay::native_access_controls::entry(&signature))
                .or_else(|| crate::replay::native_lint::entry(&signature))
                .or_else(|| crate::replay::replace_messages_helpers::entry(&signature))
                .or_else(|| crate::replay::cross_chunk::entry(&signature))
                .or_else(|| crate::replay::native_type_check::entry(&signature))
                .or_else(|| crate::replay::alpha_integration_natives::entry(&signature))
                .or_else(|| crate::replay::integration_test_helpers::entry(&signature))
            {
                self.register(&signature, f);
            }
            let borrowed = crate::replay::polyfills::borrowed_entry(&signature)
                .or_else(|| crate::replay::native_closure_primitives::borrowed_entry(&signature))
                .or_else(|| {
                    crate::replay::native_closure_rewrite_module::borrowed_entry(&signature)
                })
                .or_else(|| crate::replay::native_type_inference::borrowed_entry(&signature))
                .or_else(|| crate::replay::native_conformance::borrowed_entry(&signature))
                .or_else(|| crate::replay::native_es_modules::borrowed_entry(&signature))
                .or_else(|| crate::replay::remove_unused_code_helpers::borrowed_entry(&signature))
                .or_else(|| crate::replay::native_lint::borrowed_entry(&signature))
                .or_else(|| crate::replay::cross_chunk::borrowed_entry(&signature))
                .or_else(|| crate::replay::native_type_check::borrowed_entry(&signature));
            if let Some(f) = borrowed {
                self.register_with_compiler(&signature, f);
            }
        }
        // ReplayDsl#helper instantiates a helper holder through its declared no-argument
        // constructor, which has no recorded signature row.
        if let Some(f) = crate::replay::remove_unused_code_helpers::entry(
            crate::replay::remove_unused_code_helpers::TEST_HELPERS_INIT,
        ) {
            self.register(
                crate::replay::remove_unused_code_helpers::TEST_HELPERS_INIT,
                f,
            );
        }
        if let Some(f) = crate::replay::polyfills::entry(
            crate::replay::polyfills::REWRITE_POLYFILLS_TEST_HELPERS_INIT,
        ) {
            self.register(
                crate::replay::polyfills::REWRITE_POLYFILLS_TEST_HELPERS_INIT,
                f,
            );
        }
    }
}
// port: ImmutableSet#of
fn immutable_set(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut items: Vec<DslValue> = vec![];
    for v in args {
        if matches!(v, DslValue::Null) {
            return Err(Throwable::Exception {
                class: "java.lang.NullPointerException".into(),
                message: None,
            });
        }
        if !items.iter().any(|x| x.equals(&v)) {
            items.push(v);
        }
    }
    let class = if items.len() == 1 {
        "com.google.common.collect.SingletonImmutableSet"
    } else {
        "com.google.common.collect.RegularImmutableSet"
    };
    Ok(DslValue::Typed {
        class: class.into(),
        value: Box::new(DslValue::Set(items)),
    })
}
// port: ImmutableList#of
fn immutable_list(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if args.iter().any(|v| matches!(v, DslValue::Null)) {
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    }
    Ok(DslValue::Typed {
        class: if args.len() == 1 {
            "com.google.common.collect.SingletonImmutableList"
        } else {
            "com.google.common.collect.RegularImmutableList"
        }
        .into(),
        value: Box::new(DslValue::List(args)),
    })
}
// port: ImmutableMap#of
fn immutable_map(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut items: Vec<(DslValue, DslValue)> = vec![];
    for pair in args.chunks_exact(2) {
        if items.iter().any(|(k, _)| k.equals(&pair[0])) {
            return Err(Throwable::Exception {
                class: "java.lang.IllegalArgumentException".into(),
                message: Some("Multiple entries with same key".into()),
            });
        }
        items.push((pair[0].clone(), pair[1].clone()));
    }
    Ok(DslValue::Typed {
        class: "com.google.common.collect.RegularImmutableMap".into(),
        value: Box::new(DslValue::Map(items)),
    })
}
// port: String#charAt
fn char_at(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::String(s), DslValue::Int(i)] = args.as_slice() else {
        return Err(bad("String.charAt arguments"));
    };
    let Some(c) = usize::try_from(*i).ok().and_then(|i| s.as_units().get(i)) else {
        return Err(Throwable::Exception {
            class: "java.lang.StringIndexOutOfBoundsException".into(),
            message: Some(format!("Index {i} out of bounds for length {}", s.length())),
        });
    };
    Ok(DslValue::Char(*c))
}
// port: Class#forName
fn class_for_name(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::String(s)] = args.as_slice() else {
        return Err(bad("Class.forName arguments"));
    };
    Ok(DslValue::Class(ctx.map_class(&s.to_string_lossy())))
}
// port: ArrayList#get
fn list_get(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v, DslValue::Int(i)] = args.as_slice() else {
        return Err(bad("List.get arguments"));
    };
    let DslValue::List(items) = v.untyped() else {
        return Err(bad("List.get receiver"));
    };
    usize::try_from(*i)
        .ok()
        .and_then(|i| items.get(i))
        .cloned()
        .ok_or_else(|| Throwable::Exception {
            class: "java.lang.IndexOutOfBoundsException".into(),
            message: Some(format!(
                "Index {i} out of bounds for length {}",
                items.len()
            )),
        })
}
// port: HashMap#HashMap(Map)
fn map_copy(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v] = args.as_slice() else {
        return Err(bad("HashMap arguments"));
    };
    let DslValue::Map(m) = v.untyped() else {
        return Err(bad("HashMap input"));
    };
    Ok(DslValue::Typed {
        class: "java.util.HashMap".into(),
        value: Box::new(DslValue::Map(m.clone())),
    })
}
// port: Joiner#on
fn joiner_on(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v] = args.as_slice() else {
        return Err(bad("Joiner.on arguments"));
    };
    Ok(new_object(
        "com.google.common.base.Joiner",
        [("separator", v.clone())],
    ))
}
// port: Joiner#join(Iterable)
fn joiner_join(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(o), v] = args.as_slice() else {
        return Err(bad("Joiner.join arguments"));
    };
    let DslValue::String(sep) = &o.borrow().fields["separator"] else {
        return Err(bad("Joiner separator"));
    };
    let items = match v.untyped() {
        DslValue::List(x) | DslValue::Set(x) => x,
        _ => return Err(bad("Joiner iterable")),
    };
    let mut out = Vec::new();
    for (i, x) in items.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(sep.as_units());
        }
        match x {
            DslValue::String(s) => out.extend_from_slice(s.as_units()),
            DslValue::Int(n) => out.extend(n.to_string().encode_utf16()),
            DslValue::Bool(b) => out.extend(b.to_string().encode_utf16()),
            _ => return Err(bad("Joiner value toString")),
        }
    }
    Ok(DslValue::String(
        closure_rhino::js_string::JsString::from_units(out),
    ))
}
// port: Preconditions#checkNotNull
fn check_not_null(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v] = args.as_slice() else {
        return Err(bad("checkNotNull arguments"));
    };
    if matches!(v, DslValue::Null) {
        Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        })
    } else {
        Ok(v.clone())
    }
}
// port: ImmutableSet#contains
fn set_contains(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v, w] = args.as_slice() else {
        return Err(bad("Set.contains arguments"));
    };
    let DslValue::Set(items) = v.untyped() else {
        return Err(bad("Set.contains receiver"));
    };
    Ok(DslValue::Bool(items.iter().any(|v| v.equals(w))))
}
// port: Iterable#forEach
fn for_each(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v, DslValue::Lambda(l)] = args.as_slice() else {
        return Err(bad("forEach arguments"));
    };
    let items = match v.untyped() {
        DslValue::List(x) | DslValue::Set(x) => x,
        _ => return Err(bad("forEach receiver")),
    };
    for item in items {
        invoke_lambda(l, vec![item.clone()], ctx)?;
    }
    Ok(DslValue::Null)
}
// port: ImmutableMap.Builder#Builder
fn map_builder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(new_object(
        "com.google.common.collect.ImmutableMap$Builder",
        [("entries", DslValue::List(vec![]))],
    ))
}
// port: ImmutableMap.Builder#put
fn map_builder_put(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(o), k, v] = args.as_slice() else {
        return Err(bad("Builder.put arguments"));
    };
    if let DslValue::List(entries) = o.borrow_mut().fields.get_mut("entries").unwrap() {
        entries.push(DslValue::List(vec![k.clone(), v.clone()]));
    }
    Ok(DslValue::Object(o.clone()))
}
// port: ImmutableMap.Builder#buildOrThrow
fn map_builder_build(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(o)] = args.as_slice() else {
        return Err(bad("Builder.build arguments"));
    };
    let DslValue::List(entries) = &o.borrow().fields["entries"] else {
        return Err(bad("builder entries"));
    };
    let mut args = Vec::new();
    for e in entries {
        if let DslValue::List(pair) = e {
            args.extend_from_slice(pair);
        }
    }
    immutable_map(ctx, args)
}
// port: Truth#assertThat(Boolean)
fn boolean_subject(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [v] = args.as_slice() else {
        return Err(bad("assertThat arguments"));
    };
    Ok(new_object(
        "com.google.common.truth.BooleanSubject",
        [("actual", v.clone())],
    ))
}
// port: BooleanSubject#isTrue
fn is_true(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(o)] = args.as_slice() else {
        return Err(bad("isTrue receiver"));
    };
    assert_that(
        matches!(o.borrow().fields.get("actual"), Some(DslValue::Bool(true))),
        "expected to be true",
    )?;
    Ok(DslValue::Null)
}
// port: CompilerTestCase#makePassFactory
fn make_pass_factory(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::String(name), DslValue::Lambda(factory)] = args.as_slice() else {
        return Err(bad("makePassFactory arguments"));
    };
    let (native, token) =
        crate::replay::native_factories::create(name.to_string_lossy(), factory.clone());
    Ok(DslValue::PassFactory {
        name: name.to_string_lossy(),
        factory: factory.clone(),
        native,
        token,
    })
}
// port: ReplayValues#instantiate (ported value helpers)
fn new_object<const N: usize>(class: &str, fields: [(&str, DslValue); N]) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.into(),
        fields: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        field_types: IndexMap::<_, _>::default(),
    })))
}
// port: ReplayDsl#pick (resolution error)
fn bad(s: &str) -> Throwable {
    Throwable::HarnessError(s.into())
}

// port: Iterable#forEach (native compiler loan shared with the DSL consumer)
fn for_each_with_compiler(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [values, DslValue::Lambda(consumer)] = args.as_slice() else {
        return Err(bad("Iterable.forEach arguments"));
    };
    let (DslValue::List(values) | DslValue::Set(values)) = values.untyped() else {
        return Err(bad("Iterable.forEach receiver"));
    };
    for value in values {
        crate::replay::replay_dsl::invoke_lambda_with_compiler(
            consumer,
            vec![value.clone()],
            ctx,
            Some(compiler),
        )?;
    }
    Ok(DslValue::Null)
}
