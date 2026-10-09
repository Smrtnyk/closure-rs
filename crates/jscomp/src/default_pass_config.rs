/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CombinedCompilerPass.java,
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/DefaultPassConfig.java,
//   src/com/google/javascript/jscomp/J2clPass.java.

#![allow(clippy::needless_return)] // Preserve Java statement order and borrowing branches.
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization,
    combined_compiler_pass::CombinedCompilerPass,
    compiler_options::*,
    compiler_pass::CompilerPass,
    custom_pass_execution_time::CustomPassExecutionTime,
    diagnostic_groups,
    exploit_assigns::ExploitAssigns,
    j2cl_equality_same_rewriter_pass::J2clEqualitySameRewriterPass,
    j2cl_string_value_of_rewriter_pass::J2clStringValueOfRewriterPass,
    j2cl_undefined_checks_rewriter_pass::J2clUndefinedChecksRewriterPass,
    minimize_exit_points::MinimizeExitPoints,
    node_traversal::Callback,
    optimize_let_and_const_peephole::OptimizeLetAndConstPeephole,
    pass_config::PassConfig,
    pass_factory::{PassFactory, PreconditionResult},
    pass_list_builder::PassListBuilder,
    pass_names,
    peephole_collect_property_assignments::PeepholeCollectPropertyAssignments,
    peephole_fold_constants::PeepholeFoldConstants,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
    peephole_replace_known_methods::PeepholeReplaceKnownMethods,
    peephole_substitute_alternate_syntax::PeepholeSubstituteAlternateSyntax,
    preprocessor_symbol_table::CachedInstanceFactory,
    property_renaming_policy::PropertyRenamingPolicy,
    statement_fusion::StatementFusion,
    transpilation_passes::{self, ObjectDestructuringRewriteMode, TranspilationPasses},
    variable_renaming_policy::VariableRenamingPolicy,
};
use closure_parsing::{
    parser::feature_set::{Feature, FeatureSet},
    parser_runner::ParserRunner,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_argument,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::path::Path;
use std::sync::Arc;

// port: DefaultPassConfig#GENERATE_EXPORTS_ERROR
static GENERATE_EXPORTS_ERROR: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_GENERATE_EXPORTS_ERROR",
        "Exports can only be generated if export symbol/property functions are set.",
    );
pub struct DefaultPassConfig {
    options: CompilerOptions,
    preprocessor_symbol_table_factory: CachedInstanceFactory,
    pub check_side_effects: PassFactory,
    pub strip_side_effect_protection: PassFactory,
    pub suspicious_code: PassFactory,
    pub extra_requires: PassFactory,
    pub check_missing_requires: PassFactory,
    pub check_js_doc_and_es6_modules: PassFactory,
    pub generate_exports: PassFactory,
    pub generate_ijs: PassFactory,
    pub remove_extra_requires: PassFactory,
    pub export_test_functions: PassFactory,
    pub gather_raw_exports: PassFactory,
    pub closure_primitives: PassFactory,
    pub closure_provides_requires: PassFactory,
    pub angular_pass: PassFactory,
    pub replace_messages_for_chrome: PassFactory,
    pub closure_goog_scope_aliases_for_ijs: PassFactory,
    pub closure_goog_scope_aliases: PassFactory,
    pub inject_runtime_libraries: PassFactory,
    pub remove_weak_sources: PassFactory,
    pub declared_global_externs_on_window: PassFactory,
    pub check_type_import_code_references: PassFactory,
    pub closure_check_module: PassFactory,
    pub closure_rewrite_module: PassFactory,
    pub check_closure_imports: PassFactory,
    pub rewrite_goog_js_imports: PassFactory,
    pub closure_replace_get_css_name: PassFactory,
    pub create_synthetic_blocks: PassFactory,
    pub early_peephole_optimizations: PassFactory,
    pub early_inline_variables: PassFactory,
    pub peephole_optimizations: PassFactory,
    pub peephole_optimizations_once_normalized: PassFactory,
    pub peephole_optimizations_once_non_normalized: PassFactory,
    pub late_peephole_optimizations: PassFactory,
    pub check_vars: PassFactory,
    pub infer_consts: PassFactory,
    pub check_reg_exp: PassFactory,
    pub check_reg_exp_for_optimizations: PassFactory,
    pub check_variable_references: PassFactory,
    pub check_super: PassFactory,
    pub clear_typed_scope_creator_pass: PassFactory,
    pub clear_top_typed_scope_pass: PassFactory,
    pub infer_types: PassFactory,
    pub infer_js_doc_info: PassFactory,
    pub check_types: PassFactory,
    pub check_control_flow: PassFactory,
    pub check_access_controls: PassFactory,
    pub lint_checks: PassFactory,
    pub analyzer_checks: PassFactory,
    pub check_requires_and_provides_sorted: PassFactory,
    pub check_strict_mode: PassFactory,
    pub process_tweaks: PassFactory,
    pub process_defines_check: PassFactory,
    pub process_defines_optimize: PassFactory,
    pub strip_code: PassFactory,
    pub check_consts: PassFactory,
    pub rewrite_caller_code_location: PassFactory,
    pub replace_toggles: PassFactory,
    pub replace_id_generators: PassFactory,
    pub replace_strings: PassFactory,
    pub closure_code_removal: PassFactory,
    pub closure_optimize_primitives: PassFactory,
    pub rescope_global_symbols: PassFactory,
    pub convert_chunks_to_es_modules: PassFactory,
    pub inline_and_collapse_properties: PassFactory,
    pub collapse_object_literals: PassFactory,
    pub disambiguate_properties: PassFactory,
    pub devirtualize_methods: PassFactory,
    pub optimize_calls: PassFactory,
    pub optimize_constructors: PassFactory,
    pub mark_pure_functions: PassFactory,
    pub inline_variables: PassFactory,
    pub inline_constants: PassFactory,
    pub inline_simple_methods: PassFactory,
    pub dead_assignments_elimination: PassFactory,
    pub dead_property_assignment_elimination: PassFactory,
    pub inline_functions: PassFactory,
    pub inline_properties: PassFactory,
    pub isolate_polyfills: PassFactory,
    pub remove_unused_code: PassFactory,
    pub remove_unused_code_once: PassFactory,
    pub cross_chunk_code_motion: PassFactory,
    pub cross_chunk_method_motion: PassFactory,
    pub flow_sensitive_inline_variables: PassFactory,
    pub coalesce_variable_names: PassFactory,
    pub post_normalize_peephole: PassFactory,
    pub collapse_variable_declarations: PassFactory,
    pub extract_prototype_member_declarations: PassFactory,
    pub rewrite_function_expressions: PassFactory,
    pub collapse_anonymous_functions: PassFactory,
    pub rewrite_global_declarations_for_try_catch_wrapping: PassFactory,
    pub alias_strings: PassFactory,
    pub ambiguate_properties: PassFactory,
    pub mark_unnormalized: PassFactory,
    pub normalize: PassFactory,
    pub extern_exports: PassFactory,
    pub denormalize: PassFactory,
    pub invert_contextual_renaming: PassFactory,
    pub rename_properties: PassFactory,
    pub remove_property_renaming_calls: PassFactory,
    pub rename_vars: PassFactory,
    pub rename_labels: PassFactory,
    pub convert_to_dotted_properties: PassFactory,
    pub check_ast_validity: PassFactory,
    pub var_check_validity: PassFactory,
    pub instrument_for_code_coverage: PassFactory,
    pub gather_extern_properties_check: PassFactory,
    pub gather_extern_properties_optimize: PassFactory,
    pub polymer_pass: PassFactory,
    pub chrome_pass: PassFactory,
    pub j2cl_constant_hoister_pass: PassFactory,
    pub j2cl_clinit_pass: PassFactory,
    pub j2cl_property_inliner_pass: PassFactory,
    pub j2cl_pass: PassFactory,
    pub j2cl_util_get_define_rewriter_pass: PassFactory,
    pub j2cl_assert_removal_pass: PassFactory,
    pub j2cl_source_file_checker: PassFactory,
    pub j2cl_checks_pass: PassFactory,
    pub check_conformance: PassFactory,
    pub remove_cast_nodes: PassFactory,
    pub types_to_colors: PassFactory,
    pub serialize_typed_ast: PassFactory,
    pub remove_unnecessary_synthetic_externs: PassFactory,
    pub optimize_to_es6: PassFactory,
    pub whitespace_wrap_goog_modules: PassFactory,
    pub rewrite_common_js_modules: PassFactory,
    pub rewrite_scripts_to_es6_modules: PassFactory,
    pub gather_module_metadata_pass: PassFactory,
    pub create_module_map_pass: PassFactory,
    pub gather_getters_and_setters: PassFactory,
    pub add_synthetic_script: PassFactory,
    pub remove_synthetic_script: PassFactory,
    pub merge_synthetic_script: PassFactory,
    pub forbid_dynamic_import_usage: PassFactory,
    pub rewrite_dynamic_imports: PassFactory,
    pub protect_locale_data: PassFactory,
    pub substitute_locale_data: PassFactory,
    pub transpile_only_closure_unaware: PassFactory,
    pub transpile_and_optimize_closure_unaware: PassFactory,
}
impl DefaultPassConfig {
    /// The internal factory of a J2CL pass whose class is not ported (`J2clGatedUnportedPass`;
    /// J2CL passes are out of scope per docs/PORTING.md §2).
    fn j2cl_gated_unported_factory(class: &'static str) -> crate::pass_factory::InternalFactory {
        Arc::new(move |_| Box::new(J2clGatedUnportedPass::new(class, None)))
    }
    // port: DefaultPassConfig#DefaultPassConfig
    pub fn new(options: CompilerOptions) -> Self {
        let preprocessor_symbol_table_factory = CachedInstanceFactory::default();
        let check_side_effects = PassFactory::builder()
            .set_name("checkSideEffects")
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                Box::new(crate::check_side_effects::CheckSideEffects::new(
                    options.get_check_suspicious_code(),
                    options.should_protect_hidden_side_effects(),
                ))
            }))
            .build();
        let strip_side_effect_protection = PassFactory::builder()
            .set_name(pass_names::STRIP_SIDE_EFFECT_PROTECTION)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::check_side_effects::StripProtection::new())
            }))
            .build();
        let suspicious_code = PassFactory::builder()
            .set_name("suspiciousCode")
            .set_internal_factory(Arc::new(|compiler| {
                let mut shared_callbacks: Vec<Box<dyn crate::node_traversal::Callback>> =
                    Vec::new();
                let options = compiler.get_options();
                if options.get_check_suspicious_code() {
                    shared_callbacks.push(Box::new(
                        crate::check_suspicious_code::CheckSuspiciousCode::new(),
                    ));
                    shared_callbacks.push(Box::new(
                        crate::lint::check_duplicate_case::CheckDuplicateCase::new(),
                    ));
                }

                if options.enables(&crate::diagnostic_groups::GLOBAL_THIS) {
                    shared_callbacks
                        .push(Box::new(crate::check_global_this::CheckGlobalThis::new()));
                }

                if options.enables(&crate::diagnostic_groups::DEBUGGER_STATEMENT_PRESENT) {
                    shared_callbacks.push(Box::new(
                        crate::check_debugger_statement::CheckDebuggerStatement::new(),
                    ));
                }

                Self::combine_checks(compiler, shared_callbacks)
            }))
            .build();
        let extra_requires = PassFactory::builder()
            .set_name("checkExtraRequires")
            // Java reads `options` (the compiler's options) when the factory runs.
            .set_internal_factory(Arc::new(|compiler| {
                let requires_to_remove = compiler
                    .get_options()
                    .get_unused_imports_to_remove()
                    .clone();
                Box::new(crate::lint::check_extra_requires::CheckExtraRequires::new(
                    compiler,
                    requires_to_remove,
                ))
            }))
            .build();
        let check_missing_requires = PassFactory::builder()
            .set_name("checkMissingRequires")
            .set_internal_factory(Arc::new(|compiler| {
                // Java passes the nullable map; AbstractModuleCallback dereferences it.
                let module_metadata_map =
                    closure_rhino::check_not_null!(compiler.get_module_metadata_map().cloned());
                Box::new(crate::check_missing_requires::CheckMissingRequires::new(
                    compiler,
                    module_metadata_map,
                ))
            }))
            .build();
        let check_js_doc_and_es6_modules = PassFactory::builder()
            .set_name("checkJsDocAndEs6Modules")
            .set_internal_factory(Arc::new(|compiler| {
                Self::combine_checks(
                    compiler,
                    vec![
                        Box::new(crate::check_jsdoc::CheckJSDoc::new()),
                        Box::new(crate::es6_check_module::Es6CheckModule::new()),
                    ],
                )
            }))
            .build();
        let generate_exports = PassFactory::builder()
            .set_name(pass_names::GENERATE_EXPORTS)
            .set_internal_factory(Arc::new(|compiler| {
                let convention = compiler.get_coding_convention();
                let export_symbol_function = convention.get_export_symbol_function();
                let export_property_function = convention.get_export_property_function();
                let pass = crate::generate_exports::GenerateExports::new(
                    compiler,
                    compiler
                        .get_options()
                        .should_export_local_property_definitions(),
                    export_symbol_function,
                    export_property_function,
                );
                Box::new(GenerateExportsPass { pass })
            }))
            .build();
        let generate_ijs = PassFactory::builder()
            .set_name("generateIjs")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::ijs::convert_to_typed_interface::ConvertToTypedInterface::new())
            }))
            .build();
        let remove_extra_requires = PassFactory::builder()
            .set_name("removeExtraRequires")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::extra_require_remover::ExtraRequireRemover::new())
            }))
            .build();
        let export_test_functions = PassFactory::builder()
            .set_name(pass_names::EXPORT_TEST_FUNCTIONS)
            .set_internal_factory(Arc::new(|compiler| -> Box<dyn CompilerPass> {
                let convention = compiler.get_coding_convention();
                if let Some(export_symbol_function) = convention.get_export_symbol_function() {
                    let export_property_function = convention.get_export_property_function();
                    Box::new(crate::export_test_functions::ExportTestFunctions::new(
                        export_symbol_function,
                        export_property_function,
                    ))
                } else {
                    Box::new(crate::error_pass::ErrorPass::new(
                        compiler,
                        &GENERATE_EXPORTS_ERROR,
                    ))
                }
            }))
            .build();
        let gather_raw_exports = PassFactory::builder()
            .set_name(pass_names::GATHER_RAW_EXPORTS)
            .set_internal_factory(Arc::new(|_compiler| {
                let mut pass = crate::gather_raw_exports::GatherRawExports::new();
                Box::new(
                    move |compiler: &mut crate::AbstractCompiler, externs: NodeId, root: NodeId| {
                        pass.process(compiler, externs, root);
                        compiler.add_exported_names(
                            pass.get_exported_variable_names()
                                .iter()
                                .map(JsString::to_string_lossy),
                        );
                    },
                )
            }))
            .build();
        let closure_primitives = PassFactory::builder()
            .set_name("closurePrimitives")
            .set_internal_factory({
                let preprocessor_symbol_table_factory = preprocessor_symbol_table_factory.clone();
                Arc::new(move |compiler| {
                    preprocessor_symbol_table_factory.maybe_initialize(compiler);
                    Box::new(
                        crate::process_closure_primitives::ProcessClosurePrimitives::new(compiler),
                    )
                })
            })
            .build();
        let closure_provides_requires = PassFactory::builder()
            .set_name("closureProvidesRequires")
            .set_internal_factory({
                let preprocessor_symbol_table_factory = preprocessor_symbol_table_factory.clone();
                // DefaultPassConfig owns its options, so the Java read of
                // options.shouldPreservesGoogProvidesAndRequires() is taken here.
                let preserve_goog_provides_and_requires =
                    options.should_preserves_goog_provides_and_requires();
                Arc::new(move |compiler| {
                    preprocessor_symbol_table_factory.maybe_initialize(compiler);
                    let mut pass =
                        crate::process_closure_provides_and_requires::ProcessClosureProvidesAndRequires::new(
                            compiler,
                            preserve_goog_provides_and_requires,
                        );
                    Box::new(
                        move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                            pass.process(compiler, externs, root);
                            compiler.add_exported_names(
                                pass.get_exported_variable_names()
                                    .iter()
                                    .map(|name| name.to_string()),
                            );
                        },
                    )
                })
            })
            .build();
        let angular_pass = PassFactory::builder()
            .set_name(pass_names::ANGULAR_PASS)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::angular_pass::AngularPass::new(compiler))
            }))
            .build();
        let replace_messages_for_chrome = PassFactory::builder()
            .set_name(pass_names::REPLACE_MESSAGES)
            // unported: ReplaceMessagesForChrome (Chrome-specific, out of scope per
            // docs/PORTING.md §2; GoogleJsMessageIdGenerator is ported)
            .set_unported_internal_factory("ReplaceMessagesForChrome")
            .build();
        let closure_goog_scope_aliases_for_ijs = PassFactory::builder()
            .set_name("closureGoogScopeAliasesForIjs")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::scoped_aliases::ScopedAliases::builder().build(compiler))
            }))
            .build();
        // Applies aliases and inlines goog.scope, storing information about the transformations
        // performed.
        let closure_goog_scope_aliases = PassFactory::builder()
            .set_name("closureGoogScopeAliases")
            .set_internal_factory({
                let preprocessor_symbol_table_factory = preprocessor_symbol_table_factory.clone();
                Arc::new(move |compiler| {
                    preprocessor_symbol_table_factory.maybe_initialize(compiler);
                    Box::new(
                        crate::scoped_aliases::ScopedAliases::builder()
                            .set_preprocessor_symbol_table(
                                preprocessor_symbol_table_factory.get_instance_or_null(),
                            )
                            .set_module_metadata_map(compiler.get_module_metadata_map().cloned())
                            .set_invalid_module_get_handling(
                                crate::scoped_aliases::InvalidModuleGetHandling::GIVE_UNIQUE_NAME,
                            )
                            .build(compiler),
                    )
                })
            })
            .build();
        let inject_runtime_libraries = PassFactory::builder()
            .set_name("InjectRuntimeLibraries")
            .set_internal_factory(Arc::new(|compiler| {
                let force_injected_libraries = compiler
                    .get_options()
                    .get_force_library_injection_list()
                    .iter()
                    .cloned()
                    .collect();
                Box::new(
                    crate::inject_runtime_libraries::InjectRuntimeLibraries::new(
                        compiler,
                        force_injected_libraries,
                    ),
                )
            }))
            .build();
        let remove_weak_sources = PassFactory::builder()
            .set_name("removeWeakSources")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::remove_weak_sources::RemoveWeakSources::new())
            }))
            .build();
        let declared_global_externs_on_window = PassFactory::builder()
            .set_name(pass_names::DECLARED_GLOBAL_EXTERNS_ON_WINDOW)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::declared_global_externs_on_window::DeclaredGlobalExternsOnWindow::new(
                        compiler,
                    ),
                )
            }))
            .build();
        let check_type_import_code_references = PassFactory::builder()
            .set_name("checkTypeImportCodeReferences")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    crate::check_type_import_code_references::CheckTypeImportCodeReferences::new(),
                )
            }))
            .build();
        let closure_check_module = PassFactory::builder()
            .set_name("closureCheckModule")
            .set_internal_factory(Arc::new(|compiler| {
                // Java passes the nullable map; AbstractModuleCallback dereferences it.
                let module_metadata_map =
                    closure_rhino::check_not_null!(compiler.get_module_metadata_map().cloned());
                Box::new(crate::closure_check_module::ClosureCheckModule::new(
                    compiler,
                    module_metadata_map,
                ))
            }))
            .build();
        let closure_rewrite_module = PassFactory::builder()
            .set_name("closureRewriteModule")
            .set_internal_factory({
                let preprocessor_symbol_table_factory = preprocessor_symbol_table_factory.clone();
                Arc::new(move |compiler| {
                    preprocessor_symbol_table_factory.maybe_initialize(compiler);
                    let top_scope = compiler.get_top_scope();
                    Box::new(crate::closure_rewrite_module::ClosureRewriteModule::new(
                        compiler,
                        preprocessor_symbol_table_factory.get_instance_or_null(),
                        top_scope,
                    ))
                })
            })
            .build();
        let check_closure_imports = PassFactory::builder()
            .set_name("checkGoogRequires")
            .set_internal_factory(Arc::new(|compiler| {
                // Java passes the nullable map; AbstractModuleCallback dereferences it.
                let module_metadata_map =
                    closure_rhino::check_not_null!(compiler.get_module_metadata_map().cloned());
                Box::new(crate::check_closure_imports::CheckClosureImports::new(
                    compiler,
                    module_metadata_map,
                ))
            }))
            .build();
        let rewrite_goog_js_imports = PassFactory::builder()
            .set_name("rewriteGoogJsImports")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::rewrite_goog_js_imports::RewriteGoogJsImports::new(
                    crate::rewrite_goog_js_imports::Mode::LINT_AND_REWRITE,
                    compiler.get_module_map().cloned(),
                ))
            }))
            .build();
        let closure_replace_get_css_name = PassFactory::builder()
            .set_name("closureReplaceGetCssName")
            .set_internal_factory(Arc::new(|_compiler| Box::new(ClosureReplaceGetCssNamePass)))
            .build();
        let create_synthetic_blocks = PassFactory::builder()
            .set_name("createSyntheticBlocks")
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                Box::new(crate::create_synthetic_blocks::CreateSyntheticBlocks::new(
                    options.get_synthetic_block_start_marker().unwrap(),
                    options
                        .get_synthetic_block_end_marker()
                        .map(closure_rhino::js_string::JsString::from),
                ))
            }))
            .build();
        let early_peephole_optimizations = PassFactory::builder()
            .set_name("earlyPeepholeOptimizations")
            .set_internal_factory(Arc::new(|compiler| {
                // port: DefaultPassConfig#earlyPeepholeOptimizations (internal factory)
                let use_types_for_optimization = compiler
                    .get_options()
                    .should_use_types_for_local_optimization();
                let mut peephole_optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
                    Vec::new();
                peephole_optimizations.push(Box::new(PeepholeRemoveDeadCode::new()));
                if compiler
                    .get_options()
                    .get_j2cl_pass()
                    .should_add_j2cl_passes()
                {
                    peephole_optimizations.push(Box::new(J2clEqualitySameRewriterPass::new(
                        use_types_for_optimization,
                    )));
                }
                Box::new(PeepholeOptimizationsPass::new(
                    "earlyPeepholeOptimizations",
                    peephole_optimizations,
                ))
            }))
            .build();
        let early_inline_variables = PassFactory::builder()
            .set_name("earlyInlineVariables")
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                let mode = if options.should_inline_variables() {
                    crate::inline_variables::Mode::ALL
                } else if options.should_inline_local_variables() {
                    crate::inline_variables::Mode::LOCALS_ONLY
                } else {
                    panic!("No variable inlining option set.");
                };
                Box::new(crate::inline_variables::InlineVariables::new(mode))
            }))
            .build();
        let peephole_optimizations = PassFactory::builder()
            .set_name(pass_names::PEEPHOLE_OPTIMIZATIONS)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                Self::create_peephole_optimizations_pass(
                    compiler,
                    pass_names::PEEPHOLE_OPTIMIZATIONS,
                    true,
                )
            }))
            .build();
        let peephole_optimizations_once_normalized = PassFactory::builder()
            .set_name(pass_names::PEEPHOLE_OPTIMIZATIONS)
            .set_internal_factory(Arc::new(|compiler| {
                Self::create_peephole_optimizations_pass(
                    compiler,
                    pass_names::PEEPHOLE_OPTIMIZATIONS,
                    /* expectAstIsNormalized= */ true,
                )
            }))
            .build();
        let peephole_optimizations_once_non_normalized = PassFactory::builder()
            .set_name(pass_names::PEEPHOLE_OPTIMIZATIONS)
            .set_internal_factory(Arc::new(|compiler| {
                Self::create_peephole_optimizations_pass(
                    compiler,
                    pass_names::PEEPHOLE_OPTIMIZATIONS,
                    /* expectAstIsNormalized= */ false,
                )
            }))
            .build();
        let late_peephole_optimizations = PassFactory::builder()
            .set_name("latePeepholeOptimizations")
            .set_internal_factory(Arc::new(|compiler| {
                // port: DefaultPassConfig#latePeepholeOptimizations (internal factory)
                let late = true;
                let use_types_for_optimization = compiler
                    .get_options()
                    .should_use_types_for_local_optimization();
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![
                    Box::new(StatementFusion::new()),
                    Box::new(PeepholeRemoveDeadCode::new()),
                    Box::new(PeepholeMinimizeConditions::new(late)),
                    Box::new(PeepholeSubstituteAlternateSyntax::new(late)),
                    Box::new(PeepholeReplaceKnownMethods::new(
                        late,
                        use_types_for_optimization,
                    )),
                    Box::new(PeepholeFoldConstants::new(late, use_types_for_optimization)),
                ];
                Box::new(PeepholeOptimizationsPass::new(
                    "latePeepholeOptimizations",
                    optimizations,
                ))
            }))
            .build();
        let check_vars = PassFactory::builder()
            .set_name(pass_names::CHECK_VARS)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::var_check::VarCheck::new(compiler))
            }))
            .build();
        let infer_consts = PassFactory::builder()
            .set_name(pass_names::INFER_CONSTS)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::infer_consts::InferConsts::new(compiler))
            }))
            .build();
        let check_reg_exp = PassFactory::builder()
            .set_name(pass_names::CHECK_REG_EXP)
            .set_internal_factory(Arc::new(|compiler| {
                let pass = crate::check_reg_exp::CheckRegExp::new(
                    compiler
                        .get_options()
                        .get_assume_properties_are_statically_analyzable(),
                    /* report_errors= */ true,
                );

                Box::new(CheckRegExpAndRecordGlobalReferences { pass })
            }))
            .build();
        let check_reg_exp_for_optimizations = PassFactory::builder()
            .set_name("checkRegExpForOptimizations")
            .set_internal_factory(Arc::new(|compiler| {
                let reg_exp_check = crate::check_reg_exp::CheckRegExp::new(
                    compiler
                        .get_options()
                        .get_assume_properties_are_statically_analyzable(),
                    /* report_errors= */ false,
                );

                Box::new(CheckRegExpAndRecordGlobalReferences {
                    pass: reg_exp_check,
                })
            }))
            .build();
        let check_variable_references = PassFactory::builder()
            .set_name(pass_names::CHECK_VARIABLE_REFERENCES)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::variable_reference_check::VariableReferenceCheck::new(compiler))
            }))
            .build();
        let check_super = PassFactory::builder()
            .set_name("checkSuper")
            .set_internal_factory(Arc::new(
                |_| Box::new(crate::check_super::CheckSuper::new()),
            ))
            .build();
        // port: DefaultPassConfig#clearTypedScopeCreatorPass
        let clear_typed_scope_creator_pass = PassFactory::builder()
            .set_name("clearTypedScopeCreatorPass")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler, _, _| {
                        compiler.clear_typed_scope_creator()
                    },
                )
            }))
            .build();
        // Clears the top typed scope when we're done with it.
        // port: DefaultPassConfig#clearTopTypedScopePass
        let clear_top_typed_scope_pass = PassFactory::builder()
            .set_name("clearTopTypedScopePass")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler, _, _| {
                        // clear these scopes which we don't need anymore so they can be garbage
                        // collected
                        compiler.set_top_scope(None);
                        for compiler_input in compiler.get_inputs_in_order() {
                            compiler_input.set_typed_scope(None);
                        }
                    },
                )
            }))
            .build();
        let infer_types = PassFactory::builder()
            .set_name(pass_names::INFER_TYPES)
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler,
                     _unused: NodeId,
                     src_root: NodeId| {
                        let global_root = src_root.get_parent(compiler).unwrap();

                        let reverse_interpreter = compiler.get_reverse_abstract_interpreter();
                        let scope_creator = compiler.take_typed_scope_creator();
                        let mut inference_pass = crate::type_inference_pass::TypeInferencePass::new(
                            compiler,
                            reverse_interpreter,
                            scope_creator,
                        );
                        compiler.set_type_checking_has_run(true);
                        let top_scope = inference_pass.infer_all_scopes(compiler, global_root);
                        compiler.restore_typed_scope_creator(inference_pass.into_scope_creator());
                        compiler.set_top_scope(Some(top_scope));
                    },
                )
            }))
            .build();
        let infer_js_doc_info = PassFactory::builder()
            .set_name("inferJsDocInfo")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::infer_js_doc_info::InferJSDocInfo::new(compiler))
            }))
            .build();
        // port: DefaultPassConfig#checkTypes
        let check_types = PassFactory::builder()
            .set_name(pass_names::CHECK_TYPES)
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler,
                     externs: NodeId,
                     root: NodeId| {
                        use crate::error_manager::ErrorManager;
                        use crate::type_check::{
                            INEXISTENT_PROPERTY, TypeCheck, UNKNOWN_EXPR_TYPE,
                        };
                        let reverse_interpreter = compiler.get_reverse_abstract_interpreter();
                        let top_scope = compiler.get_top_scope();
                        // Java shares the compiler's TypedScopeCreator; TypeCheck owns it while
                        // it runs and hands it back afterwards.
                        let scope_creator = compiler.take_typed_scope_creator();
                        let report_unknown_types = compiler.get_options().enables(
                            &crate::diagnostic_group::DiagnosticGroup::for_type(&UNKNOWN_EXPR_TYPE),
                        );
                        let report_missing_properties = !compiler.get_options().disables(
                            &crate::diagnostic_group::DiagnosticGroup::for_type(
                                &INEXISTENT_PROPERTY,
                            ),
                        );
                        let mut check = TypeCheck::new_with_scope(
                            compiler,
                            reverse_interpreter,
                            top_scope,
                            Some(scope_creator),
                        )
                        .report_unknown_types(report_unknown_types)
                        .report_missing_properties(report_missing_properties);
                        check.process(compiler, externs, root);
                        let typed_percent = check.get_typed_percent();
                        compiler.restore_typed_scope_creator(check.into_scope_creator().unwrap());
                        compiler
                            .get_error_manager()
                            .set_typed_percent(typed_percent);
                    },
                )
            }))
            .build();
        let check_control_flow = PassFactory::builder()
            .set_name("checkControlFlow")
            .set_internal_factory(Arc::new(|compiler| {
                let mut callbacks: Vec<Box<dyn crate::node_traversal::Callback>> = Vec::new();
                let options = compiler.get_options();
                if !options.disables(&crate::diagnostic_groups::CHECK_USELESS_CODE) {
                    callbacks.push(Box::new(
                        crate::check_unreachable_code::CheckUnreachableCode::new(),
                    ));
                }
                if !options.disables(&crate::diagnostic_groups::MISSING_RETURN) {
                    callbacks.push(Box::new(
                        crate::check_missing_return::CheckMissingReturn::new(),
                    ));
                }
                Self::combine_checks(compiler, callbacks)
            }))
            .build();
        let check_access_controls = PassFactory::builder()
            .set_name("checkAccessControls")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::check_access_controls::CheckAccessControls::new(
                    compiler,
                ))
            }))
            .build();
        let lint_checks = PassFactory::builder()
            .set_name(pass_names::LINT_CHECKS)
            .set_internal_factory(Arc::new(|compiler| {
                use crate::lint::*;
                let callbacks: Vec<Box<dyn Callback>> = vec![
                    Box::new(
                        check_const_private_properties::CheckConstPrivateProperties::new(compiler),
                    ),
                    Box::new(check_constant_case_names::CheckConstantCaseNames::new(
                        compiler,
                    )),
                    Box::new(check_empty_statements::CheckEmptyStatements::new(compiler)),
                    Box::new(check_enums::CheckEnums::new(compiler)),
                    Box::new(
                        check_es6_module_file_structure::CheckEs6ModuleFileStructure::new(compiler),
                    ),
                    Box::new(check_es6_modules::CheckEs6Modules::new(compiler)),
                    Box::new(check_no_mutated_es6_exports::CheckNoMutatedEs6Exports::new(
                        compiler,
                    )),
                    Box::new(
                        check_goog_module_type_script_name::CheckGoogModuleTypeScriptName::new(
                            compiler,
                        ),
                    ),
                    Box::new(check_interfaces::CheckInterfaces::new(compiler)),
                    Box::new(check_jsdoc_style::CheckJSDocStyle::new(compiler)),
                    Box::new(check_missing_semicolon::CheckMissingSemicolon::new(
                        compiler,
                    )),
                    Box::new(check_nullability_modifiers::CheckNullabilityModifiers::new(
                        compiler,
                    )),
                    Box::new(check_primitive_as_object::CheckPrimitiveAsObject::new(
                        compiler,
                    )),
                    Box::new(check_prototype_properties::CheckPrototypeProperties::new(
                        compiler,
                    )),
                    Box::new(
                        check_unused_private_properties::CheckUnusedPrivateProperties::new(
                            compiler,
                        ),
                    ),
                    Box::new(check_unused_labels::CheckUnusedLabels::new(compiler)),
                    Box::new(check_useless_blocks::CheckUselessBlocks::new(compiler)),
                    Box::new(check_var::CheckVar::new(compiler)),
                ];
                Self::combine_checks(compiler, callbacks)
            }))
            .build();
        let analyzer_checks = PassFactory::builder()
            .set_name(pass_names::ANALYZER_CHECKS)
            .set_internal_factory(Arc::new(|compiler| {
                let callbacks: Vec<Box<dyn crate::node_traversal::Callback>> = vec![
                    Box::new(
                        crate::lint::check_array_with_goog_object::CheckArrayWithGoogObject::new(
                            compiler,
                        ),
                    ),
                    Box::new(
                        crate::implicit_nullability_check::ImplicitNullabilityCheck::new(compiler),
                    ),
                    Box::new(crate::lint::check_nested_names::CheckNestedNames::new(
                        compiler,
                    )),
                ];

                Self::combine_checks(compiler, callbacks)
            }))
            .build();
        let check_requires_and_provides_sorted = PassFactory::builder()
            .set_name("checkRequiresAndProvidesSorted")
            .set_internal_factory(Arc::new(|compiler| {
                use crate::lint::{check_provides_sorted, check_requires_sorted};
                Self::combine_checks(
                    compiler,
                    vec![
                        Box::new(check_provides_sorted::CheckProvidesSorted::new(
                            check_provides_sorted::Mode::COLLECT_AND_REPORT,
                        )),
                        Box::new(check_requires_sorted::CheckRequiresSorted::new(
                            check_requires_sorted::Mode::COLLECT_AND_REPORT,
                        )),
                    ],
                )
            }))
            .build();
        let check_strict_mode = PassFactory::builder()
            .set_name("checkStrictMode")
            .set_internal_factory(Arc::new(|compiler| {
                // Java reads DefaultPassConfig's options, which are the compiler's options.
                let default_level = if compiler.get_options().expect_strict_mode_input() {
                    crate::check_level::CheckLevel::ERROR
                } else {
                    crate::check_level::CheckLevel::OFF
                };
                Box::new(crate::strict_mode_check::StrictModeCheck::new(
                    compiler,
                    default_level,
                ))
            }))
            .build();
        let process_tweaks = PassFactory::builder()
            .set_name("processTweaks")
            .set_internal_factory(Arc::new(|compiler| {
                let strip_tweaks = compiler.get_options().get_tweak_processing().should_strip();
                Box::new(crate::process_tweaks::ProcessTweaks::new(strip_tweaks))
            }))
            .build();
        let process_defines_check =
            Self::create_process_defines(crate::process_defines::Mode::CHECK);
        let process_defines_optimize =
            Self::create_process_defines(crate::process_defines::Mode::OPTIMIZE);
        let strip_code = PassFactory::builder()
            .set_name("stripCode")
            .set_internal_factory(Arc::new(|_compiler| Box::new(StripCodeFactoryPass)))
            // TODO(johnlenz): StripCode may be fooled by some newer features, like destructuring,
            // an).build();
            .build();
        let check_consts = PassFactory::builder()
            .set_name("checkConsts")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::const_check::ConstCheck::new(compiler))
            }))
            .build();
        let rewrite_caller_code_location = PassFactory::builder()
            .set_name("rewriteCallerCodeLocation")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::rewrite_caller_code_location::RewriteCallerCodeLocation::new(compiler),
                )
            }))
            .build();
        let replace_toggles = PassFactory::builder()
            .set_name("replaceToggles")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::replace_toggles::ReplaceToggles::new(compiler))
            }))
            .build();
        let replace_id_generators = PassFactory::builder()
            .set_name(pass_names::REPLACE_ID_GENERATORS)
            .set_internal_factory(Arc::new(|_compiler| Box::new(ReplaceIdGeneratorsPass)))
            .build();
        let replace_strings = PassFactory::builder()
            .set_name("replaceStrings")
            .set_internal_factory(Arc::new(|_compiler| Box::new(ReplaceStringsFactoryPass)))
            .build();
        let closure_code_removal = PassFactory::builder()
            .set_name("closureCodeRemoval")
            .set_internal_factory(Arc::new(|compiler| {
                let remove_abstract_methods =
                    compiler.get_options().should_remove_abstract_methods();
                let remove_closure_asserts = compiler.get_options().should_remove_closure_asserts();
                Box::new(crate::closure_code_removal::ClosureCodeRemoval::new(
                    compiler,
                    remove_abstract_methods,
                    remove_closure_asserts,
                ))
            }))
            .build();
        let closure_optimize_primitives = PassFactory::builder()
            .set_name("closureOptimizePrimitives")
            .set_internal_factory(Arc::new(|compiler| {
                let can_use_es6_syntax = compiler
                    .get_options()
                    .get_output_feature_set()
                    .contains(closure_parsing::parser::feature_set::FeatureSet::ES2015);
                Box::new(
                    crate::closure_optimize_primitives::ClosureOptimizePrimitives::new(
                        compiler,
                        can_use_es6_syntax,
                    ),
                )
            }))
            .build();
        let rescope_global_symbols = PassFactory::builder()
            .set_name("rescopeGlobalSymbols")
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                Box::new(crate::rescope_global_symbols::RescopeGlobalSymbols::new(
                    options
                        .get_rename_prefix_namespace()
                        .expect("NullPointerException"),
                    options.assume_cross_chunk_names_for_rename_prefix_namespace(),
                    options.get_optimize_local_access_for_global_symbol_namespace(),
                ))
            }))
            .build();
        let convert_chunks_to_es_modules = PassFactory::builder()
            .set_name("convertChunksToESModules")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::convert_chunks_to_es_modules::ConvertChunksToESModules::new())
            }))
            .build();
        let inline_and_collapse_properties = PassFactory::builder()
            .set_name("inlineAndCollapseProperties")
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_property_collapse_level() == PropertyCollapseLevel::NONE
                    || options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                let property_collapse_level = options.get_property_collapse_level();
                let chunk_output_type = options.get_chunk_output_type();
                let have_modules_been_rewritten = options.get_process_common_js_modules();
                let module_resolution_mode = options.get_module_resolution_mode();
                Box::new(
                    crate::inline_and_collapse_properties::InlineAndCollapseProperties::builder(
                        compiler,
                    )
                    .set_property_collapse_level(property_collapse_level)
                    .set_chunk_output_type(chunk_output_type)
                    .set_have_modules_been_rewritten(have_modules_been_rewritten)
                    .set_module_resolution_mode(module_resolution_mode)
                    .build(),
                )
            }))
            .build();
        let collapse_object_literals = PassFactory::builder()
            .set_name(pass_names::COLLAPSE_OBJECT_LITERALS)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::inline_object_literals::InlineObjectLiterals::new(
                    compiler.get_unique_name_id_supplier(),
                ))
            }))
            .build();
        let disambiguate_properties = PassFactory::builder()
            .set_name(pass_names::DISAMBIGUATE_PROPERTIES)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                let properties_that_must_disambiguate = compiler
                    .get_options()
                    .get_properties_that_must_disambiguate()
                    .iter()
                    .map(|name| closure_rhino::js_string::JsString::from(name.as_str()))
                    .collect();
                Box::new(
                    crate::disambiguate::disambiguate_properties::DisambiguateProperties::new(
                        compiler,
                        properties_that_must_disambiguate,
                    ),
                )
            }))
            .build();
        let devirtualize_methods = PassFactory::builder()
            .set_name(pass_names::DEVIRTUALIZE_METHODS)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::optimize_calls::OptimizeCalls::builder()
                        .set_compiler(compiler)
                        .set_consider_externs(false)
                        .add_pass(Box::new(
                            crate::devirtualize_methods::DevirtualizeMethods::new(),
                        ))
                        .build(),
                )
            }))
            .build();
        let optimize_calls = PassFactory::builder()
            .set_name(pass_names::OPTIMIZE_CALLS)
            .set_run_in_fixed_point_loop(true)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::optimize_calls::OptimizeCalls::builder()
                        .set_compiler(compiler)
                        .set_consider_externs(false)
                        // Remove unused return values.
                        .add_pass(Box::new(crate::optimize_returns::OptimizeReturns::new()))
                        // Remove all parameters that are constants or unused.
                        .add_pass(Box::new(
                            crate::optimize_parameters::OptimizeParameters::new(compiler),
                        ))
                        .build(),
                )
            }))
            .build();
        let optimize_constructors = PassFactory::builder()
            .set_name("optimizeConstructors")
            .set_run_in_fixed_point_loop(false)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::optimize_calls::OptimizeCalls::builder()
                        .set_compiler(compiler)
                        .set_consider_externs(false)
                        // Remove redundant constructor definitions.
                        .add_pass(Box::new(
                            crate::optimize_constructors::OptimizeConstructors::new(compiler),
                        ))
                        .build(),
                )
            }))
            .build();
        let mark_pure_functions = PassFactory::builder()
            .set_name("markPureFunctions")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::pure_function_identifier::Driver::new())
            }))
            .set_precondition_check(Arc::new(Self::require_properties_are_statically_analyzable))
            .build();
        let inline_variables = PassFactory::builder()
            .set_name(pass_names::INLINE_VARIABLES)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                let mode = if options.should_inline_variables() {
                    crate::inline_variables::Mode::ALL
                } else if options.should_inline_local_variables() {
                    crate::inline_variables::Mode::LOCALS_ONLY
                } else {
                    panic!("No variable inlining option set.");
                };
                Box::new(crate::inline_variables::InlineVariables::new(mode))
            }))
            .build();
        let inline_constants = PassFactory::builder()
            .set_name("inlineConstants")
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::inline_variables::InlineVariables::new(
                    crate::inline_variables::Mode::CONSTANTS_ONLY,
                ))
            }))
            .build();
        let inline_simple_methods = PassFactory::builder()
            .set_name("inlineSimpleMethods")
            .set_run_in_fixed_point_loop(true)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::inline_simple_methods::InlineSimpleMethods::new(
                    compiler,
                ))
            }))
            .build();
        let dead_assignments_elimination = PassFactory::builder()
            .set_name(pass_names::DEAD_ASSIGNMENT_ELIMINATION)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::dead_assignments_elimination::DeadAssignmentsElimination::new())
            }))
            .build();
        let dead_property_assignment_elimination = PassFactory::builder()
            .set_name("deadPropertyAssignmentElimination")
            .set_run_in_fixed_point_loop(true)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    crate::dead_property_assignment_elimination::DeadPropertyAssignmentElimination::new(),
                )
            }))
            .build();
        let inline_functions = PassFactory::builder()
            .set_name(pass_names::INLINE_FUNCTIONS)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                let reach = options.get_inline_functions_level();
                let assume_strict_this =
                    options.assume_strict_this() || options.expect_strict_mode_input();
                let assume_minimum_capture = options.assume_closures_only_capture_references();
                let max_size_after_inlining = options.get_max_function_size_after_inlining();
                Box::new(crate::inline_functions::InlineFunctions::new(
                    compiler,
                    compiler.get_unique_name_id_supplier(),
                    reach,
                    assume_strict_this,
                    assume_minimum_capture,
                    max_size_after_inlining,
                ))
            }))
            .build();
        let inline_properties = PassFactory::builder()
            .set_name(pass_names::INLINE_PROPERTIES)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::inline_properties::InlineProperties::new(compiler))
            }))
            .build();
        let isolate_polyfills = PassFactory::builder()
            .set_name("IsolatePolyfills")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::isolate_polyfills::IsolatePolyfills::new(compiler))
            }))
            .build();
        let remove_unused_code = PassFactory::builder()
            .set_name(pass_names::REMOVE_UNUSED_CODE)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                let options = compiler.get_options();
                let remove_local_vars = options.should_remove_unused_local_variables();
                let remove_globals = options.should_remove_unused_variables();
                let remove_unused_prototype_properties =
                    options.should_remove_unused_prototype_properties();
                let remove_unused_class_properties = options.is_remove_unused_class_properties();
                // If we are forcing injection of some library code, don't remove polyfills.
                // Otherwise, we might end up removing polyfills the user specifically asked to
                // include.
                let remove_unused_polyfills = options.get_force_library_injection_list().is_empty()
                    && options.get_inject_polyfills_newer_than().is_none();
                let assume_getters_are_pure = options.get_assume_getters_are_pure();
                Box::new(
                    crate::remove_unused_code::RemoveUnusedCode::builder(compiler)
                        .remove_local_vars(remove_local_vars)
                        .remove_globals(remove_globals)
                        .preserve_function_expression_names(false)
                        .remove_unused_prototype_properties(remove_unused_prototype_properties)
                        .remove_unused_this_properties(remove_unused_class_properties)
                        .remove_unused_object_define_properties_definitions(
                            remove_unused_class_properties,
                        )
                        .remove_unused_polyfills(remove_unused_polyfills)
                        .assume_getters_are_pure(assume_getters_are_pure)
                        .build(),
                )
            }))
            .build();
        let remove_unused_code_once = remove_unused_code
            .to_builder()
            .set_run_in_fixed_point_loop(false)
            .build();
        let cross_chunk_code_motion = PassFactory::builder()
            .set_name(pass_names::CROSS_CHUNK_CODE_MOTION)
            .set_run_in_fixed_point_loop(true)
            // Java's `options` field is the compiler's options object, read when the factory runs.
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::cross_chunk_code_motion::CrossChunkCodeMotion::new(
                    compiler
                        .get_options()
                        .get_parent_chunk_can_see_symbols_declared_in_children(),
                ))
            }))
            .build();
        let cross_chunk_method_motion = PassFactory::builder()
            .set_name(pass_names::CROSS_CHUNK_METHOD_MOTION)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                let no_stub_methods = compiler
                    .get_options()
                    .get_cross_chunk_code_motion_no_stub_methods();
                Box::new(
                    crate::cross_chunk_method_motion::CrossChunkMethodMotion::new(
                        compiler,
                        crate::cross_chunk_method_motion::CrossChunkIdGenerator::Compiler,
                        /* can_modify_externs= */ false, // remove this
                        no_stub_methods,
                    ),
                )
            }))
            .build();
        let flow_sensitive_inline_variables = PassFactory::builder()
            .set_name(pass_names::FLOW_SENSITIVE_INLINE_VARIABLES)
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    crate::flow_sensitive_inline_variables::FlowSensitiveInlineVariables::new(),
                )
            }))
            .build();
        let coalesce_variable_names = PassFactory::builder()
            .set_name(pass_names::COALESCE_VARIABLE_NAMES)
            .set_internal_factory(Arc::new(|compiler| {
                // Java reads the pass config's `options`, which is the compiler's options object.
                let use_pseudo_names = compiler.get_options().should_generate_pseudo_names();
                Box::new(crate::coalesce_variable_names::CoalesceVariableNames::new(
                    compiler,
                    use_pseudo_names,
                ))
            }))
            .build();
        let post_normalize_peephole = PassFactory::builder()
            .set_name(pass_names::POST_NORMALIZE_PEEPHOLE)
            .set_internal_factory(Arc::new(|compiler| {
                // port: DefaultPassConfig#postNormalizePeephole (internal factory)
                let mut optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = Vec::new();
                if compiler
                    .get_options()
                    .should_collapse_variable_declarations()
                {
                    optimizations.push(Box::new(ExploitAssigns::new()));
                }
                if compiler.get_options().should_optimize_let_and_const() {
                    optimizations.push(Box::new(OptimizeLetAndConstPeephole::new(
                        compiler
                            .get_options()
                            .should_treat_global_scope_as_isolated(),
                    )));
                }
                Box::new(PeepholeOptimizationsPass::new(
                    pass_names::POST_NORMALIZE_PEEPHOLE,
                    optimizations,
                ))
            }))
            .build();
        let collapse_variable_declarations = PassFactory::builder()
            .set_name(pass_names::COLLAPSE_VARIABLE_DECLARATIONS)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::collapse_variable_declarations::CollapseVariableDeclarations::new(
                        compiler,
                    ),
                )
            }))
            .build();
        let extract_prototype_member_declarations = PassFactory::builder()
            .set_name(pass_names::EXTRACT_PROTOTYPE_MEMBER_DECLARATIONS)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                use crate::extract_prototype_member_declarations::{
                    ExtractPrototypeMemberDeclarations, Pattern,
                };
                let pattern = match compiler
                    .get_options()
                    .get_extract_prototype_member_declarations_mode()
                {
                    ExtractPrototypeMemberDeclarationsMode::USE_GLOBAL_TEMP => {
                        Pattern::USE_GLOBAL_TEMP
                    }
                    ExtractPrototypeMemberDeclarationsMode::USE_CHUNK_TEMP => {
                        Pattern::USE_CHUNK_TEMP
                    }
                    ExtractPrototypeMemberDeclarationsMode::USE_IIFE => Pattern::USE_IIFE,
                    _ => panic!("unexpected"),
                };

                Box::new(ExtractPrototypeMemberDeclarations::new(pattern))
            }))
            .build();
        let rewrite_function_expressions = PassFactory::builder()
            .set_name(pass_names::REWRITE_FUNCTION_EXPRESSIONS)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::function_rewriter::FunctionRewriter::new())
            }))
            .build();
        let collapse_anonymous_functions = PassFactory::builder()
            .set_name(pass_names::COLLAPSE_ANONYMOUS_FUNCTIONS)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::collapse_anonymous_functions::CollapseAnonymousFunctions::new(compiler),
                )
            }))
            .build();
        let rewrite_global_declarations_for_try_catch_wrapping = PassFactory::builder()
            .set_name("rewriteGlobalDeclarationsForTryCatchWrapping")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    crate::rewrite_global_declarations_for_try_catch_wrapping::RewriteGlobalDeclarationsForTryCatchWrapping::new(),
                )
            }))
            .build();
        let alias_strings = PassFactory::builder()
            .set_name("aliasStrings")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::alias_strings::AliasStrings::new(
                    compiler.get_options().should_output_js_string_usage(),
                    compiler.get_options().get_alias_strings_mode(),
                ))
            }))
            .build();
        let ambiguate_properties = PassFactory::builder()
            .set_name(pass_names::AMBIGUATE_PROPERTIES)
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                let extern_properties =
                    closure_rhino::check_not_null!(compiler.get_extern_properties())
                        .iter()
                        .map(|name| closure_rhino::js_string::JsString::from(name.as_str()))
                        .collect();
                Box::new(
                    crate::disambiguate::ambiguate_properties::AmbiguateProperties::new(
                        compiler,
                        compiler
                            .get_options()
                            .get_property_reserved_naming_first_chars(),
                        compiler
                            .get_options()
                            .get_property_reserved_naming_non_first_chars(),
                        &extern_properties,
                    ),
                )
            }))
            .build();
        let mark_unnormalized = PassFactory::builder()
            .set_name("markUnnormalized")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    // port: DefaultPassConfig#markUnnormalized (anonymous CompilerPass)
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler,
                     _externs: NodeId,
                     _root: NodeId| {
                        compiler
                            .set_life_cycle_stage(crate::abstract_compiler::LifeCycleStage::RAW);
                    },
                )
            }))
            .build();
        let normalize = PassFactory::builder()
            .set_name(pass_names::NORMALIZE)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::normalize::Normalize::create_normalize_for_optimizations(compiler))
            }))
            .build();
        let extern_exports = PassFactory::builder()
            .set_name(pass_names::EXTERN_EXPORTS)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::extern_exports_pass::ExternExportsPass::new(compiler))
            }))
            .build();
        let denormalize = PassFactory::builder()
            .set_name("denormalize")
            .set_internal_factory(Arc::new(|compiler| {
                let output_feature_set = compiler.get_options().get_output_feature_set();
                Box::new(crate::denormalize::Denormalize::new(
                    compiler,
                    output_feature_set,
                ))
            }))
            .build();
        let invert_contextual_renaming = PassFactory::builder()
            .set_name("invertContextualRenaming")
            .set_internal_factory(Arc::new(
                crate::make_declared_names_unique::MakeDeclaredNamesUnique::get_contextual_rename_inverter,
            ))
            .build();
        let rename_properties = PassFactory::builder()
            .set_name("renameProperties")
            .set_precondition_check(Arc::new(|options| PreconditionResult {
                success: options.get_assume_properties_are_statically_analyzable(),
                message: Some(
                    "requires assumePropertiesAreStaticallyAnalyzable to be enabled".into(),
                ),
            }))
            .set_internal_factory(Arc::new(|compiler| {
                closure_rhino::check_state!(
                    compiler.get_options().get_property_renaming()
                        == PropertyRenamingPolicy::ALL_UNQUOTED
                );
                let prev_property_map = compiler.get_options().get_input_property_map().clone();
                Box::new(
                    move |compiler: &mut crate::AbstractCompiler, externs: NodeId, root: NodeId| {
                        let options = compiler.get_options();
                        let mut rprop = crate::rename_properties::RenameProperties::new(
                            compiler,
                            options.should_generate_pseudo_names(),
                            prev_property_map.clone(),
                            options.get_property_reserved_naming_first_chars(),
                            options.get_property_reserved_naming_non_first_chars(),
                            Self::shared_name_generator(options),
                        );
                        rprop.process(compiler, externs, root);
                        compiler.set_property_map(Arc::new(rprop.get_property_map()));
                    },
                )
            }))
            .build();
        let remove_property_renaming_calls = PassFactory::builder()
            .set_name("removePropertyRenamingCalls")
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(crate::remove_property_renaming_calls::RemovePropertyRenamingCalls::new())
            }))
            .build();
        let rename_vars = PassFactory::builder()
            .set_name("renameVars")
            .set_internal_factory(Arc::new(|compiler| {
                let prev_variable_map = compiler.get_options().get_input_variable_map().clone();
                Box::new(
                    move |compiler: &mut crate::AbstractCompiler, externs: NodeId, root: NodeId| {
                        let variable_map = Self::run_variable_renaming(
                            compiler,
                            prev_variable_map.clone(),
                            externs,
                            root,
                        );
                        compiler.set_variable_map(Arc::new(variable_map));
                    },
                )
            }))
            .build();
        let rename_labels = PassFactory::builder()
            .set_name("renameLabels")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::rename_labels::RenameLabels::new())
            }))
            .build();
        let convert_to_dotted_properties = PassFactory::builder()
            .set_name(pass_names::CONVERT_TO_DOTTED_PROPERTIES)
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(crate::convert_to_dotted_properties::ConvertToDottedProperties::new())
            }))
            .build();
        let check_ast_validity = PassFactory::builder()
            .set_name("checkAstValidity")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::ast_validator::AstValidator::new(compiler))
            }))
            .build();
        let var_check_validity = PassFactory::builder()
            .set_name("varCheckValidity")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::var_check::VarCheck::new_with_validity_check(
                    compiler, true,
                ))
            }))
            .build();
        let instrument_for_code_coverage = PassFactory::builder()
            .set_name("instrumentForCodeCoverage")
            // unported: CoverageInstrumentationPass (instrumentation is out of scope per
            // docs/PORTING.md §2)
            .set_unported_internal_factory("CoverageInstrumentationPass")
            .build();
        let gather_extern_properties_check =
            Self::create_gather_extern_properties(crate::gather_extern_properties::Mode::CHECK);
        let gather_extern_properties_optimize =
            Self::create_gather_extern_properties(crate::gather_extern_properties::Mode::OPTIMIZE);
        let polymer_pass = PassFactory::builder()
            .set_name("polymerPass")
            // unported: PolymerPass (Polymer is out of scope per docs/PORTING.md §2)
            .set_unported_internal_factory("PolymerPass")
            .build();
        let chrome_pass = PassFactory::builder()
            .set_name("chromePass")
            // unported: ChromePass (Chrome-specific, out of scope per docs/PORTING.md §2)
            .set_unported_internal_factory("ChromePass")
            .build();
        let j2cl_constant_hoister_pass = PassFactory::builder()
            .set_name("j2clConstantHoisterPass")
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Self::j2cl_gated_unported_factory("J2clConstantHoisterPass"))
            .build();
        let j2cl_clinit_pass = PassFactory::builder()
            .set_name("j2clClinitPass")
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                let changed_scope_nodes = compiler
                    .get_change_tracker()
                    .get_changed_scope_nodes_for_pass("j2clClinitPass");
                Box::new(J2clGatedUnportedPass::new(
                    "J2clClinitPrunerPass",
                    changed_scope_nodes,
                ))
            }))
            .build();
        let j2cl_property_inliner_pass = PassFactory::builder()
            .set_name("j2clPropertyInlinerPass")
            .set_internal_factory(Self::j2cl_gated_unported_factory("J2clPropertyInlinerPass"))
            .build();
        let j2cl_pass = PassFactory::builder()
            .set_name("j2clPass")
            .set_internal_factory(Self::j2cl_gated_unported_factory("J2clPass"))
            .build();
        let j2cl_util_get_define_rewriter_pass = PassFactory::builder()
            .set_name("j2clUtilGetDefineRewriterPass")
            .set_internal_factory(Self::j2cl_gated_unported_factory(
                "J2clUtilGetDefineRewriterPass",
            ))
            .build();
        let j2cl_assert_removal_pass = PassFactory::builder()
            .set_name("j2clAssertRemovalPass")
            .set_internal_factory(Self::j2cl_gated_unported_factory("J2clAssertRemovalPass"))
            .build();
        let j2cl_source_file_checker = PassFactory::builder()
            .set_name("j2clSourceFileChecker")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::j2cl_source_file_checker::J2clSourceFileChecker::new())
            }))
            .build();
        let j2cl_checks_pass = PassFactory::builder()
            .set_name("j2clChecksPass")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::j2cl_checks_pass::J2clChecksPass::new())
            }))
            .build();
        let check_conformance = PassFactory::builder()
            .set_name(pass_names::CHECK_CONFORMANCE)
            .set_internal_factory(Arc::new(|compiler| {
                let configs = compiler.get_options().get_conformance_configs().clone();
                let reporting_mode = compiler.get_options().get_conformance_reporting_mode();
                Box::new(crate::check_conformance::CheckConformance::new(
                    compiler,
                    &configs,
                    Some(reporting_mode),
                ))
            }))
            .build();
        let remove_cast_nodes = PassFactory::builder()
            .set_name("removeCastNodes")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::remove_cast_nodes::RemoveCastNodes::new())
            }))
            .build();
        let types_to_colors = PassFactory::builder()
            .set_name("typesToColors")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler,
                     externs: NodeId,
                     js: NodeId| {
                        use crate::compiler_pass::CompilerPass;
                        use crate::serialization::convert_types_to_colors::ConvertTypesToColors;
                        use crate::serialization::serialization_options::SerializationOptions;
                        ConvertTypesToColors::new(
                            SerializationOptions::builder()
                                .set_include_debug_info(
                                    compiler.get_options().should_serialize_extra_debug_info(),
                                )
                                .build(),
                        )
                        .process(compiler, externs, js);
                        compiler.set_life_cycle_stage(
                            crate::abstract_compiler::LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC,
                        );
                    },
                )
            }))
            .build();
        let serialize_typed_ast = PassFactory::builder()
            .set_name("serializeTypedAst")
            .set_internal_factory({
                let typed_ast_output_file =
                    options.get_typed_ast_output_file().map(Path::to_path_buf);
                Arc::new(move |compiler| {
                    use crate::serialization::serialization_options::SerializationOptions;
                    use crate::serialization::serialize_typed_ast_pass::SerializeTypedAstPass;
                    let runtime_libraries = compiler
                        .get_runtime_js_lib_manager()
                        .lock()
                        .unwrap()
                        .get_injected_libraries();
                    Box::new(SerializeTypedAstPass::create_from_path(
                        typed_ast_output_file
                            .clone()
                            .expect("NullPointerException: typedAstOutputFile"),
                        SerializationOptions::builder()
                            .set_include_debug_info(
                                compiler.get_options().should_serialize_extra_debug_info(),
                            )
                            // set the runtime libraries to serialize in the TypedAST proto
                            .set_runtime_libraries(runtime_libraries)
                            .build(),
                    ))
                })
            })
            .build();
        let remove_unnecessary_synthetic_externs = PassFactory::builder()
            .set_name("removeUnnecessarySyntheticExterns")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::remove_unnecessary_synthetic_externs::RemoveUnnecessarySyntheticExterns::new(
                        compiler,
                    ),
                )
            }))
            .build();
        let optimize_to_es6 = PassFactory::builder()
            .set_name("optimizeToEs6")
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(crate::substitute_es6_syntax::SubstituteEs6Syntax::new())
            }))
            .build();
        let whitespace_wrap_goog_modules = PassFactory::builder()
            .set_name("whitespaceWrapGoogModules")
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::whitespace_wrap_goog_modules::WhitespaceWrapGoogModules::new())
            }))
            .build();
        let rewrite_common_js_modules = PassFactory::builder()
            .set_name(pass_names::REWRITE_COMMON_JS_MODULES)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::process_common_js_modules::ProcessCommonJSModules::new())
            }))
            .build();
        let rewrite_scripts_to_es6_modules = PassFactory::builder()
            .set_name(pass_names::REWRITE_SCRIPTS_TO_ES6_MODULES)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::es6_rewrite_scripts_to_modules::Es6RewriteScriptsToModules::new())
            }))
            .build();
        let gather_module_metadata_pass = PassFactory::builder()
            .set_name(pass_names::GATHER_MODULE_METADATA)
            .set_internal_factory(Arc::new(|compiler| {
                // Force creation of the synthetic input so that we create metadata for it
                compiler.get_synthesized_externs_input();
                // DefaultPassConfig's options are the compiler's options.
                Box::new(crate::gather_module_metadata::GatherModuleMetadata::new(
                    compiler.get_options().get_process_common_js_modules(),
                    compiler.get_options().get_module_resolution_mode(),
                ))
            }))
            .build();
        let create_module_map_pass = PassFactory::builder()
            .set_name(pass_names::CREATE_MODULE_MAP)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::modules::module_map_creator::ModuleMapCreator::new(
                    compiler.get_module_metadata_map().unwrap().clone(),
                ))
            }))
            // does not look at AST
            .build();
        let gather_getters_and_setters = PassFactory::builder()
            .set_name(pass_names::GATHER_GETTERS_AND_SETTERS)
            .set_internal_factory(Arc::new(|_| {
                Box::new(crate::gather_getter_and_setter_properties::GatherGetterAndSetterProperties::new())
            }))
            .build();
        let add_synthetic_script = PassFactory::builder()
            .set_name("ADD_SYNTHETIC_SCRIPT")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut AbstractCompiler, _externs: NodeId, _js: NodeId| {
                        compiler.initialize_synthetic_code_input()
                    },
                )
            }))
            .build();
        let remove_synthetic_script = PassFactory::builder()
            .set_name("REMOVE_SYNTHETIC_SCRIPT")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut AbstractCompiler, _externs: NodeId, _js: NodeId| {
                        compiler.remove_synthetic_code_input()
                    },
                )
            }))
            .build();
        let merge_synthetic_script = PassFactory::builder()
            .set_name("MERGE_SYNTHETIC_SCRIPT")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut AbstractCompiler, _externs: NodeId, _js: NodeId| {
                        compiler.merge_synthetic_code_input()
                    },
                )
            }))
            .build();
        let forbid_dynamic_import_usage = PassFactory::builder()
            .set_name("FORBID_DYNAMIC_IMPORT")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    crate::forbid_dynamic_import_usage::ForbidDynamicImportUsage::new(compiler),
                )
            }))
            .build();
        let rewrite_dynamic_imports = PassFactory::builder()
            .set_name("REWRITE_DYNAMIC_IMPORT")
            .set_internal_factory(Arc::new(|compiler| {
                let alias = compiler
                    .get_options()
                    .get_dynamic_import_alias()
                    .map(str::to_owned);
                let chunk_output_type = compiler.get_options().get_chunk_output_type();
                Box::new(crate::rewrite_dynamic_imports::RewriteDynamicImports::new(
                    compiler,
                    alias.as_deref(),
                    chunk_output_type,
                ))
            }))
            .build();
        let protect_locale_data = PassFactory::builder()
            .set_name("protectLocaleData")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler,
                     externs: NodeId,
                     root: NodeId| {
                        use crate::compiler_pass::CompilerPass;
                        crate::locale_data_passes::ProtectGoogLocale::new(compiler)
                            .process(compiler, externs, root);
                    },
                )
            }))
            .build();
        let substitute_locale_data = PassFactory::builder()
            .set_name("SubstituteLocaleData")
            .set_internal_factory(Arc::new(|_| {
                Box::new(
                    |compiler: &mut crate::abstract_compiler::AbstractCompiler,
                     externs: NodeId,
                     root: NodeId| {
                        use crate::compiler_pass::CompilerPass;
                        let locale = compiler.get_options().get_locale().map(str::to_string);
                        crate::locale_data_passes::LocaleSubstitutions::new(
                            compiler,
                            locale.as_deref(),
                        )
                        .process(compiler, externs, root);
                    },
                )
            }))
            .build();
        let transpile_only_closure_unaware = PassFactory::builder()
            .set_name("TranspileOnlyClosureUnaware")
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(
                    crate::transpile_and_optimize_closure_unaware::TranspileAndOptimizeClosureUnaware::new(
                        crate::nested_compiler_runner::Mode::TRANSPILE_ONLY,
                    ),
                )
            }))
            .build();
        let transpile_and_optimize_closure_unaware = PassFactory::builder()
            .set_name("TranspileAndOptimizeClosureUnaware")
            .set_internal_factory(Arc::new(|_compiler| {
                Box::new(
                    crate::transpile_and_optimize_closure_unaware::TranspileAndOptimizeClosureUnaware::new(
                        crate::nested_compiler_runner::Mode::TRANSPILE_AND_OPTIMIZE,
                    ),
                )
            }))
            .build();
        Self {
            options,
            preprocessor_symbol_table_factory,
            check_side_effects,
            strip_side_effect_protection,
            suspicious_code,
            extra_requires,
            check_missing_requires,
            check_js_doc_and_es6_modules,
            generate_exports,
            generate_ijs,
            remove_extra_requires,
            export_test_functions,
            gather_raw_exports,
            closure_primitives,
            closure_provides_requires,
            angular_pass,
            replace_messages_for_chrome,
            closure_goog_scope_aliases_for_ijs,
            closure_goog_scope_aliases,
            inject_runtime_libraries,
            remove_weak_sources,
            declared_global_externs_on_window,
            check_type_import_code_references,
            closure_check_module,
            closure_rewrite_module,
            check_closure_imports,
            rewrite_goog_js_imports,
            closure_replace_get_css_name,
            create_synthetic_blocks,
            early_peephole_optimizations,
            early_inline_variables,
            peephole_optimizations,
            peephole_optimizations_once_normalized,
            peephole_optimizations_once_non_normalized,
            late_peephole_optimizations,
            check_vars,
            infer_consts,
            check_reg_exp,
            check_reg_exp_for_optimizations,
            check_variable_references,
            check_super,
            clear_typed_scope_creator_pass,
            clear_top_typed_scope_pass,
            infer_types,
            infer_js_doc_info,
            check_types,
            check_control_flow,
            check_access_controls,
            lint_checks,
            analyzer_checks,
            check_requires_and_provides_sorted,
            check_strict_mode,
            process_tweaks,
            process_defines_check,
            process_defines_optimize,
            strip_code,
            check_consts,
            rewrite_caller_code_location,
            replace_toggles,
            replace_id_generators,
            replace_strings,
            closure_code_removal,
            closure_optimize_primitives,
            rescope_global_symbols,
            convert_chunks_to_es_modules,
            inline_and_collapse_properties,
            collapse_object_literals,
            disambiguate_properties,
            devirtualize_methods,
            optimize_calls,
            optimize_constructors,
            mark_pure_functions,
            inline_variables,
            inline_constants,
            inline_simple_methods,
            dead_assignments_elimination,
            dead_property_assignment_elimination,
            inline_functions,
            inline_properties,
            isolate_polyfills,
            remove_unused_code,
            remove_unused_code_once,
            cross_chunk_code_motion,
            cross_chunk_method_motion,
            flow_sensitive_inline_variables,
            coalesce_variable_names,
            post_normalize_peephole,
            collapse_variable_declarations,
            extract_prototype_member_declarations,
            rewrite_function_expressions,
            collapse_anonymous_functions,
            rewrite_global_declarations_for_try_catch_wrapping,
            alias_strings,
            ambiguate_properties,
            mark_unnormalized,
            normalize,
            extern_exports,
            denormalize,
            invert_contextual_renaming,
            rename_properties,
            remove_property_renaming_calls,
            rename_vars,
            rename_labels,
            convert_to_dotted_properties,
            check_ast_validity,
            var_check_validity,
            instrument_for_code_coverage,
            gather_extern_properties_check,
            gather_extern_properties_optimize,
            polymer_pass,
            chrome_pass,
            j2cl_constant_hoister_pass,
            j2cl_clinit_pass,
            j2cl_property_inliner_pass,
            j2cl_pass,
            j2cl_util_get_define_rewriter_pass,
            j2cl_assert_removal_pass,
            j2cl_source_file_checker,
            j2cl_checks_pass,
            check_conformance,
            remove_cast_nodes,
            types_to_colors,
            serialize_typed_ast,
            remove_unnecessary_synthetic_externs,
            optimize_to_es6,
            whitespace_wrap_goog_modules,
            rewrite_common_js_modules,
            rewrite_scripts_to_es6_modules,
            gather_module_metadata_pass,
            create_module_map_pass,
            gather_getters_and_setters,
            add_synthetic_script,
            remove_synthetic_script,
            merge_synthetic_script,
            forbid_dynamic_import_usage,
            rewrite_dynamic_imports,
            protect_locale_data,
            substitute_locale_data,
            transpile_only_closure_unaware,
            transpile_and_optimize_closure_unaware,
        }
    }
    // port: DefaultPassConfig#getFullReplaceMessagesPass
    fn get_full_replace_messages_pass(&self) -> PassFactory {
        PassFactory::builder()
            .set_name(pass_names::REPLACE_MESSAGES)
            .set_internal_factory(Arc::new(
                |compiler: &mut crate::abstract_compiler::AbstractCompiler| {
                    let bundle = compiler.get_options().get_message_bundle().clone().unwrap();
                    let strict = compiler.get_options().get_strict_message_replacement();
                    Box::new(
                        crate::replace_messages::ReplaceMessages::new(
                            compiler, bundle, /* allow messages with goog.getMsg */
                            strict,
                        )
                        .get_full_replacement_pass(),
                    )
                },
            ))
            .build()
    }
    // port: DefaultPassConfig#getProtectMessagesPass
    fn get_protect_messages_pass(&self) -> PassFactory {
        PassFactory::builder()
            .set_name("protectMessages")
            .set_internal_factory(Arc::new(
                |compiler: &mut crate::abstract_compiler::AbstractCompiler| {
                    let bundle = compiler.get_options().get_message_bundle().clone().unwrap();
                    let strict = compiler.get_options().get_strict_message_replacement();
                    Box::new(
                        crate::replace_messages::ReplaceMessages::new(
                            compiler, bundle, /* allow messages with goog.getMsg */
                            strict,
                        )
                        .get_msg_protection_pass(),
                    )
                },
            ))
            .build()
    }
    // port: DefaultPassConfig#getReplaceProtectedMessagesPass
    fn get_replace_protected_messages_pass(&self) -> PassFactory {
        PassFactory::builder()
            .set_name("replaceProtectedMessages")
            .set_internal_factory(Arc::new(
                |compiler: &mut crate::abstract_compiler::AbstractCompiler| {
                    let bundle = compiler.get_options().get_message_bundle().clone().unwrap();
                    let strict = compiler.get_options().get_strict_message_replacement();
                    Box::new(
                        crate::replace_messages::ReplaceMessages::new(
                            compiler, bundle, /* allow messages with goog.getMsg */
                            strict,
                        )
                        .get_replacement_completion_pass(),
                    )
                },
            ))
            .build()
    }
    // port: DefaultPassConfig#createProcessDefines
    fn create_process_defines(mode: crate::process_defines::Mode) -> PassFactory {
        PassFactory::builder()
            .set_name(format!("processDefines_{}", mode.name()))
            .set_internal_factory(Arc::new(move |compiler| {
                let options = compiler.get_options().clone();
                let additional_replacements = Self::get_additional_replacements(compiler, &options);
                let define_replacements = options.get_define_replacements(compiler);
                Box::new(
                    crate::process_defines::Builder::new(compiler)
                        .put_replacements(additional_replacements)
                        .put_replacements(define_replacements)
                        .set_mode(mode)
                        .set_recognize_closure_defines(compiler.get_options().get_closure_pass())
                        .set_enable_zones_define_name(
                            options.get_enable_zones_define_name().map(str::to_string),
                        )
                        .set_zone_input_pattern(options.get_zone_input_pattern().clone())
                        .set_unknown_defines_to_ignore(
                            options.get_unknown_defines_to_ignore().clone(),
                        )
                        .build(compiler),
                )
            }))
            .build()
    }
    // port: DefaultPassConfig#combineChecks
    fn combine_checks(
        _compiler: &AbstractCompiler,
        callbacks: Vec<Box<dyn Callback>>,
    ) -> Box<dyn CompilerPass> {
        check_argument!(!callbacks.is_empty());
        Box::new(CombinedChecks { callbacks })
    }
    // port: DefaultPassConfig#runVariableRenaming
    fn run_variable_renaming(
        compiler: &mut crate::AbstractCompiler,
        prev_variable_map: Option<Arc<crate::variable_map::VariableMap>>,
        externs: NodeId,
        root: NodeId,
    ) -> crate::variable_map::VariableMap {
        let options = compiler.get_options();
        let reserved_chars: closure_rhino::fast_hash::IndexSet<u16> =
            closure_rhino::fast_hash::IndexSet::<_>::default();
        let mut reserved_names: closure_rhino::fast_hash::IndexSet<JsString> =
            closure_rhino::fast_hash::IndexSet::<_>::default();
        if let Some(rename_prefix_namespace) = options.get_rename_prefix_namespace() {
            // don't use the prefix name as a global symbol.
            reserved_names.insert(JsString::from(rename_prefix_namespace));
        }
        reserved_names.extend(
            compiler
                .get_exported_names()
                .iter()
                .map(|name| JsString::from(name.as_str())),
        );
        reserved_names.extend(ParserRunner::get_reserved_vars().iter().cloned());
        let mut rn = crate::rename_vars::RenameVars::new(
            options.get_rename_prefix().map(JsString::from),
            options.get_variable_renaming() == VariableRenamingPolicy::LOCAL,
            options.should_generate_pseudo_names(),
            options.should_prefer_stable_names(),
            prev_variable_map,
            reserved_chars,
            Some(reserved_names),
            Self::shared_name_generator(options),
        );
        rn.process(compiler, externs, root);
        rn.get_variable_map()
    }
    /// `options.getNameGenerator()`: the shared generator. Java's passes reset the shared instance
    /// before generating names (only `reset` and `generateNextName` are called on it, never
    /// `favors`), so a fresh `clone` of it (which keeps its character priorities) generates the
    /// same names. The Rust options keep the shared generator immutable behind an `Arc`.
    fn shared_name_generator(
        options: &CompilerOptions,
    ) -> Box<dyn crate::name_generator::NameGenerator> {
        crate::name_generator::NameGenerator::clone(
            &**options.get_name_generator(),
            Arc::new(std::sync::RwLock::new(
                closure_rhino::fast_hash::IndexSet::<_>::default(),
            )),
            JsString::from(""),
            &closure_rhino::fast_hash::IndexSet::<_>::default(),
        )
    }
    // port: DefaultPassConfig#createGatherExternProperties
    fn create_gather_extern_properties(mode: crate::gather_extern_properties::Mode) -> PassFactory {
        PassFactory::builder()
            .set_name("gatherExternProperties")
            .set_internal_factory(Arc::new(move |compiler| {
                Box::new(
                    crate::gather_extern_properties::GatherExternProperties::new(compiler, mode),
                )
            }))
            .build()
    }
    // port: DefaultPassConfig#getCustomPasses
    fn get_custom_passes(&self, execution_time: CustomPassExecutionTime) -> PassFactory {
        let passes = self.options.get_custom_passes_at(execution_time);
        PassFactory::builder()
            .set_name("runCustomPasses")
            .set_internal_factory(Arc::new(move |_| Self::run_in_serial(passes.clone())))
            .build()
    }
    // port: DefaultPassConfig#runInSerial
    fn run_in_serial(
        passes: Vec<Arc<std::sync::Mutex<dyn crate::compiler_pass::CompilerPass + Send>>>,
    ) -> Box<dyn crate::compiler_pass::CompilerPass> {
        Box::new(SerialPass(passes))
    }
    // port: DefaultPassConfig#getPreprocessorSymbolTable
    pub fn get_preprocessor_symbol_table(
        &self,
    ) -> Option<
        std::sync::Arc<std::sync::Mutex<crate::preprocessor_symbol_table::PreprocessorSymbolTable>>,
    > {
        self.preprocessor_symbol_table_factory
            .get_instance_or_null()
    }
    fn valid_global_symbol_namespace(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '$' || c == '_')
    }
    // port: DefaultPassConfig#requirePropertiesAreStaticallyAnalyzable
    pub fn require_properties_are_statically_analyzable(
        options: &CompilerOptions,
    ) -> PreconditionResult {
        PreconditionResult {
            success: options.get_assume_properties_are_statically_analyzable(),
            message: Some("requires assumePropertiesAreStaticallyAnalyzable to be enabled".into()),
        }
    }
    // port: DefaultPassConfig#getAdditionalReplacements
    pub fn get_additional_replacements(
        ast: &mut Ast,
        options: &CompilerOptions,
    ) -> IndexMap<String, NodeId> {
        let mut additional_replacements = IndexMap::<_, _>::default();
        if options.should_mark_as_compiled() || options.get_closure_pass() {
            additional_replacements.insert("COMPILED".into(), IR::true_node(ast));
        }
        if options.get_closure_pass()
            && options.get_locale().is_some()
            && !options.do_late_localization()
        {
            additional_replacements.insert(
                "goog.LOCALE".into(),
                IR::string(ast, options.get_locale().unwrap()),
            );
        }
        additional_replacements
    }
    // port: DefaultPassConfig#getTranspileOnlyPasses
    pub fn get_transpile_only_passes(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        passes.maybe_add(self.check_variable_references.clone());
        passes.maybe_add(self.check_vars.clone());
        passes.maybe_add(self.gather_module_metadata_pass.clone());
        passes.maybe_add(self.create_module_map_pass.clone());
        if options
            .get_language_in()
            .to_feature_set()
            .has(Feature::MODULES)
        {
            passes.maybe_add(self.rewrite_goog_js_imports.clone());
            if options.get_es6_module_transpilation() == Es6ModuleTranspilation::COMPILE {
                TranspilationPasses::add_es6_module_pass(
                    &mut passes,
                    &self.preprocessor_symbol_table_factory,
                );
            } else if options.get_es6_module_transpilation()
                == Es6ModuleTranspilation::TO_COMMON_JS_LIKE_MODULES
            {
                TranspilationPasses::add_es6_module_to_cjs_pass(&mut passes);
            } else if options.get_es6_module_transpilation()
                == Es6ModuleTranspilation::RELATIVIZE_IMPORT_PATHS
            {
                TranspilationPasses::add_es6_rewrite_import_path_pass(&mut passes);
            }
        }
        passes.maybe_add(self.check_super.clone());
        TranspilationPasses::add_transpilation_runtime_libraries(&mut passes);
        passes.maybe_add(self.transpile_only_closure_unaware.clone());
        if options.needs_transpilation_from(FeatureSet::ES2015) && options.get_rewrite_polyfills() {
            if options.get_isolate_polyfills() {
                panic!("Polyfill isolation cannot be used in transpileOnly mode");
            }
            TranspilationPasses::add_rewrite_polyfill_pass(&mut passes);
        } else if options.get_inject_polyfills_newer_than().is_some() {
            TranspilationPasses::add_rewrite_polyfill_pass(&mut passes);
        }
        passes.maybe_add(self.inject_runtime_libraries.clone());
        passes.maybe_add(self.normalize.clone());
        passes.maybe_add(self.gather_getters_and_setters.clone());
        TranspilationPasses::add_transpilation_passes(&mut passes, options);
        passes.maybe_add(self.mark_unnormalized.clone());
        passes.maybe_add(self.invert_contextual_renaming.clone());
        passes.maybe_add(self.remove_property_renaming_calls.clone());
        passes.assert_all_one_time_passes();
        self.assert_valid_order_for_checks(&mut passes);
        return passes;
    }
    // port: DefaultPassConfig#getWhitespaceOnlyPasses
    pub fn get_whitespace_only_passes(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        if options.get_process_common_js_modules() {
            passes.maybe_add(self.rewrite_common_js_modules.clone());
        } else if options
            .get_language_in()
            .to_feature_set()
            .has(Feature::MODULES)
        {
            passes.maybe_add(self.rewrite_scripts_to_es6_modules.clone());
        }
        if options.should_wrap_goog_modules_for_whitespace_only() {
            passes.maybe_add(self.whitespace_wrap_goog_modules.clone());
        }
        return passes;
    }
    // port: DefaultPassConfig#getChecks
    pub fn get_checks(&self) -> PassListBuilder {
        let options = &self.options;
        let mut checks = PassListBuilder::new(options.clone());
        assert!(
            !options.get_skip_non_transpilation_passes(),
            "options.getSkipNonTranspilationPasses() cannot be mixed with PassConfig::getChecks. Call PassConfig::getTranspileOnlyPasses instead."
        );
        if options.is_property_renaming_only_compilation_mode() {
            checks.maybe_add(self.add_synthetic_script.clone());
            checks.maybe_add(self.gather_getters_and_setters.clone());
            checks.maybe_add(self.gather_module_metadata_pass.clone());
            checks.maybe_add(self.create_module_map_pass.clone());
            checks.maybe_add(self.declared_global_externs_on_window.clone());
            checks.maybe_add(self.check_side_effects.clone());
            checks.maybe_add(self.angular_pass.clone());
            checks.maybe_add(self.closure_goog_scope_aliases.clone());
            checks.maybe_add(self.closure_primitives.clone());
            self.add_module_rewriting_passes(&mut checks);
            checks.maybe_add(self.clear_typed_scope_creator_pass.clone());
            checks.maybe_add(self.clear_top_typed_scope_pass.clone());
            checks.maybe_add(self.generate_exports.clone());
            checks.maybe_add(PassFactory::create_empty_pass(
                pass_names::AFTER_STANDARD_CHECKS,
            ));
            checks.maybe_add(self.merge_synthetic_script.clone());
            checks.maybe_add(self.gather_extern_properties_check.clone());
            checks.maybe_add(PassFactory::create_empty_pass(
                pass_names::BEFORE_SERIALIZATION,
            ));
            return checks;
        }
        if options.should_generate_typed_externs() {
            checks.maybe_add(self.add_synthetic_script.clone());
            checks.maybe_add(self.closure_goog_scope_aliases_for_ijs.clone());
            checks.maybe_add(self.generate_ijs.clone());
            checks.maybe_add(self.remove_extra_requires.clone());
            if options.should_wrap_goog_modules_for_whitespace_only() {
                checks.maybe_add(self.whitespace_wrap_goog_modules.clone());
            }
            checks.maybe_add(self.remove_synthetic_script.clone());
            return checks;
        }
        checks.maybe_add(self.add_synthetic_script.clone());
        checks.maybe_add(self.gather_getters_and_setters.clone());
        if options
            .get_language_in()
            .to_feature_set()
            .contains(Feature::DYNAMIC_IMPORT)
            && !options.should_allow_dynamic_import()
        {
            checks.maybe_add(self.forbid_dynamic_import_usage.clone());
        }
        checks.maybe_add(PassFactory::create_empty_pass("beforeStandardChecks"));
        if !options.get_process_common_js_modules()
            && options
                .get_language_in()
                .to_feature_set()
                .has(Feature::MODULES)
        {
            checks.maybe_add(self.rewrite_scripts_to_es6_modules.clone());
        }
        checks.maybe_add(self.gather_module_metadata_pass.clone());
        checks.maybe_add(self.create_module_map_pass.clone());
        if options.get_process_common_js_modules() {
            checks.maybe_add(self.rewrite_common_js_modules.clone());
        }
        if options.is_chrome_pass_enabled() {
            checks.maybe_add(self.chrome_pass.clone());
        }
        checks.maybe_add(self.check_js_doc_and_es6_modules.clone());
        checks.maybe_add(self.check_type_import_code_references.clone());
        if options.enables(&diagnostic_groups::LINT_CHECKS) {
            checks.maybe_add(self.lint_checks.clone());
        }
        if options.get_closure_pass() && options.enables(&diagnostic_groups::LINT_CHECKS) {
            checks.maybe_add(self.check_requires_and_provides_sorted.clone());
        }
        if options.enables(&diagnostic_groups::EXTRA_REQUIRE) {
            checks.maybe_add(self.extra_requires.clone());
        }
        if options.enables(&diagnostic_groups::MISSING_REQUIRE) {
            checks.maybe_add(self.check_missing_requires.clone());
        }
        checks.maybe_add(self.declared_global_externs_on_window.clone());
        if !options.get_process_common_js_modules() {
            checks.maybe_add(self.check_variable_references.clone());
            checks.maybe_add(self.check_vars.clone());
        }
        if options.get_closure_pass() {
            checks.maybe_add(self.check_closure_imports.clone());
        }
        checks.maybe_add(self.check_strict_mode.clone());
        if options.get_closure_pass() {
            checks.maybe_add(self.closure_check_module.clone());
        }
        checks.maybe_add(self.check_super.clone());
        checks.maybe_add(self.check_side_effects.clone());
        if options.get_angular_pass() {
            checks.maybe_add(self.angular_pass.clone());
        }
        if options.get_closure_pass() {
            checks.maybe_add(self.closure_goog_scope_aliases.clone());
        }
        if options.get_closure_pass() {
            checks.maybe_add(self.closure_primitives.clone());
        }
        if options.should_rewrite_modules_before_typechecking() {
            self.add_module_rewriting_passes(&mut checks);
        }
        if options.is_polymer_pass_enabled() {
            checks.maybe_add(self.polymer_pass.clone());
        }
        if options.get_process_common_js_modules() {
            checks.maybe_add(self.check_variable_references.clone());
            checks.maybe_add(self.check_vars.clone());
        }
        if options.should_infer_consts() {
            checks.maybe_add(self.infer_consts.clone());
        }
        if options.should_compute_function_side_effects() {
            checks.maybe_add(self.check_reg_exp.clone());
        }
        checks.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_TYPE_CHECKING,
        ));
        if options.get_check_types() || options.get_infer_types() {
            checks.maybe_add(self.infer_types.clone());
            if options.get_check_types() {
                checks.maybe_add(self.check_types.clone());
            } else {
                checks.maybe_add(self.infer_js_doc_info.clone());
            }
        }
        if options.enables(&diagnostic_groups::ANALYZER_CHECKS) && options.is_typechecking_enabled()
        {
            checks.maybe_add(self.analyzer_checks.clone());
        }
        if !options.preserves_detailed_source_info() {
            checks.maybe_add(self.clear_typed_scope_creator_pass.clone());
        }
        if options.should_rewrite_modules_after_typechecking() {
            self.add_module_rewriting_passes(&mut checks);
        }
        if options.should_allow_dynamic_import()
            && options
                .get_language_in()
                .to_feature_set()
                .has(Feature::DYNAMIC_IMPORT)
        {
            checks.maybe_add(self.rewrite_dynamic_imports.clone());
        }
        if !options.preserves_detailed_source_info() {
            checks.maybe_add(self.clear_top_typed_scope_pass.clone());
        }
        if options.get_check_suspicious_code()
            || options.enables(&diagnostic_groups::GLOBAL_THIS)
            || options.enables(&diagnostic_groups::DEBUGGER_STATEMENT_PRESENT)
        {
            checks.maybe_add(self.suspicious_code.clone());
        }
        if options.get_j2cl_pass().should_add_j2cl_passes() {
            checks.maybe_add(self.j2cl_source_file_checker.clone());
        }
        if !options.disables(&diagnostic_groups::CHECK_USELESS_CODE)
            || !options.disables(&diagnostic_groups::MISSING_RETURN)
        {
            checks.maybe_add(self.check_control_flow.clone());
        }
        if options.is_typechecking_enabled()
            && (!options.disables(&diagnostic_groups::ACCESS_CONTROLS)
                || options.enables(&diagnostic_groups::CONSTANT_PROPERTY))
        {
            checks.maybe_add(self.check_access_controls.clone());
        }
        checks.maybe_add(self.check_consts.clone());
        checks.maybe_add(self.rewrite_caller_code_location.clone());
        if !options.get_conformance_configs().is_empty() {
            checks.maybe_add(self.check_conformance.clone());
        }
        if options.get_tweak_processing().is_on() {
            checks.maybe_add(self.process_tweaks.clone());
        }
        checks.maybe_add(self.process_defines_check.clone());
        if options.get_j2cl_pass().should_add_j2cl_passes() {
            checks.maybe_add(self.j2cl_checks_pass.clone());
        }
        if options.should_generate_exports() {
            checks.maybe_add(self.generate_exports.clone());
        }
        checks.maybe_add(PassFactory::create_empty_pass(
            pass_names::AFTER_STANDARD_CHECKS,
        ));
        checks.maybe_add(self.merge_synthetic_script.clone());
        if options.get_extern_exports_path().is_some() {
            checks.maybe_add(self.extern_exports.clone());
        }
        if !options.is_checks_only() {
            checks.maybe_add(self.remove_weak_sources.clone());
        }
        checks.maybe_add(self.gather_extern_properties_check.clone());
        checks.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_SERIALIZATION,
        ));
        if options.get_typed_ast_output_file().is_some() {
            checks.maybe_add(self.serialize_typed_ast.clone());
        }
        checks.assert_all_one_time_passes();
        self.assert_valid_order_for_checks(&mut checks);
        return checks;
    }
    // port: DefaultPassConfig#getOptimizations
    pub fn get_optimizations(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        if options.is_property_renaming_only_compilation_mode() {
            passes.maybe_add(self.remove_unnecessary_synthetic_externs.clone());
            TranspilationPasses::add_transpilation_runtime_libraries(&mut passes);
            passes.maybe_add(self.closure_provides_requires.clone());
            passes.maybe_add(self.process_defines_optimize.clone());
            passes.maybe_add(self.normalize.clone());
            passes.maybe_add(self.gather_getters_and_setters.clone());
            TranspilationPasses::add_transpilation_passes(&mut passes, options);
            passes.maybe_add(self.gather_extern_properties_optimize.clone());
            passes.maybe_add(PassFactory::create_empty_pass(
                pass_names::BEFORE_STANDARD_OPTIMIZATIONS,
            ));
            passes.maybe_add(self.inline_and_collapse_properties.clone());
            passes.maybe_add(self.closure_optimize_primitives.clone());
            passes.maybe_add(self.strip_side_effect_protection.clone());
            return passes;
        }
        if options.get_skip_non_transpilation_passes() {
            return passes;
        }
        passes.add_all(&self.get_early_optimization_passes());
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::OPTIMIZATIONS_HALFWAY_POINT,
        ));
        passes.add_all(&self.get_late_optimization_passes());
        return passes;
    }
    // port: DefaultPassConfig#getFinalizations
    pub fn get_finalizations(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        if options.is_property_renaming_only_compilation_mode() {
            if options.should_rewrite_global_declarations_for_try_catch_wrapping()
                || options.get_rename_prefix_namespace().is_some()
            {
                passes.maybe_add(
                    self.rewrite_global_declarations_for_try_catch_wrapping
                        .clone(),
                );
            }
            passes.maybe_add(self.rename_properties.clone());
            if options.get_rename_prefix_namespace().is_some()
                && options.get_chunk_output_type() == ChunkOutputType::GLOBAL_NAMESPACE
            {
                if !Self::valid_global_symbol_namespace(
                    options.get_rename_prefix_namespace().unwrap(),
                ) {
                    panic!(
                        "Illegal character in renamePrefixNamespace name: {}",
                        options.get_rename_prefix_namespace().unwrap()
                    );
                }
                passes.maybe_add(self.rescope_global_symbols.clone());
            }
            return passes;
        }
        if options.do_late_localization() {
            if options.should_run_replace_messages_pass() {
                passes.maybe_add(self.get_replace_protected_messages_pass());
            }
            passes.maybe_add(self.substitute_locale_data.clone());
            passes.maybe_add(self.peephole_optimizations_once_normalized.clone());
        }
        if options.should_inline_variables() || options.should_inline_local_variables() {
            passes.maybe_add(self.flow_sensitive_inline_variables.clone());
            if !options.do_late_localization() && self.should_run_remove_unused_code() {
                passes.maybe_add(self.remove_unused_code_once.clone());
            }
        }
        if options.do_late_localization() {
            passes.add_all(&self.get_post_l10n_optimizations());
        }
        passes.maybe_add(PassFactory::create_empty_pass("beforeChunkMotion"));
        if options.should_run_cross_chunk_code_motion() {
            passes.maybe_add(self.cross_chunk_code_motion.clone());
        }
        if options.should_run_cross_chunk_method_motion() {
            passes.maybe_add(self.cross_chunk_method_motion.clone());
        }
        passes.maybe_add(PassFactory::create_empty_pass("afterChunkMotion"));
        if options.get_optimize_es_class_constructors()
            && options
                .get_output_feature_set()
                .contains(FeatureSet::ES2015)
        {
            passes.maybe_add(self.optimize_constructors.clone());
        }
        if options.get_isolate_polyfills() {
            passes.maybe_add(self.isolate_polyfills.clone());
        }
        if options.should_collapse_anonymous_functions() {
            passes.maybe_add(self.collapse_anonymous_functions.clone());
        }
        if options.should_rewrite_global_declarations_for_try_catch_wrapping()
            || options.get_rename_prefix_namespace().is_some()
        {
            passes.maybe_add(
                self.rewrite_global_declarations_for_try_catch_wrapping
                    .clone(),
            );
        }
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_EXTRACT_PROTOTYPE_MEMBER_DECLARATIONS,
        ));
        if options.get_extract_prototype_member_declarations_mode()
            != ExtractPrototypeMemberDeclarationsMode::OFF
        {
            passes.maybe_add(self.extract_prototype_member_declarations.clone());
        }
        if options.should_ambiguate_properties()
            && options.get_property_renaming() == PropertyRenamingPolicy::ALL_UNQUOTED
            && options.is_typechecking_enabled()
        {
            passes.maybe_add(self.ambiguate_properties.clone());
        }
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_RENAME_PROPERTIES,
        ));
        if options.get_property_renaming() == PropertyRenamingPolicy::ALL_UNQUOTED {
            passes.maybe_add(self.rename_properties.clone());
        } else {
            passes.maybe_add(self.remove_property_renaming_calls.clone());
        }
        if options.should_reserve_raw_exports() {
            passes.maybe_add(self.gather_raw_exports.clone());
        }
        if options.should_convert_to_dotted_properties() {
            passes.maybe_add(self.convert_to_dotted_properties.clone());
        }
        if options.should_rewrite_function_expressions() {
            passes.maybe_add(self.rewrite_function_expressions.clone());
        }
        if options.get_alias_strings_mode() != AliasStringsMode::NONE {
            passes.maybe_add(self.alias_strings.clone());
        }
        if options.should_coalesce_variable_names() {
            passes.maybe_add(self.coalesce_variable_names.clone());
            if options.should_fold_constants() {
                passes.maybe_add(self.peephole_optimizations_once_non_normalized.clone());
            }
        }
        passes.maybe_add(self.mark_unnormalized.clone());
        if options.should_collapse_variable_declarations()
            || options.should_optimize_let_and_const()
        {
            passes.maybe_add(self.post_normalize_peephole.clone());
        }
        if options.should_collapse_variable_declarations() {
            passes.maybe_add(self.collapse_variable_declarations.clone());
        }
        passes.maybe_add(self.denormalize.clone());
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_VARIABLE_RENAMING,
        ));
        if options.get_variable_renaming() != VariableRenamingPolicy::ALL {
            passes.maybe_add(self.invert_contextual_renaming.clone());
        }
        if options.get_variable_renaming() != VariableRenamingPolicy::OFF {
            passes.maybe_add(self.rename_vars.clone());
        }
        if options.should_rename_labels() {
            passes.maybe_add(self.rename_labels.clone());
        }
        if options.should_fold_constants() {
            passes.maybe_add(self.late_peephole_optimizations.clone());
        }
        if options.should_protect_hidden_side_effects()
            || options.get_merged_precompiled_libraries()
        {
            passes.maybe_add(self.strip_side_effect_protection.clone());
        }
        if options.get_rename_prefix_namespace().is_some()
            && options.get_chunk_output_type() == ChunkOutputType::GLOBAL_NAMESPACE
        {
            if !Self::valid_global_symbol_namespace(options.get_rename_prefix_namespace().unwrap())
            {
                panic!(
                    "Illegal character in renamePrefixNamespace name: {}",
                    options.get_rename_prefix_namespace().unwrap()
                );
            }
            passes.maybe_add(self.rescope_global_symbols.clone());
        }
        if options
            .get_output_feature_set()
            .contains(FeatureSet::ES2015)
        {
            passes.maybe_add(self.optimize_to_es6.clone());
        }
        if options.get_chunk_output_type() == ChunkOutputType::ES_MODULES {
            passes.maybe_add(self.convert_chunks_to_es_modules.clone());
        }
        passes.maybe_add(self.check_ast_validity.clone());
        passes.maybe_add(self.var_check_validity.clone());
        return passes;
    }
    // port: DefaultPassConfig#getEarlyOptimizationLoopPasses
    pub fn get_early_optimization_loop_passes(&self) -> PassListBuilder {
        let options = &self.options;
        let mut early_loop_passes = PassListBuilder::new(options.clone());
        if options.should_inline_variables() || options.should_inline_local_variables() {
            early_loop_passes.maybe_add(self.inline_variables.clone());
        } else if options.should_inline_constant_vars() {
            early_loop_passes.maybe_add(self.inline_constants.clone());
        }
        if options.get_collapse_object_literals() {
            early_loop_passes.maybe_add(self.collapse_object_literals.clone());
        }
        if self.should_run_remove_unused_code() {
            early_loop_passes.maybe_add(self.remove_unused_code.clone());
        }
        if options.should_fold_constants() {
            early_loop_passes.maybe_add(self.peephole_optimizations.clone());
        }
        early_loop_passes.assert_all_loopable_passes();
        return early_loop_passes;
    }
    // port: DefaultPassConfig#getPostL10nOptimizations
    pub fn get_post_l10n_optimizations(&self) -> PassListBuilder {
        let options = &self.options;
        let mut loop_passes = PassListBuilder::new(options.clone());
        if options.should_optimize_calls() {
            loop_passes.maybe_add(self.optimize_calls.clone());
        }
        if options.get_j2cl_pass().should_add_j2cl_passes() {
            loop_passes.maybe_add(self.j2cl_constant_hoister_pass.clone());
            loop_passes.maybe_add(self.j2cl_clinit_pass.clone());
        }
        if options.get_inline_functions_level() != Reach::NONE {
            loop_passes.maybe_add(self.inline_functions.clone());
        }
        if options.should_inline_variables() || options.should_inline_local_variables() {
            loop_passes.maybe_add(self.inline_variables.clone());
        } else if options.should_inline_constant_vars() {
            loop_passes.maybe_add(self.inline_constants.clone());
        }
        if self.should_run_remove_unused_code() {
            loop_passes.maybe_add(self.remove_unused_code.clone());
        }
        if options.should_fold_constants() {
            loop_passes.maybe_add(self.peephole_optimizations.clone());
        }
        loop_passes.assert_all_loopable_passes();
        return loop_passes;
    }
    // port: DefaultPassConfig#getEarlyOptimizationPasses
    pub fn get_early_optimization_passes(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        if options.should_export_test_functions() {
            passes.maybe_add(self.export_test_functions.clone());
        }
        if options.get_merged_precompiled_libraries() {
            passes.maybe_add(self.remove_weak_sources.clone());
            passes.maybe_add(self.check_reg_exp_for_optimizations.clone());
            if options.get_j2cl_pass().should_add_j2cl_passes() {
                passes.maybe_add(self.j2cl_source_file_checker.clone());
            }
        } else {
            self.add_non_typed_ast_normalization_passes(&mut passes);
        }
        passes.maybe_add(self.remove_unnecessary_synthetic_externs.clone());
        if options.get_synthetic_block_start_marker().is_some() {
            passes.maybe_add(self.create_synthetic_blocks.clone());
        }
        if options.get_j2cl_pass().should_add_j2cl_passes() {
            passes.maybe_add(self.j2cl_pass.clone());
        }
        passes.maybe_add(self.transpile_and_optimize_closure_unaware.clone());
        TranspilationPasses::add_transpilation_runtime_libraries(&mut passes);
        if options.get_rewrite_polyfills()
            || options.get_isolate_polyfills()
            || options.get_inject_polyfills_newer_than().is_some()
        {
            TranspilationPasses::add_rewrite_polyfill_pass(&mut passes);
        }
        passes.maybe_add(self.inject_runtime_libraries.clone());
        if options.get_closure_pass() {
            passes.maybe_add(self.closure_provides_requires.clone());
        }
        if options.should_run_replace_messages_for_chrome() {
            passes.maybe_add(self.replace_messages_for_chrome.clone());
        } else if options.should_run_replace_messages_pass() {
            if options.do_late_localization() {
                passes.maybe_add(self.get_protect_messages_pass());
            } else {
                passes.maybe_add(self.get_full_replace_messages_pass());
            }
        }
        if options.do_late_localization() {
            passes.maybe_add(self.protect_locale_data.clone());
        }
        if options.get_closure_pass() && !options.should_preserve_goog_library_primitives() {
            passes.maybe_add(self.closure_replace_get_css_name.clone());
        }
        passes.maybe_add(self.process_defines_optimize.clone());
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_EARLY_OPTIMIZATIONS_TRANSPILATION,
        ));
        passes.maybe_add(self.normalize.clone());
        passes.maybe_add(self.gather_getters_and_setters.clone());
        TranspilationPasses::add_transpilation_passes(&mut passes, options);
        if options.get_j2cl_pass().should_add_j2cl_passes() {
            passes.maybe_add(self.j2cl_util_get_define_rewriter_pass.clone());
        }
        if options.get_instrument_for_coverage_option() != InstrumentOption::NONE {
            passes.maybe_add(self.instrument_for_code_coverage.clone());
        }
        passes.maybe_add(self.gather_extern_properties_optimize.clone());
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_STANDARD_OPTIMIZATIONS,
        ));
        if options.get_closure_pass()
            && (options.should_remove_abstract_methods() || options.should_remove_closure_asserts())
        {
            passes.maybe_add(self.closure_code_removal.clone());
        }
        if options.should_remove_j2cl_asserts() {
            passes.maybe_add(self.j2cl_assert_removal_pass.clone());
        }
        passes.maybe_add(self.replace_toggles.clone());
        passes.maybe_add(self.inline_and_collapse_properties.clone());
        if options.get_tweak_processing().should_strip()
            || !options.get_strip_types().is_empty()
            || !options.get_strip_name_suffixes().is_empty()
            || !options.get_strip_name_prefixes().is_empty()
        {
            passes.maybe_add(self.strip_code.clone());
        }
        if options.should_replace_id_generators() {
            passes.maybe_add(self.replace_id_generators.clone());
        }
        if options.get_j2cl_pass().should_add_j2cl_passes()
            && options.get_property_collapse_level() == PropertyCollapseLevel::ALL
        {
            passes.maybe_add(self.j2cl_property_inliner_pass.clone());
        }
        if options.should_infer_consts() {
            passes.maybe_add(self.infer_consts.clone());
        }
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::OBFUSCATION_PASS_MARKER,
        ));
        if options.get_smart_name_removal() {
            if options.should_fold_constants()
                && (options.should_inline_variables() || options.should_inline_local_variables())
            {
                passes.maybe_add(self.early_inline_variables.clone());
                passes.maybe_add(self.early_peephole_optimizations.clone());
            }
            passes.maybe_add(self.remove_unused_code_once.clone());
        }
        if options.should_disambiguate_properties() && options.is_typechecking_enabled() {
            passes.maybe_add(self.disambiguate_properties.clone());
        }
        if options.should_compute_function_side_effects() {
            passes.maybe_add(self.mark_pure_functions.clone());
        }
        passes.assert_all_one_time_passes();
        return passes;
    }
    // port: DefaultPassConfig#getLateOptimizationPasses
    pub fn get_late_optimization_passes(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        if options.get_smart_name_removal() {
            passes.maybe_add(PassFactory::create_empty_pass(
                pass_names::BEFORE_EARLY_OPTIMIZATION_LOOP,
            ));
            passes.add_all(&self.get_early_optimization_loop_passes());
            passes.maybe_add(PassFactory::create_empty_pass(
                pass_names::AFTER_EARLY_OPTIMIZATION_LOOP,
            ));
        }
        if options.get_closure_pass() {
            passes.maybe_add(self.closure_optimize_primitives.clone());
        }
        if !options
            .get_replace_strings_function_descriptions()
            .is_empty()
        {
            passes.maybe_add(self.replace_strings.clone());
        }
        if options.should_run_cross_chunk_code_motion() {
            passes.maybe_add(self.cross_chunk_code_motion.clone());
        }
        if options.should_devirtualize_methods() {
            passes.maybe_add(self.devirtualize_methods.clone());
        }
        passes.maybe_add(self.get_custom_passes(CustomPassExecutionTime::BEFORE_OPTIMIZATION_LOOP));
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::BEFORE_MAIN_OPTIMIZATIONS,
        ));
        if options.should_inline_variables() || options.should_inline_local_variables() {
            passes.maybe_add(self.flow_sensitive_inline_variables.clone());
        }
        passes.add_all(&self.get_main_optimization_loop());
        passes.maybe_add(PassFactory::create_empty_pass(
            pass_names::AFTER_MAIN_OPTIMIZATIONS,
        ));
        passes.maybe_add(self.get_custom_passes(CustomPassExecutionTime::AFTER_OPTIMIZATION_LOOP));
        self.assert_valid_order_for_optimizations(&mut passes);
        return passes;
    }
    // port: DefaultPassConfig#getMainOptimizationLoop
    pub fn get_main_optimization_loop(&self) -> PassListBuilder {
        let options = &self.options;
        let mut passes = PassListBuilder::new(options.clone());
        if options.should_inline_getters() {
            passes.maybe_add(self.inline_simple_methods.clone());
        }
        if options.should_inline_properties() && options.is_typechecking_enabled() {
            passes.maybe_add(self.inline_properties.clone());
        }
        if options.should_run_dead_property_assignment_elimination() {
            passes.maybe_add(self.dead_property_assignment_elimination.clone());
        }
        if options.should_optimize_calls() {
            passes.maybe_add(self.optimize_calls.clone());
        }
        if options.get_j2cl_pass().should_add_j2cl_passes() {
            passes.maybe_add(self.j2cl_constant_hoister_pass.clone());
            passes.maybe_add(self.j2cl_clinit_pass.clone());
        }
        if options.get_inline_functions_level() != Reach::NONE {
            passes.maybe_add(self.inline_functions.clone());
        }
        if options.should_inline_variables() || options.should_inline_local_variables() {
            passes.maybe_add(self.inline_variables.clone());
        } else if options.should_inline_constant_vars() {
            passes.maybe_add(self.inline_constants.clone());
        }
        if options.should_run_dead_assignment_elimination() {
            passes.maybe_add(self.dead_assignments_elimination.clone());
        }
        if options.get_collapse_object_literals() {
            passes.maybe_add(self.collapse_object_literals.clone());
        }
        if self.should_run_remove_unused_code() {
            passes.maybe_add(self.remove_unused_code.clone());
        }
        if options.should_fold_constants() {
            passes.maybe_add(self.peephole_optimizations.clone());
        }
        passes.assert_all_loopable_passes();
        return passes;
    }
    // port: DefaultPassConfig#addModuleRewritingPasses
    pub fn add_module_rewriting_passes(&self, checks: &mut PassListBuilder) {
        let options = &self.options;
        if options
            .get_language_in()
            .to_feature_set()
            .has(Feature::MODULES)
        {
            checks.maybe_add(self.rewrite_goog_js_imports.clone());
            TranspilationPasses::add_es6_module_pass(
                checks,
                &self.preprocessor_symbol_table_factory,
            );
        }
        if options.get_closure_pass() {
            checks.maybe_add(self.closure_rewrite_module.clone());
        }
    }
    // port: DefaultPassConfig#addNonTypedAstNormalizationPasses
    pub fn add_non_typed_ast_normalization_passes(&self, passes: &mut PassListBuilder) {
        passes.maybe_add(self.remove_cast_nodes.clone());
        passes.maybe_add(self.types_to_colors.clone());
    }
    // port: DefaultPassConfig#shouldRunRemoveUnusedCode
    pub fn should_run_remove_unused_code(&self) -> bool {
        let options = &self.options;
        return options.should_remove_unused_variables()
            || options.should_remove_unused_local_variables()
            || options.should_remove_unused_prototype_properties()
            || options.is_remove_unused_class_properties()
            || options.get_rewrite_polyfills();
    }
    // port: DefaultPassConfig#assertValidOrderForChecks
    pub fn assert_valid_order_for_checks(&self, checks: &mut PassListBuilder) {
        checks.assert_pass_order(&self.declared_global_externs_on_window.clone(), &self.check_vars.clone(), "declaredGlobalExternsOnWindow must happen before VarCheck, which adds synthetic externs");
        checks.assert_pass_order(
            &self.chrome_pass.clone(),
            &self.check_js_doc_and_es6_modules.clone(),
            "The ChromePass must run before after JsDoc and Es6 module checking.",
        );
        checks.assert_pass_order(&self.closure_rewrite_module.clone(), &self.process_defines_check.clone(), "Must rewrite goog.module before processing @define's, so that @defines in modules work.");
        checks.assert_pass_order(
            &self.closure_primitives.clone(),
            &self.polymer_pass.clone(),
            "The Polymer pass must run after goog.provide processing.",
        );
        checks.assert_pass_order(
            &self.chrome_pass.clone(),
            &self.polymer_pass.clone(),
            "The Polymer pass must run after ChromePass processing.",
        );
        checks.assert_pass_order(
            &self.polymer_pass.clone(),
            &self.suspicious_code.clone(),
            "The Polymer pass must run before suspiciousCode processing.",
        );
        checks.assert_pass_order(
            &self.add_synthetic_script.clone(),
            &self.gather_module_metadata_pass.clone(),
            "Cannot add a synthetic script node after module metadata creation.",
        );
        checks.assert_pass_order(
            &self.closure_rewrite_module.clone(),
            &self.remove_synthetic_script.clone(),
            "Synthetic script node should be removed only after module rewriting.",
        );
        checks.assert_pass_order(&self.closure_rewrite_module.clone(), &self.rewrite_caller_code_location.clone(), "ClosureRewriteModule must happen before RewriteCallerCodeLocation, so that exported functions and call sites are rewritten correctly.");
        checks.assert_pass_order(&self.closure_rewrite_module.clone(), &TranspilationPasses::get_es6_rewrite_destructuring( ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS), "RewriteCallerCodeLocation must happen before Es6RewriteDestructuring, because we need ReWriteCallerCodeLocation to run before default parameters get rewritten.");
        checks.assert_pass_order(&self.closure_rewrite_module.clone(), &TranspilationPasses::get_es6_rewrite_destructuring( ObjectDestructuringRewriteMode::REWRITE_OBJECT_REST), "RewriteCallerCodeLocation must happen before Es6RewriteDestructuring, because we need ReWriteCallerCodeLocation to run before default parameters get rewritten.");
        if checks.contains(&self.closure_goog_scope_aliases) {
            assert!(
                checks.contains(&self.check_variable_references),
                "goog.scope processing requires variable checking"
            );
        }
        checks.assert_pass_order(
            &self.gather_module_metadata_pass.clone(),
            &self.closure_check_module.clone(),
            "Need to gather module metadata before checking closure modules.",
        );
        checks.assert_pass_order(
            &self.gather_module_metadata_pass.clone(),
            &self.create_module_map_pass.clone(),
            "Need to gather module metadata before scanning modules.",
        );
        checks.assert_pass_order(
            &self.create_module_map_pass.clone(),
            &self.rewrite_common_js_modules.clone(),
            "Need to gather module information before rewriting CommonJS modules.",
        );
        checks.assert_pass_order(
            &self.rewrite_scripts_to_es6_modules.clone(),
            &self.gather_module_metadata_pass.clone(),
            "Need to gather module information after rewriting scripts to modules.",
        );
        checks.assert_pass_order(
            &self.gather_module_metadata_pass.clone(),
            &self.check_missing_requires.clone(),
            "Need to gather module information before checking for missing requires.",
        );
    }
    /// Various peephole optimizations.
    // port: DefaultPassConfig#createPeepholeOptimizationsPass
    pub fn create_peephole_optimizations_pass(
        compiler: &mut AbstractCompiler,
        pass_name: &str,
        expect_ast_is_normalized: bool,
    ) -> Box<dyn CompilerPass> {
        check_argument!(
            expect_ast_is_normalized == compiler.get_life_cycle_stage().is_normalized(),
            "%s",
            format!("{:?}", compiler.get_life_cycle_stage())
        );
        let late = false;
        let use_types_for_optimization = compiler
            .get_options()
            .should_use_types_for_local_optimization();
        let mut optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = Vec::new();
        if expect_ast_is_normalized {
            // MinimizeExitPoints requires the AST to be normalized.
            optimizations.push(Box::new(MinimizeExitPoints::new()));
        }
        optimizations.push(Box::new(PeepholeMinimizeConditions::new(late)));
        optimizations.push(Box::new(PeepholeSubstituteAlternateSyntax::new(late)));
        optimizations.push(Box::new(PeepholeReplaceKnownMethods::new(
            late,
            use_types_for_optimization,
        )));
        optimizations.push(Box::new(PeepholeRemoveDeadCode::new()));
        if compiler
            .get_options()
            .get_j2cl_pass()
            .should_add_j2cl_passes()
        {
            optimizations.push(Box::new(J2clEqualitySameRewriterPass::new(
                use_types_for_optimization,
            )));
            optimizations.push(Box::new(J2clStringValueOfRewriterPass::new()));
            optimizations.push(Box::new(J2clUndefinedChecksRewriterPass::new()));
        }
        optimizations.push(Box::new(PeepholeFoldConstants::new(
            late,
            use_types_for_optimization,
        )));
        optimizations.push(Box::new(PeepholeCollectPropertyAssignments::new()));
        Box::new(PeepholeOptimizationsPass::new(pass_name, optimizations))
    }
    // port: DefaultPassConfig#assertValidOrderForOptimizations
    pub fn assert_valid_order_for_optimizations(&self, optimizations: &mut PassListBuilder) {
        optimizations.assert_pass_order(
            &self.j2cl_pass.clone(),
            &transpilation_passes::REWRITE_GENERATORS.clone(),
            "J2CL normalization should be done before generator re-writing.",
        );
        optimizations.assert_pass_order(&transpilation_passes::REWRITE_POLYFILLS.clone(), &self.process_defines_optimize.clone(), "Polyfill injection must be done before processDefines as some polyfills reference goog.defines.");
        optimizations.assert_pass_order(&transpilation_passes::INJECT_TRANSPILATION_RUNTIME_LIBRARIES.clone(), &self.process_defines_optimize.clone(), "Runtime library injection must be done before processDefines some runtime libraries reference goog.defines.");
        optimizations.assert_pass_order(&self.process_defines_optimize.clone(), &self.j2cl_util_get_define_rewriter_pass.clone(), "J2CL define re-writing should be done after processDefines since it relies on Compiler#getDefineNames to have been populated by it.");
        optimizations.assert_pass_order(&self.remove_unused_code.clone(), &self.isolate_polyfills.clone(), "Polyfill isolation should be done after RemovedUnusedCode. Otherwise unused polyfill removal will not find any polyfill usages and will delete all polyfills.");
        optimizations.assert_pass_order(
            &transpilation_passes::INSTRUMENT_ASYNC_CONTEXT.clone(),
            &transpilation_passes::REWRITE_ASYNC_ITERATION.clone(),
            "AsyncContext should be instrumentated before await and/or yield is transpiled away",
        );
        optimizations.assert_pass_order(
            &transpilation_passes::INSTRUMENT_ASYNC_CONTEXT.clone(),
            &transpilation_passes::REWRITE_ASYNC_FUNCTIONS.clone(),
            "AsyncContext should be instrumentated before await and/or yield is transpiled away",
        );
        optimizations.assert_pass_order(
            &transpilation_passes::INSTRUMENT_ASYNC_CONTEXT.clone(),
            &transpilation_passes::REWRITE_GENERATORS.clone(),
            "AsyncContext should be instrumentated before await and/or yield is transpiled away",
        );
    }
}
impl PassConfig for DefaultPassConfig {
    fn as_default_pass_config(&self) -> Option<&DefaultPassConfig> {
        Some(self)
    }
    fn get_options(&self) -> &CompilerOptions {
        &self.options
    }
    fn as_pass_config(&self) -> &dyn PassConfig {
        self
    }
    // port: DefaultPassConfig#getTranspileOnlyPasses
    fn get_transpile_only_passes(&self) -> PassListBuilder {
        DefaultPassConfig::get_transpile_only_passes(self)
    }
    // port: DefaultPassConfig#getWhitespaceOnlyPasses
    fn get_whitespace_only_passes(&self) -> PassListBuilder {
        DefaultPassConfig::get_whitespace_only_passes(self)
    }
    // port: DefaultPassConfig#getChecks
    fn get_checks(&self) -> PassListBuilder {
        DefaultPassConfig::get_checks(self)
    }
    // port: DefaultPassConfig#getOptimizations
    fn get_optimizations(&self) -> PassListBuilder {
        DefaultPassConfig::get_optimizations(self)
    }
    // port: DefaultPassConfig#getFinalizations
    fn get_finalizations(&self) -> PassListBuilder {
        DefaultPassConfig::get_finalizations(self)
    }
}
/// The anonymous pass of the stripCode factory: builds StripCode from the options when it runs.
struct StripCodeFactoryPass;
impl crate::compiler_pass::CompilerPass for StripCodeFactoryPass {
    // port: DefaultPassConfig#stripCode.process
    fn process(
        &mut self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        externs: NodeId,
        js_root: NodeId,
    ) {
        let options = compiler.get_options();
        let mut pass = crate::strip_code::StripCode::new(
            options.get_strip_types(),
            options.get_strip_name_suffixes(),
            options.get_strip_name_prefixes(),
            options.get_tweak_processing().should_strip(),
        );

        pass.process(compiler, externs, js_root);
    }
}
struct ReplaceStringsFactoryPass;
impl crate::compiler_pass::CompilerPass for ReplaceStringsFactoryPass {
    // port: DefaultPassConfig#replaceStrings.process
    fn process(
        &mut self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        let options = compiler.get_options();
        let placeholder_token = options.get_replace_strings_placeholder_token().to_owned();
        let function_descriptions = options.get_replace_strings_function_descriptions().clone();
        let mut pass = crate::replace_strings::ReplaceStrings::new(
            compiler,
            &placeholder_token,
            &function_descriptions,
        );
        pass.process(compiler, externs, root);
        compiler.set_string_map(Arc::new(pass.get_string_map()));
    }
}
struct SerialPass(Vec<Arc<std::sync::Mutex<dyn crate::compiler_pass::CompilerPass + Send>>>);
impl crate::compiler_pass::CompilerPass for SerialPass {
    // port: DefaultPassConfig#anonymous.runInSerial.process
    fn process(
        &mut self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        for pass in &self.0 {
            pass.lock().unwrap().process(compiler, externs, root);
        }
    }
}
/// The pass `combineChecks` returns: Java's `new CombinedCompilerPass(compiler, callbacks)`. The
/// Rust CombinedCompilerPass borrows its callbacks, so this pass owns them and lends them to it.
struct CombinedChecks {
    callbacks: Vec<Box<dyn Callback>>,
}
impl crate::compiler_pass::CompilerPass for CombinedChecks {
    // port: CombinedCompilerPass#process
    fn process(
        &mut self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        let callbacks: Vec<&mut dyn Callback> = self
            .callbacks
            .iter_mut()
            .map(|callback| callback.as_mut() as &mut dyn Callback)
            .collect();
        CombinedCompilerPass::new(compiler, callbacks).process(compiler, Some(externs), root);
    }
}

/// The anonymous CompilerPass of the checkRegExp and checkRegExpForOptimizations factories.
struct CheckRegExpAndRecordGlobalReferences {
    pass: crate::check_reg_exp::CheckRegExp,
}

impl crate::compiler_pass::CompilerPass for CheckRegExpAndRecordGlobalReferences {
    // port: DefaultPassConfig#checkRegExp (anonymous CompilerPass#process)
    fn process(
        &mut self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        self.pass.process(compiler, externs, root);
        compiler.set_has_reg_exp_global_references(self.pass.is_global_reg_exp_properties_used());
    }
}

/// A J2CL optimization pass whose class is not ported (docs/PORTING.md §2 excludes J2CL). Like the
/// Java pass, it returns at once when `J2clSourceFileChecker.shouldRunJ2clPasses` is false;
/// otherwise it reports the class as unported.
pub struct J2clGatedUnportedPass {
    class: &'static str,
    /// J2clClinitPrunerPass's `initialChangedScopeNodes`.
    #[allow(dead_code)]
    initial_changed_scope_nodes: Option<Vec<NodeId>>,
}
impl J2clGatedUnportedPass {
    pub fn new(class: &'static str, initial_changed_scope_nodes: Option<Vec<NodeId>>) -> Self {
        Self {
            class,
            initial_changed_scope_nodes,
        }
    }
}
impl crate::compiler_pass::CompilerPass for J2clGatedUnportedPass {
    // port: J2clPass#process (the shouldRunJ2clPasses gate every J2CL pass starts with)
    fn process(
        &mut self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        _externs: NodeId,
        _root: NodeId,
    ) {
        if !crate::j2cl_source_file_checker::J2clSourceFileChecker::should_run_j2cl_passes(compiler)
        {
            return;
        }
        PassFactory::report_unported(self.class);
    }
}

/// The anonymous `CompilerPass` of `DefaultPassConfig#generateExports`.
struct GenerateExportsPass {
    pass: crate::generate_exports::GenerateExports,
}
impl crate::compiler_pass::CompilerPass for GenerateExportsPass {
    // port: DefaultPassConfig#generateExports (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.pass.process(compiler, externs, root);
        compiler.add_exported_names(
            self.pass
                .get_exported_variable_names()
                .iter()
                .map(|name| name.to_string_lossy()),
        );
    }
}

/// DefaultPassConfig's anonymous `CompilerPass` of `replaceIdGenerators`: runs the pass with the
/// options' settings and stores its serialized id mappings on the compiler.
struct ReplaceIdGeneratorsPass;

impl CompilerPass for ReplaceIdGeneratorsPass {
    // port: DefaultPassConfig#replaceIdGenerators (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let options = compiler.get_options();
        let template_literals_are_transpiled = options.needs_transpilation_of(
            closure_parsing::parser::feature_set::Feature::TEMPLATE_LITERALS,
        );
        let id_generators = options.get_id_generators().clone();
        let generate_pseudo_names = options.should_generate_pseudo_names();
        let previous_map_serialized = options
            .get_id_generators_map_serialized()
            .map(str::to_string);
        let xid_hash_function = options.get_xid_hash_function().clone();
        let mut pass = crate::replace_id_generators::ReplaceIdGenerators::new(
            compiler,
            template_literals_are_transpiled,
            Some(&id_generators),
            generate_pseudo_names,
            previous_map_serialized.as_deref(),
            xid_hash_function,
        );
        pass.process(compiler, externs, root);
        compiler.set_id_generator_map(pass.get_serialized_id_mappings());
    }
}

/// DefaultPassConfig's anonymous `CompilerPass` of `closureReplaceGetCssName`: runs ReplaceCssNames
/// with the options' renaming map and skiplist, optionally gathering the CSS names.
struct ClosureReplaceGetCssNamePass;

impl CompilerPass for ClosureReplaceGetCssNamePass {
    // port: DefaultPassConfig#closureReplaceGetCssName (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, js_root: NodeId) {
        let options = compiler.get_options();
        let mut css_names: Option<closure_rhino::fast_hash::IndexSet<String>> =
            if options.should_gather_css_names() {
                Some(closure_rhino::fast_hash::IndexSet::<_>::default())
            } else {
                None
            };
        let css_renaming_map = options
            .get_css_renaming_map()
            .clone()
            .map(|map| -> Arc<dyn crate::css_renaming_map::CssRenamingMap> { map });
        let skiplist = options.get_css_renaming_skiplist().clone();

        {
            let mut pass = crate::replace_css_names::ReplaceCssNames::new(
                compiler,
                css_renaming_map,
                Box::new(|css_name| {
                    if let Some(builder) = css_names.as_mut() {
                        builder.insert(css_name.to_string());
                    }
                }),
                skiplist,
            );
            pass.process(compiler, externs, js_root);
        }

        compiler.set_css_names(css_names);
    }
}
