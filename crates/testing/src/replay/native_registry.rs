/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2005 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
 * Copyright 2023 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AngularPass.java,
//   src/com/google/javascript/jscomp/ChangeTracker.java,
//   src/com/google/javascript/jscomp/ClosureCodeRemoval.java,
//   src/com/google/javascript/jscomp/ClosureOptimizePrimitives.java,
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/CompilerInput.java,
//   src/com/google/javascript/jscomp/CompilerOptions.java,
//   src/com/google/javascript/jscomp/ConcretizeStaticInheritanceForInlining.java,
//   src/com/google/javascript/jscomp/ConvertChunksToESModules.java,
//   src/com/google/javascript/jscomp/DefaultNameGenerator.java,
//   src/com/google/javascript/jscomp/Es6RewriteScriptsToModules.java,
//   src/com/google/javascript/jscomp/GatherGetterAndSetterProperties.java,
//   src/com/google/javascript/jscomp/GatherModuleMetadata.java,
//   src/com/google/javascript/jscomp/GenerateExports.java,
//   src/com/google/javascript/jscomp/InjectTranspilationRuntimeLibraries.java,
//   src/com/google/javascript/jscomp/J2clChecksPass.java,
//   src/com/google/javascript/jscomp/J2clSourceFileChecker.java,
//   src/com/google/javascript/jscomp/MakeDeclaredNamesUnique.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/NodeUtil.java,
//   src/com/google/javascript/jscomp/Normalize.java,
//   src/com/google/javascript/jscomp/PhaseOptimizer.java,
//   src/com/google/javascript/jscomp/ProcessCommonJSModules.java,
//   src/com/google/javascript/jscomp/RemoveCastNodes.java,
//   src/com/google/javascript/jscomp/RemoveWeakSources.java,
//   src/com/google/javascript/jscomp/RenameLabels.java,
//   src/com/google/javascript/jscomp/ReplaceToggles.java,
//   src/com/google/javascript/jscomp/RescopeGlobalSymbols.java,
//   src/com/google/javascript/jscomp/RewriteAsyncFunctions.java,
//   src/com/google/javascript/jscomp/RewriteAsyncIteration.java,
//   src/com/google/javascript/jscomp/RewriteGlobalDeclarationsForTryCatchWrapping.java,
//   src/com/google/javascript/jscomp/SyntacticScopeCreator.java,
//   src/com/google/javascript/jscomp/WhitespaceWrapGoogModules.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for compiler APIs already present in the merged branch.
use crate::{
    replay::{
        options_values::OptionValue,
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_input::CompilerInput, syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.Compiler#getRoot()" => root,
        "com.google.javascript.jscomp.Compiler#getExternsRoot()" => externs_root,
        "com.google.javascript.jscomp.Compiler#getJsRoot()" => js_root,
        "com.google.javascript.jscomp.Compiler#getExternProperties()" => extern_properties,
        "com.google.javascript.jscomp.Compiler#getChunkGraph()" => chunk_graph,
        "com.google.javascript.jscomp.Compiler#getModuleMetadataMap()" => module_metadata_map,
        "com.google.javascript.jscomp.Compiler#getModuleMap()" => module_map,
        "com.google.javascript.jscomp.Compiler#getTypeRegistry()" => type_registry,
        "com.google.javascript.jscomp.Compiler#getReverseAbstractInterpreter()" => {
            reverse_abstract_interpreter
        }
        "com.google.javascript.jscomp.Compiler#setTypeCheckingHasRun(boolean)" => {
            set_type_checking_has_run
        }
        "com.google.javascript.jscomp.Compiler#forwardDeclareType(java.lang.String)" => {
            forward_declare_type
        }
        "com.google.javascript.jscomp.Compiler#getChangeTracker()" => change_tracker,
        "com.google.javascript.jscomp.ChangeTracker#getChangedScopeNodesForPass(java.lang.String)" => {
            changed_scope_nodes
        }
        "com.google.javascript.jscomp.Compiler#getSynthesizedExternsInput()" => {
            synthesized_externs_input
        }
        "com.google.javascript.jscomp.CompilerInput#getAstRoot(com.google.javascript.jscomp.AbstractCompiler)" => {
            input_ast_root
        }
        "com.google.javascript.jscomp.CompilerOptions#needsTranspilationOf(com.google.javascript.jscomp.parsing.parser.FeatureSet$Feature)" => {
            needs_transpilation_of
        }
        "com.google.javascript.jscomp.CompilerOptions#setIdGenerators(java.util.Map)" => {
            set_id_generators
        }
        "com.google.javascript.jscomp.CompilerOptions$LanguageMode#toFeatureSet()" => {
            language_features
        }
        "com.google.javascript.jscomp.CompilerOptions$BrowserFeaturesetYear#getFeatureSet()" => {
            browser_features
        }
        "com.google.javascript.jscomp.SyntacticScopeCreator#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            syntactic_scope_creator
        }
        "com.google.javascript.jscomp.DefaultNameGenerator#<init>()" => default_name_generator,
        "com.google.javascript.jscomp.Normalize#createNormalizeForOptimizations(com.google.javascript.jscomp.AbstractCompiler)" => {
            create_normalize_for_optimizations
        }
        "com.google.javascript.jscomp.MakeDeclaredNamesUnique#getContextualRenameInverter(com.google.javascript.jscomp.AbstractCompiler)" => {
            get_contextual_rename_inverter
        }
        "com.google.javascript.jscomp.RewriteAsyncFunctions#create(com.google.javascript.jscomp.AbstractCompiler)" => {
            rewrite_async_functions_create
        }
        "com.google.javascript.jscomp.RewriteAsyncIteration#create(com.google.javascript.jscomp.AbstractCompiler)" => {
            rewrite_async_iteration_create
        }
        "com.google.javascript.jscomp.InjectTranspilationRuntimeLibraries#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            inject_transpilation_runtime_libraries
        }
        "com.google.javascript.jscomp.RenameLabels#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            rename_labels
        }
        "com.google.javascript.jscomp.GatherModuleMetadata#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,com.google.javascript.jscomp.deps.ModuleLoader$ResolutionMode)" => {
            gather_module_metadata
        }
        "com.google.javascript.jscomp.Es6RewriteScriptsToModules#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            es6_rewrite_scripts_to_modules
        }
        "com.google.javascript.jscomp.WhitespaceWrapGoogModules#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            whitespace_wrap_goog_modules
        }
        "com.google.javascript.jscomp.ProcessCommonJSModules#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            process_common_js_modules
        }
        "com.google.javascript.jscomp.Compiler#getUniqueNameIdSupplier()" => unique_name_supplier,
        "com.google.javascript.jscomp.CompilerOptions#addCustomPass(com.google.javascript.jscomp.CustomPassExecutionTime,com.google.javascript.jscomp.CompilerPass)" => {
            add_custom_pass
        }
        "com.google.javascript.jscomp.NodeTraversal#traverse(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.rhino.Node,com.google.javascript.jscomp.NodeTraversal$Callback)" => {
            traverse
        }
        "com.google.javascript.jscomp.NodeUtil#visitPreOrder(com.google.javascript.rhino.Node,com.google.javascript.jscomp.NodeUtil$Visitor)" => {
            visit_pre_order
        }
        "com.google.javascript.jscomp.PhaseOptimizer#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PerformanceTracker)" => {
            phase_optimizer
        }
        "com.google.javascript.jscomp.PhaseOptimizer#addOneTimePass(com.google.javascript.jscomp.PassFactory)" => {
            add_one_time_pass
        }
        "com.google.javascript.rhino.IR#name(java.lang.String)" => ir_name,
        "com.google.javascript.rhino.IR#string(java.lang.String)" => ir_string,
        "com.google.javascript.rhino.IR#var(com.google.javascript.rhino.Node)" => ir_var,
        "com.google.javascript.rhino.Node#<init>(com.google.javascript.rhino.Token)" => new_node,
        "com.google.javascript.rhino.Node#newString(java.lang.String)" => new_string,
        "com.google.javascript.rhino.Node#addChildToBack(com.google.javascript.rhino.Node)" => {
            add_child_to_back
        }
        "com.google.javascript.rhino.Node#cloneTree()" => clone_tree,
        "com.google.javascript.rhino.Node#getParent()" => get_parent,
        "com.google.javascript.jscomp.RescopeGlobalSymbols#<init>(com.google.javascript.jscomp.AbstractCompiler,java.lang.String,boolean,boolean,com.google.javascript.jscomp.CompilerOptions$OptimizeLocalAccess)" => {
            rescope_global_symbols
        }
        "com.google.javascript.jscomp.ConvertChunksToESModules#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            convert_chunks_to_es_modules
        }
        "com.google.javascript.jscomp.RewriteGlobalDeclarationsForTryCatchWrapping#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            rewrite_global_declarations_for_try_catch_wrapping
        }
        "com.google.javascript.jscomp.J2clSourceFileChecker#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            j2cl_source_file_checker
        }
        "com.google.javascript.jscomp.J2clSourceFileChecker#markToRunJ2clPasses(com.google.javascript.jscomp.AbstractCompiler)" => {
            mark_to_run_j2cl_passes
        }
        "com.google.javascript.jscomp.J2clChecksPass#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            j2cl_checks_pass
        }
        "com.google.javascript.jscomp.RemoveWeakSources#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            remove_weak_sources
        }
        "com.google.javascript.jscomp.ConcretizeStaticInheritanceForInlining#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            concretize_static_inheritance_for_inlining
        }
        "com.google.javascript.jscomp.RemoveCastNodes#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            remove_cast_nodes
        }
        "com.google.javascript.jscomp.serialization.TypedAstSerializerTest_Helpers#<init>()" => {
            crate::replay::typed_ast_serializer_test_helpers::holder
        }
        "com.google.javascript.jscomp.serialization.TypedAstSerializerTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::typed_ast_serializer_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.serialization.SerializeTypedAstPass#<init>(com.google.javascript.jscomp.AbstractCompiler,java.util.function.Consumer,com.google.javascript.jscomp.serialization.SerializationOptions)" => {
            crate::replay::serialize_typed_ast_pass_helpers::serialize_typed_ast_pass
        }
        "com.google.javascript.jscomp.serialization.SerializationOptions#builder()" => {
            crate::replay::serialize_typed_ast_pass_helpers::builder
        }
        "com.google.javascript.jscomp.serialization.AutoBuilder_SerializationOptions_Builder#setIncludeDebugInfo(boolean)" => {
            crate::replay::serialize_typed_ast_pass_helpers::set_include_debug_info
        }
        "com.google.javascript.jscomp.serialization.AutoBuilder_SerializationOptions_Builder#setRuntimeLibraries(com.google.common.collect.ImmutableList)" => {
            crate::replay::serialize_typed_ast_pass_helpers::set_runtime_libraries
        }
        "com.google.javascript.jscomp.serialization.AutoBuilder_SerializationOptions_Builder#build()" => {
            crate::replay::serialize_typed_ast_pass_helpers::build
        }
        "com.google.javascript.jscomp.serialization.SerializeTypedAstPassTest_Compile#<init>()" => {
            crate::replay::serialize_typed_ast_pass_helpers::compile_holder
        }
        "com.google.javascript.jscomp.SerializeAndDeserializeAstTest_Helpers#<init>()" => {
            crate::replay::serialize_typed_ast_pass_helpers::ast_test_holder
        }
        "com.google.javascript.jscomp.SerializeAndDeserializeAstTest_Helpers#toInputStream()"
        | "com.google.javascript.jscomp.SerializeAndDeserializeAstTest_Helpers#runWithoutExpected()" => {
            crate::replay::serialize_typed_ast_pass_helpers::ast_test_new_consumer
        }
        "com.google.javascript.jscomp.serialization.JSTypeColorIdHasherTest_Helpers#<init>()" => {
            crate::replay::js_type_serialization_test_helpers::hasher_holder
        }
        "com.google.javascript.jscomp.serialization.JSTypeColorIdHasherTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::js_type_serialization_test_helpers::hasher_get_processor
        }
        "com.google.javascript.jscomp.serialization.JSTypeReconserializerTest_Helpers#<init>()" => {
            crate::replay::js_type_serialization_test_helpers::reconserializer_holder
        }
        "com.google.javascript.jscomp.serialization.JSTypeReconserializerTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::js_type_serialization_test_helpers::reconserializer_get_processor
        }
        "com.google.javascript.jscomp.LocaleDataPassesTest_Helpers#<init>()" => {
            crate::replay::locale_data_passes_test_helpers::holder
        }
        "com.google.javascript.jscomp.LocaleDataPassesTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::locale_data_passes_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.Es6RewriteGeneratorsTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.Es6RewriteGeneratorsTest_Helpers)" => {
            crate::replay::es6_rewrite_generators_test_helpers::get_processor_new
        }
        "com.google.javascript.jscomp.Es6RewriteGeneratorsTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::es6_rewrite_generators_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.ManageClosureUnawareCodeTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.ManageClosureUnawareCodeTest_Helpers)" => {
            crate::replay::manage_closure_unaware_code_test_helpers::get_processor_new
        }
        "com.google.javascript.jscomp.ManageClosureUnawareCodeTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::manage_closure_unaware_code_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.ClosureUnawarePhaseOptimizerTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::closure_unaware_phase_optimizer_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.ClosureUnawarePhaseOptimizerTest_Helpers#testClosureUnawareCodePassModifiesMainAST_incorrectly()" => {
            crate::replay::closure_unaware_phase_optimizer_test_helpers::test_closure_unaware_code_pass_modifies_main_ast_incorrectly
        }
        "com.google.javascript.jscomp.ClosureUnawarePhaseOptimizerTest_Helpers#testMainASTPassModifiesClosureUnawareCode_incorrectly()" => {
            crate::replay::closure_unaware_phase_optimizer_test_helpers::test_main_ast_pass_modifies_closure_unaware_code_incorrectly
        }
        "com.google.javascript.jscomp.MultiPassTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.MultiPassTest_Helpers,java.lang.String,java.lang.String[])" => {
            crate::replay::multi_pass_test_helpers::get_processor_new
        }
        "com.google.javascript.jscomp.MultiPassTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::multi_pass_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.ProcessClosureProvidesAndRequiresTest_Helpers$GetProcessorLambda#<init>(com.google.javascript.jscomp.ProcessClosureProvidesAndRequiresTest_Helpers,com.google.javascript.jscomp.Compiler)" => {
            crate::replay::process_closure_provides_and_requires_test_helpers::get_processor_lambda_new
        }
        "com.google.javascript.jscomp.TypeCheckFunctionCheckTest_Helpers$GetProcessorPass#<init>()" => {
            crate::replay::type_check_function_check_test_helpers::new_pass
        }
        "com.google.javascript.jscomp.TypeValidatorTest_Helpers$Anon1#<init>()" => {
            crate::replay::type_validator_test_helpers::new_pass
        }
        _ => {
            return crate::replay::native_dead_assignments::entry(signature)
                .or_else(|| harness_prereqs_entry(signature));
        }
    })
}
// port: ReplayDsl#invoke (harness-prereqs passes and their test helpers)
fn harness_prereqs_entry(signature: &str) -> Option<Entry> {
    use crate::replay::{
        ast_validator_test_helpers as av, gather_extern_properties_test_helpers as gep,
        infer_consts_test_helpers as ic, reference_collector_test_helpers as rc,
    };
    Some(match signature {
        "com.google.javascript.jscomp.AstValidatorTest_Helpers#<init>()" => av::holder,
        "com.google.javascript.jscomp.AstValidatorTest_Helpers$CreateValidatorViolationHandler#<init>(com.google.javascript.jscomp.AstValidatorTest_Helpers)" => {
            av::create_validator_violation_handler
        }
        "com.google.javascript.jscomp.AstValidator#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.AstValidator$ViolationHandler,boolean,boolean)" => {
            av::ast_validator
        }
        "com.google.javascript.jscomp.AstValidator#setTypeValidationMode(com.google.javascript.jscomp.AstValidator$TypeInfoValidation)" => {
            av::set_type_validation_mode
        }
        "com.google.javascript.jscomp.InferConstsTest_Helpers#<init>()" => ic::holder,
        "com.google.javascript.jscomp.InferConstsTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            ic::get_processor
        }
        "com.google.javascript.jscomp.GatherExternPropertiesTest_Helpers#<init>()" => gep::holder,
        "com.google.javascript.jscomp.GatherExternPropertiesTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            gep::get_processor
        }
        "com.google.javascript.jscomp.GatherExternPropertiesTest_Helpers$ExpectExterns#<init>(java.lang.String[])" => {
            gep::expect_externs
        }
        "com.google.javascript.jscomp.ReferenceCollector#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.ReferenceCollector$Behavior,com.google.javascript.jscomp.ScopeCreator)" => {
            rc::reference_collector
        }
        "com.google.javascript.jscomp.GatherGetterAndSetterProperties#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            gather_getter_and_setter_properties
        }
        "com.google.javascript.jscomp.ReplaceToggles#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            replace_toggles
        }
        "com.google.javascript.jscomp.ClosureCodeRemoval#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,boolean)" => {
            closure_code_removal
        }
        "com.google.javascript.jscomp.ClosureOptimizePrimitives#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean)" => {
            closure_optimize_primitives
        }
        "com.google.javascript.jscomp.AngularPass#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            angular_pass
        }
        "com.google.javascript.jscomp.GenerateExports#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,java.lang.String,java.lang.String)" => {
            generate_exports
        }
        "com.google.javascript.jscomp.ReplaceIdGenerators#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,java.util.Map,boolean,java.lang.String,com.google.javascript.jscomp.Xid$HashFunction)" => {
            crate::replay::replace_id_generators_test_helpers::replace_id_generators
        }
        "com.google.javascript.jscomp.ReplaceIdGeneratorsTest_Helpers$IdTestMap#<init>()" => {
            crate::replay::replace_id_generators_test_helpers::id_test_map
        }
        "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers#<init>()" => {
            crate::replay::replace_css_names_test_helpers::holder
        }
        "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers#getPartialMap()" => {
            crate::replay::replace_css_names_test_helpers::get_partial_map
        }
        "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers#getFullMap()" => {
            crate::replay::replace_css_names_test_helpers::get_full_map
        }
        "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::replace_css_names_test_helpers::get_processor
        }
        "com.google.javascript.jscomp.ProcessDefinesTest_Helpers$GetProcessorHost#<init>()" => {
            crate::replay::process_defines_test_helpers::host
        }
        "com.google.javascript.jscomp.ProcessDefinesTest_Helpers$GetProcessorHost#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::process_defines_test_helpers::get_processor
        }
        _ => {
            return rc::entry(signature)
                .or_else(|| crate::replay::suspicious_checks_natives::entry(signature))
                .or_else(|| crate::replay::check_reg_exp_test_helpers::entry(signature))
                .or_else(|| crate::replay::combined_compiler_pass_natives::entry(signature))
                .or_else(|| crate::replay::ijs_natives::entry(signature));
        }
    })
}
// port: GenerateExports#GenerateExports
fn generate_exports(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [
        _,
        DslValue::Bool(allow_non_global_exports),
        symbol,
        property,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let nullable_string = |value: &DslValue| match value {
        DslValue::Null => Ok(None),
        DslValue::String(s) => Ok(Some(s.clone())),
        _ => Err(bad()),
    };
    let pass = closure_jscomp::generate_exports::GenerateExports::new(
        &c.borrow(),
        *allow_non_global_exports,
        nullable_string(symbol)?,
        nullable_string(property)?,
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: AngularPass#AngularPass
fn angular_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = closure_jscomp::angular_pass::AngularPass::new(&c.borrow());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: ClosureOptimizePrimitives#ClosureOptimizePrimitives
fn closure_optimize_primitives(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, DslValue::Bool(can_use_es6_syntax)] = args.as_slice() else {
        return Err(bad());
    };
    let pass = closure_jscomp::closure_optimize_primitives::ClosureOptimizePrimitives::new(
        &c.borrow(),
        *can_use_es6_syntax,
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: ClosureCodeRemoval#ClosureCodeRemoval
fn closure_code_removal(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [
        _,
        DslValue::Bool(remove_abstract_methods),
        DslValue::Bool(remove_assertion_calls),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let pass = closure_jscomp::closure_code_removal::ClosureCodeRemoval::new(
        &c.borrow(),
        *remove_abstract_methods,
        *remove_assertion_calls,
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: ReplaceToggles#ReplaceToggles
fn replace_toggles(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = closure_jscomp::replace_toggles::ReplaceToggles::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: GatherGetterAndSetterProperties#GatherGetterAndSetterProperties
fn gather_getter_and_setter_properties(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
        closure_jscomp::gather_getter_and_setter_properties::GatherGetterAndSetterProperties::new(),
    )))))
}
// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(bad())
    }
}
// port: ReplayDsl#invoke (return value conversion)
fn node(value: Option<NodeId>) -> DslValue {
    value.map_or(DslValue::Null, DslValue::Node)
}
// port: Compiler#getRoot
fn root(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(node(compiler(&args)?.borrow().get_root()))
}
// port: Compiler#getExternsRoot
fn externs_root(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(node(compiler(&args)?.borrow().get_externs_root()))
}
// port: Compiler#getJsRoot
fn js_root(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(node(compiler(&args)?.borrow().get_js_root()))
}
// port: Compiler#getExternProperties
fn extern_properties(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?
        .borrow()
        .get_extern_properties()
        .cloned()
        .encode_value()
}
struct CompilerView {
    compiler: CompilerHandle,
    class: &'static str,
}
impl NativeObject for CompilerView {
    // port: ReplayDsl#invoke (compiler-owned object view)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: ReplayDsl#invoke (compiler-owned object identity)
fn view(compiler: CompilerHandle, class: &'static str) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(CompilerView { compiler, class })))
}
// port: Compiler#getChunkGraph
fn chunk_graph(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let present = c.borrow().get_chunk_graph().is_some();
    Ok(if present {
        view(c, "com.google.javascript.jscomp.JSChunkGraph")
    } else {
        DslValue::Null
    })
}
// port: Compiler#getModuleMetadataMap
fn module_metadata_map(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(
        crate::replay::native_closure_primitives::module_metadata_map_value(
            compiler(&args)?.borrow().get_module_metadata_map(),
        ),
    )
}
// port: Compiler#getModuleMap
fn module_map(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(crate::replay::native_es_modules::module_map_value(
        compiler(&args)?.borrow().get_module_map(),
    ))
}
// port: Compiler#getTypeRegistry
fn type_registry(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    crate::harness_passes::compiler_type_registry(&compiler(&args)?)
}
// port: Compiler#getReverseAbstractInterpreter
fn reverse_abstract_interpreter(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let compiler = compiler(&args)?;
    crate::harness_passes::compiler_type_registry(&compiler)?;
    Ok(
        crate::replay::native_type_inference::reverse_abstract_interpreter(
            &mut compiler.borrow_mut(),
        ),
    )
}
// port: Compiler#setTypeCheckingHasRun
fn set_type_checking_has_run(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?
        .borrow_mut()
        .set_type_checking_has_run(bool::decode_value(args.get(1).ok_or_else(bad)?)?);
    Ok(DslValue::Null)
}
// port: Compiler#forwardDeclareType
fn forward_declare_type(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?
        .borrow_mut()
        .forward_declare_type(String::decode_value(args.get(1).ok_or_else(bad)?)?);
    Ok(DslValue::Null)
}
// port: Compiler#getChangeTracker
fn change_tracker(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    c.borrow_mut().get_change_tracker();
    Ok(view(c, "com.google.javascript.jscomp.ChangeTracker"))
}
// port: ChangeTracker#getChangedScopeNodesForPass
fn changed_scope_nodes(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), name] = args.as_slice() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<CompilerView>()
        .ok_or_else(bad)?;
    let value = receiver
        .compiler
        .borrow_mut()
        .get_change_tracker()
        .get_changed_scope_nodes_for_pass(&String::decode_value(name)?);
    Ok(value.map_or(DslValue::Null, |nodes| {
        DslValue::List(nodes.into_iter().map(DslValue::Node).collect())
    }))
}
struct NativeInput(CompilerInput);
impl NativeObject for NativeInput {
    // port: ReplayDsl#invoke (CompilerInput receiver)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CompilerInput"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: Compiler#getSynthesizedExternsInput
fn synthesized_externs_input(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let input = compiler(&args)?
        .borrow_mut()
        .get_synthesized_externs_input()
        .clone();
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeInput(input)))))
}
// port: CompilerInput#getAstRoot
fn input_ast_root(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<NativeInput>()
        .ok_or_else(bad)?;
    Ok(DslValue::Node(receiver.0.get_ast_root(&mut c.borrow_mut())))
}
// port: CompilerOptions#needsTranspilationOf
fn needs_transpilation_of(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, feature] = args.as_slice() else {
        return Err(bad());
    };
    let feature = closure_parsing::parser::feature_set::Feature::decode_value(feature)?;
    Ok(DslValue::Bool(match receiver {
        DslValue::Options(o) => o.borrow().needs_transpilation_of(feature),
        DslValue::OptionsView(c) => c.borrow().get_options().needs_transpilation_of(feature),
        _ => return Err(bad()),
    }))
}
// port: CompilerOptions#setIdGenerators
fn set_id_generators(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, generators] = args.as_slice() else {
        return Err(bad());
    };
    let generators = IndexMap::decode_value(generators)?;
    match receiver {
        DslValue::Options(o) => o.borrow_mut().set_id_generators(generators),
        DslValue::OptionsView(c) => c
            .borrow_mut()
            .get_options_mut()
            .set_id_generators(generators),
        _ => return Err(bad()),
    }
    Ok(DslValue::Null)
}
// port: CompilerOptions.LanguageMode#toFeatureSet
fn language_features(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    crate::jscomp_api::LanguageMode::decode_value(args.first().ok_or_else(bad)?)?
        .to_feature_set()
        .encode_value()
}
// port: CompilerOptions.BrowserFeaturesetYear#getFeatureSet
fn browser_features(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    crate::jscomp_api::BrowserFeaturesetYear::decode_value(args.first().ok_or_else(bad)?)?
        .get_feature_set()
        .encode_value()
}
struct NativeScopeCreator {
    _creator: SyntacticScopeCreator<'static>,
}
impl NativeObject for NativeScopeCreator {
    // port: ReplayDsl#invoke (SyntacticScopeCreator receiver)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.SyntacticScopeCreator"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: SyntacticScopeCreator#SyntacticScopeCreator
fn syntactic_scope_creator(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeScopeCreator {
            _creator: SyntacticScopeCreator::new(),
        },
    ))))
}
// port: Normalize#createNormalizeForOptimizations
fn create_normalize_for_optimizations(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = closure_jscomp::normalize::Normalize::create_normalize_for_optimizations(
        &mut c.borrow_mut(),
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: MakeDeclaredNamesUnique#getContextualRenameInverter
fn get_contextual_rename_inverter(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = closure_jscomp::make_declared_names_unique::MakeDeclaredNamesUnique::get_contextual_rename_inverter(
        &mut c.borrow_mut(),
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
// port: RewriteAsyncFunctions#create
fn rewrite_async_functions_create(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass =
        closure_jscomp::rewrite_async_functions::RewriteAsyncFunctions::create(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: RewriteAsyncIteration#create
fn rewrite_async_iteration_create(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass =
        closure_jscomp::rewrite_async_iteration::RewriteAsyncIteration::create(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: InjectTranspilationRuntimeLibraries#InjectTranspilationRuntimeLibraries(AbstractCompiler)
fn inject_transpilation_runtime_libraries(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = closure_jscomp::inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries::new(
        &mut c.borrow_mut(),
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: RenameLabels#RenameLabels(AbstractCompiler)
fn rename_labels(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let pass: Box<dyn closure_jscomp::compiler_pass::CompilerPass> =
        Box::new(closure_jscomp::rename_labels::RenameLabels::new());
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: GatherModuleMetadata#GatherModuleMetadata
fn gather_module_metadata(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let [_, process_common_js_modules, module_resolution_mode] = args.as_slice() else {
        return Err(bad());
    };
    let process_common_js_modules = bool::decode_value(process_common_js_modules)?;
    let module_resolution_mode =
        crate::replay::options_fields::resolution_mode(module_resolution_mode)?;
    Ok(native_pass(
        "com.google.javascript.jscomp.GatherModuleMetadata",
        closure_jscomp::gather_module_metadata::GatherModuleMetadata::new(
            process_common_js_modules,
            module_resolution_mode,
        ),
    ))
}
// port: Es6RewriteScriptsToModules#Es6RewriteScriptsToModules
fn es6_rewrite_scripts_to_modules(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(native_pass(
        "com.google.javascript.jscomp.Es6RewriteScriptsToModules",
        closure_jscomp::es6_rewrite_scripts_to_modules::Es6RewriteScriptsToModules::new(),
    ))
}
// port: WhitespaceWrapGoogModules#WhitespaceWrapGoogModules
fn whitespace_wrap_goog_modules(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(native_pass(
        "com.google.javascript.jscomp.WhitespaceWrapGoogModules",
        closure_jscomp::whitespace_wrap_goog_modules::WhitespaceWrapGoogModules::new(),
    ))
}
// port: ProcessCommonJSModules#ProcessCommonJSModules
fn process_common_js_modules(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(native_pass(
        "com.google.javascript.jscomp.ProcessCommonJSModules",
        closure_jscomp::process_common_js_modules::ProcessCommonJSModules::new(),
    ))
}
/// A ported `CompilerPass` that keeps its Java runtime class, so that DSL calls such as
/// `new GatherModuleMetadata(...).process(externs, root)` inside a processor lambda resolve
/// through the recorded `<Class>.process` signature.
struct NativePass {
    class: &'static str,
    pass: Box<dyn closure_jscomp::compiler_pass::CompilerPass>,
}
impl NativeObject for NativePass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayValues#findField (native object adapter)
    // No field of these passes holds a UnitRecorder result producer, so UnitRecorder#collect
    // finds nothing below them (as for an opaque ported CompilerPass).
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplayDsl.SequencePass#process (native CompilerPass adapter)
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.pass.process(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: ReplayDsl#invoke (constructor of a ported CompilerPass)
fn native_pass(
    class: &'static str,
    pass: impl closure_jscomp::compiler_pass::CompilerPass + 'static,
) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativePass {
        class,
        pass: Box::new(pass),
    })))
}

// port: DefaultNameGenerator#DefaultNameGenerator
fn default_name_generator(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::NameGenerator(std::sync::Arc::new(
        closure_jscomp::default_name_generator::DefaultNameGenerator::new(),
    )))
}

// port: ReplayDsl#invoke (real Compiler API through a factory's existing mutable loan)
pub fn invoke_borrowed(
    signature: &str,
    ctx: &mut Ctx,
    args: &[DslValue],
    compiler: &mut crate::jscomp_api::Compiler,
) -> Option<Result<DslValue, Throwable>> {
    if let Some(result) = invoke_node(signature, args, compiler) {
        return Some(result);
    }
    // CompilerPass#process(Node, Node) on a ported pass, called from a DSL lambda body.
    if signature
        .ends_with("#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)")
        && let [
            DslValue::Native(pass),
            DslValue::Node(externs),
            DslValue::Node(root),
        ] = args
    {
        return Some(
            pass.borrow_mut()
                .process_with_ctx(compiler, *externs, *root, ctx)
                .map(|()| DslValue::Null),
        );
    }
    let current = ctx.compiler.as_ref();
    let same = match args.first() {
        Some(DslValue::Compiler(c)) => current.is_some_and(|current| Rc::ptr_eq(c, current)),
        _ => false,
    };
    if same {
        let result = match signature {
            "com.google.javascript.jscomp.Compiler#getRoot()" => Ok(node(compiler.get_root())),
            "com.google.javascript.jscomp.Compiler#getJsRoot()" => Ok(node(compiler.get_js_root())),
            "com.google.javascript.jscomp.Compiler#getExternsRoot()" => {
                Ok(node(compiler.get_externs_root()))
            }
            "com.google.javascript.jscomp.Compiler#getExternProperties()" => {
                compiler.get_extern_properties().cloned().encode_value()
            }
            "com.google.javascript.jscomp.Compiler#setTypeCheckingHasRun(boolean)" => {
                bool::decode_value(&args[1]).map(|value| {
                    compiler.set_type_checking_has_run(value);
                    DslValue::Null
                })
            }
            "com.google.javascript.jscomp.Compiler#forwardDeclareType(java.lang.String)" => {
                String::decode_value(&args[1]).map(|name| {
                    compiler.forward_declare_type(name);
                    DslValue::Null
                })
            }
            "com.google.javascript.jscomp.Compiler#getChangeTracker()" => {
                compiler.get_change_tracker();
                Ok(view(
                    current.unwrap().clone(),
                    "com.google.javascript.jscomp.ChangeTracker",
                ))
            }
            "com.google.javascript.jscomp.Compiler#getChunkGraph()" => {
                Ok(if compiler.get_chunk_graph().is_some() {
                    view(
                        current.unwrap().clone(),
                        "com.google.javascript.jscomp.JSChunkGraph",
                    )
                } else {
                    DslValue::Null
                })
            }
            "com.google.javascript.jscomp.Compiler#getModuleMap()" => Ok(
                crate::replay::native_es_modules::module_map_value(compiler.get_module_map()),
            ),
            "com.google.javascript.jscomp.Compiler#getModuleMetadataMap()" => Ok(
                crate::replay::native_closure_primitives::module_metadata_map_value(
                    compiler.get_module_metadata_map(),
                ),
            ),
            "com.google.javascript.jscomp.Compiler#getTypeRegistry()" => match &args[0] {
                DslValue::Compiler(handle) => Ok(
                    crate::harness_passes::compiler_type_registry_borrowed(handle, compiler),
                ),
                _ => unreachable!("checked above"),
            },
            "com.google.javascript.jscomp.Compiler#getReverseAbstractInterpreter()" => {
                compiler.get_type_registry();
                Ok(crate::replay::native_type_inference::reverse_abstract_interpreter(compiler))
            }
            "com.google.javascript.jscomp.Compiler#getSynthesizedExternsInput()" => {
                Ok(DslValue::Native(Rc::new(RefCell::new(NativeInput(
                    compiler.get_synthesized_externs_input().clone(),
                )))))
            }
            "com.google.javascript.jscomp.Compiler#getUniqueNameIdSupplier()" => {
                Ok(DslValue::Native(Rc::new(RefCell::new(NameSupplier(
                    compiler.get_unique_name_id_supplier(),
                )))))
            }
            "com.google.javascript.jscomp.PhaseOptimizer#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PerformanceTracker)" => {
                native_phase_optimizer(compiler, args)
            }
            "com.google.javascript.jscomp.RewriteAsyncFunctions#create(com.google.javascript.jscomp.AbstractCompiler)" => {
                Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
                    closure_jscomp::rewrite_async_functions::RewriteAsyncFunctions::create(
                        compiler,
                    ),
                )))))
            }
            "com.google.javascript.jscomp.RewriteAsyncIteration#create(com.google.javascript.jscomp.AbstractCompiler)" => {
                Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
                    closure_jscomp::rewrite_async_iteration::RewriteAsyncIteration::create(
                        compiler,
                    ),
                )))))
            }
            "com.google.javascript.jscomp.InjectTranspilationRuntimeLibraries#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
                Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
                    closure_jscomp::inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries::new(
                        compiler,
                    ),
                )))))
            }
            "com.google.javascript.jscomp.NodeTraversal#traverse(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.rhino.Node,com.google.javascript.jscomp.NodeTraversal$Callback)" =>
            {
                let [_, DslValue::Node(root), DslValue::Native(callback)] = args else {
                    return Some(Err(bad()));
                };
                let mut callback = callback.borrow_mut();
                let class = callback.class_name().to_string();
                match callback.as_traversal_callback() {
                    Some(callback) => {
                        closure_jscomp::node_traversal::NodeTraversal::traverse(
                            compiler, *root, callback,
                        );
                        Ok(DslValue::Null)
                    }
                    None => Err(Throwable::Unported(class)),
                }
            }
            _ => return None,
        };
        return Some(result);
    }
    if signature
        == "com.google.javascript.jscomp.CompilerOptions#addCustomPass(com.google.javascript.jscomp.CustomPassExecutionTime,com.google.javascript.jscomp.CompilerPass)"
        && let [DslValue::OptionsView(c), time, DslValue::Native(pass)] = args
        && current.is_some_and(|current| Rc::ptr_eq(c, current))
    {
        return Some((|| {
            let time = crate::jscomp_api::CustomPassExecutionTime::decode_value(time)?;
            let mut pass = pass.borrow_mut();
            let class = pass.class_name().to_string();
            let pass = pass.as_custom_pass().ok_or(Throwable::Unported(class))?;
            compiler.get_options_mut().add_custom_pass(time, pass);
            Ok(DslValue::Null)
        })());
    }
    if let [DslValue::OptionsView(c), value] = args
        && current.is_some_and(|current| Rc::ptr_eq(c, current))
    {
        return match signature {
            "com.google.javascript.jscomp.CompilerOptions#needsTranspilationOf(com.google.javascript.jscomp.parsing.parser.FeatureSet$Feature)" => {
                Some(
                    closure_parsing::parser::feature_set::Feature::decode_value(value).map(
                        |feature| {
                            DslValue::Bool(compiler.get_options().needs_transpilation_of(feature))
                        },
                    ),
                )
            }
            "com.google.javascript.jscomp.CompilerOptions#setIdGenerators(java.util.Map)" => {
                Some(IndexMap::decode_value(value).map(|generators| {
                    compiler.get_options_mut().set_id_generators(generators);
                    DslValue::Null
                }))
            }
            _ => None,
        };
    }
    if let [DslValue::Native(receiver), value] = args {
        let mut receiver = receiver.borrow_mut();
        if signature
            == "com.google.javascript.jscomp.ChangeTracker#getChangedScopeNodesForPass(java.lang.String)"
            && let Some(view) = receiver.as_any_mut().downcast_mut::<CompilerView>()
            && current.is_some_and(|current| Rc::ptr_eq(&view.compiler, current))
        {
            return Some(String::decode_value(value).map(|name| {
                compiler
                    .get_change_tracker()
                    .get_changed_scope_nodes_for_pass(&name)
                    .map_or(DslValue::Null, |nodes| {
                        DslValue::List(nodes.into_iter().map(DslValue::Node).collect())
                    })
            }));
        }
        if signature
            == "com.google.javascript.jscomp.CompilerInput#getAstRoot(com.google.javascript.jscomp.AbstractCompiler)"
            && let DslValue::Compiler(c) = value
            && current.is_some_and(|current| Rc::ptr_eq(c, current))
            && let Some(input) = receiver.as_any_mut().downcast_mut::<NativeInput>()
        {
            return Some(Ok(DslValue::Node(input.0.get_ast_root(compiler))));
        }
    }
    if signature
        == "com.google.javascript.jscomp.NodeUtil#visitPreOrder(com.google.javascript.rhino.Node,com.google.javascript.jscomp.NodeUtil$Visitor)"
    {
        let [DslValue::Node(root), DslValue::Native(visitor)] = args else {
            return Some(Err(bad()));
        };
        let mut visitor = visitor.borrow_mut();
        let class = visitor.class_name().to_string();
        return Some(match visitor.as_node_visitor() {
            Some(visitor) => {
                closure_jscomp::node_util::NodeUtil::visit_pre_order(compiler, *root, visitor);
                Ok(DslValue::Null)
            }
            None => Err(Throwable::Unported(class)),
        });
    }
    None
}

// port: ReplayDsl#invoke (nodes use the executing compiler's arena)
fn invoke_node(
    signature: &str,
    args: &[DslValue],
    compiler: &mut crate::jscomp_api::Compiler,
) -> Option<Result<DslValue, Throwable>> {
    use closure_rhino::{ir::IR, token::Token};
    if !signature.starts_with("com.google.javascript.rhino.IR#")
        && !signature.starts_with("com.google.javascript.rhino.Node#")
    {
        return None;
    }
    Some((|| {
        Ok(match signature {
            "com.google.javascript.rhino.IR#name(java.lang.String)" => DslValue::Node(IR::name(
                &mut compiler.ast,
                closure_rhino::js_string::JsString::decode_value(&args[0])?,
            )),
            "com.google.javascript.rhino.IR#string(java.lang.String)" => {
                DslValue::Node(IR::string(
                    &mut compiler.ast,
                    closure_rhino::js_string::JsString::decode_value(&args[0])?,
                ))
            }
            "com.google.javascript.rhino.IR#var(com.google.javascript.rhino.Node)" => {
                let [DslValue::Node(name)] = args else {
                    return Err(bad());
                };
                DslValue::Node(IR::var(&mut compiler.ast, *name))
            }
            "com.google.javascript.rhino.Node#<init>(com.google.javascript.rhino.Token)" => {
                DslValue::Node(compiler.ast.new_node(Token::decode_value(&args[0])?))
            }
            "com.google.javascript.rhino.Node#newString(java.lang.String)" => DslValue::Node(
                compiler
                    .ast
                    .new_string(closure_rhino::js_string::JsString::decode_value(&args[0])?),
            ),
            "com.google.javascript.rhino.Node#addChildToBack(com.google.javascript.rhino.Node)" => {
                let [DslValue::Node(parent), DslValue::Node(child)] = args else {
                    return Err(bad());
                };
                parent.add_child_to_back(&mut compiler.ast, *child);
                DslValue::Null
            }
            "com.google.javascript.rhino.Node#cloneTree()" => {
                let [DslValue::Node(root)] = args else {
                    return Err(bad());
                };
                DslValue::Node(root.clone_tree(&mut compiler.ast))
            }
            "com.google.javascript.rhino.Node#getParent()" => {
                let [DslValue::Node(root)] = args else {
                    return Err(bad());
                };
                node(root.get_parent(&compiler.ast))
            }
            _ => {
                return Err(Throwable::HarnessError(
                    "unresolved native Node dispatch".into(),
                ));
            }
        })
    })())
}

macro_rules! node_entry {
    ($name:ident, $java:literal, $signature:literal) => {
        #[doc = concat!("Native replay for ", $java)]
        // port: ReplayDsl#invoke (ported Node/IR operation)
        fn $name(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
            let compiler = ctx.compiler.clone().ok_or_else(bad)?;
            invoke_node($signature, &args, &mut compiler.borrow_mut())
                .expect("registered Node signature")
        }
    };
}
node_entry!(
    ir_name,
    "IR#name",
    "com.google.javascript.rhino.IR#name(java.lang.String)"
);
node_entry!(
    ir_string,
    "IR#string",
    "com.google.javascript.rhino.IR#string(java.lang.String)"
);
node_entry!(
    ir_var,
    "IR#var",
    "com.google.javascript.rhino.IR#var(com.google.javascript.rhino.Node)"
);
node_entry!(
    new_node,
    "Node#Node",
    "com.google.javascript.rhino.Node#<init>(com.google.javascript.rhino.Token)"
);
node_entry!(
    new_string,
    "Node#newString",
    "com.google.javascript.rhino.Node#newString(java.lang.String)"
);
node_entry!(
    add_child_to_back,
    "Node#addChildToBack",
    "com.google.javascript.rhino.Node#addChildToBack(com.google.javascript.rhino.Node)"
);
node_entry!(
    clone_tree,
    "Node#cloneTree",
    "com.google.javascript.rhino.Node#cloneTree()"
);
node_entry!(
    get_parent,
    "Node#getParent",
    "com.google.javascript.rhino.Node#getParent()"
);
pub(crate) struct NameSupplier(pub(crate) std::sync::Arc<dyn Fn() -> String + Send + Sync>);
impl NativeObject for NameSupplier {
    // port: Compiler#getUniqueNameIdSupplier (functional interface)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Compiler$$Lambda"
    }
    // port: ReplayDsl#invoke (native supplier)
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "get" {
            Ok(DslValue::String((self.0)().into()))
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: Compiler#getUniqueNameIdSupplier
fn unique_name_supplier(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(NameSupplier(
        compiler(&args)?.borrow().get_unique_name_id_supplier(),
    )))))
}
// port: CompilerOptions#addCustomPass
fn add_custom_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, time, DslValue::Native(pass)] = args.as_slice() else {
        return Err(bad());
    };
    let time = crate::jscomp_api::CustomPassExecutionTime::decode_value(time)?;
    let mut pass = pass.borrow_mut();
    let class = pass.class_name().to_string();
    let pass = pass.as_custom_pass().ok_or(Throwable::Unported(class))?;
    match receiver {
        DslValue::Options(o) => o.borrow_mut().add_custom_pass(time, pass),
        DslValue::OptionsView(c) => c.borrow_mut().get_options_mut().add_custom_pass(time, pass),
        _ => return Err(bad()),
    }
    Ok(DslValue::Null)
}
// port: NodeTraversal#traverse
fn traverse(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(c),
        DslValue::Node(root),
        DslValue::Native(callback),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let mut callback = callback.borrow_mut();
    let class = callback.class_name().to_string();
    let callback = callback
        .as_traversal_callback()
        .ok_or(Throwable::Unported(class))?;
    closure_jscomp::node_traversal::NodeTraversal::traverse(&mut c.borrow_mut(), *root, callback);
    Ok(DslValue::Null)
}
// port: NodeUtil#visitPreOrder
fn visit_pre_order(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Node(root), DslValue::Native(visitor)] = args.as_slice() else {
        return Err(bad());
    };
    let c = ctx.compiler.clone().ok_or_else(bad)?;
    let mut visitor = visitor.borrow_mut();
    let class = visitor.class_name().to_string();
    let visitor = visitor
        .as_node_visitor()
        .ok_or(Throwable::Unported(class))?;
    closure_jscomp::node_util::NodeUtil::visit_pre_order(&mut c.borrow_mut(), *root, visitor);
    Ok(DslValue::Null)
}
pub(crate) struct NativePhaseOptimizer(pub(crate) closure_jscomp::phase_optimizer::PhaseOptimizer);
impl NativeObject for NativePhaseOptimizer {
    // port: ReplayDsl#invoke (PhaseOptimizer receiver)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.PhaseOptimizer"
    }
    // port: ReplayValues#findField (native object adapter)
    // UnitRecorder#collect walks a processor's instance fields for result producers. A
    // PhaseOptimizer reaches only its compiler (not walked), its tracker, and NamedPass /
    // PassFactory wrappers whose passes are created and dropped inside process; none of them
    // is a result producer, so the walk finds nothing below it.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: PhaseOptimizer#addOneTimePass
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "addOneTimePass" {
            let [DslValue::PassFactory { native, .. }] = args.as_slice() else {
                return Err(bad());
            };
            self.0.add_one_time_pass(native.clone());
            Ok(DslValue::Null)
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: PhaseOptimizer#process
    fn process_with_ctx(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
        ctx: &mut Ctx,
    ) -> Result<(), Throwable> {
        crate::replay::native_factories::with_context(ctx, || {
            crate::replay::native_unported::capture(|| {
                crate::jscomp_api::CompilerPass::process(&mut self.0, compiler, externs, root);
            })
        })
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: PhaseOptimizer#PhaseOptimizer
fn native_phase_optimizer(
    compiler: &crate::jscomp_api::Compiler,
    args: &[DslValue],
) -> Result<DslValue, Throwable> {
    if args.len() != 2 || !matches!(args[1], DslValue::Null) {
        return Err(Throwable::Unported(
            "com.google.javascript.jscomp.PerformanceTracker (DSL adapter)".into(),
        ));
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativePhaseOptimizer(closure_jscomp::phase_optimizer::PhaseOptimizer::new(
            compiler, None,
        )),
    ))))
}
// port: PhaseOptimizer#PhaseOptimizer
fn phase_optimizer(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    native_phase_optimizer(&compiler(&args)?.borrow(), &args)
}
// port: PhaseOptimizer#addOneTimePass
fn add_one_time_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(optimizer), factory] = args.as_slice() else {
        return Err(bad());
    };
    optimizer
        .borrow_mut()
        .call("addOneTimePass", vec![factory.clone()])
}

// port: RescopeGlobalSymbols#RescopeGlobalSymbols(AbstractCompiler,String,boolean,boolean,OptimizeLocalAccess)
fn rescope_global_symbols(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(_),
        DslValue::String(namespace),
        add_extern,
        assume_cross_chunk_names,
        optimize_local_access,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let pass = closure_jscomp::rescope_global_symbols::RescopeGlobalSymbols::new_with_add_extern(
        namespace.clone(),
        bool::decode_value(add_extern)?,
        bool::decode_value(assume_cross_chunk_names)?,
        closure_jscomp::compiler_options::OptimizeLocalAccess::decode_value(optimize_local_access)?,
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: ConvertChunksToESModules#ConvertChunksToESModules
fn convert_chunks_to_es_modules(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let pass = closure_jscomp::convert_chunks_to_es_modules::ConvertChunksToESModules::new();
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: RewriteGlobalDeclarationsForTryCatchWrapping#RewriteGlobalDeclarationsForTryCatchWrapping
fn rewrite_global_declarations_for_try_catch_wrapping(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let pass = closure_jscomp::rewrite_global_declarations_for_try_catch_wrapping::RewriteGlobalDeclarationsForTryCatchWrapping::new();
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
/// Helper holders the DSL instantiates through their declared no-argument constructor
/// (`ReplayValues#instantiate`); the recorded signatures list only their inner classes.
pub const HELPER_HOLDERS: [(&str, Entry); 5] = [
    (
        "com.google.javascript.jscomp.ProcessClosureProvidesAndRequiresTest_Helpers#<init>()",
        crate::replay::process_closure_provides_and_requires_test_helpers::holder,
    ),
    (
        "com.google.javascript.jscomp.ManageClosureUnawareCodeTest_Helpers#<init>()",
        crate::replay::manage_closure_unaware_code_test_helpers::holder,
    ),
    (
        "com.google.javascript.jscomp.ClosureUnawarePhaseOptimizerTest_Helpers#<init>()",
        crate::replay::closure_unaware_phase_optimizer_test_helpers::holder,
    ),
    (
        "com.google.javascript.jscomp.MultiPassTest_Helpers#<init>()",
        crate::replay::multi_pass_test_helpers::holder,
    ),
    (
        "com.google.javascript.jscomp.Es6RewriteGeneratorsTest_Helpers#<init>()",
        crate::replay::es6_rewrite_generators_test_helpers::holder,
    ),
];
// port: ReplayDsl#invoke (a CompilerPass constructed by the processor expression)
fn pass(
    args: &[DslValue],
    pass: Box<dyn closure_jscomp::compiler_pass::CompilerPass>,
) -> Result<DslValue, Throwable> {
    compiler(args)?;
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
// port: J2clSourceFileChecker#J2clSourceFileChecker
fn j2cl_source_file_checker(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    pass(
        &args,
        Box::new(closure_jscomp::j2cl_source_file_checker::J2clSourceFileChecker::new()),
    )
}
// port: J2clSourceFileChecker#markToRunJ2clPasses
fn mark_to_run_j2cl_passes(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    closure_jscomp::j2cl_source_file_checker::J2clSourceFileChecker::mark_to_run_j2cl_passes(
        &mut compiler(&args)?.borrow_mut(),
    );
    Ok(DslValue::Null)
}
// port: J2clChecksPass#J2clChecksPass
fn j2cl_checks_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    pass(
        &args,
        Box::new(closure_jscomp::j2cl_checks_pass::J2clChecksPass::new()),
    )
}
// port: RemoveCastNodes#RemoveCastNodes
fn remove_cast_nodes(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    pass(
        &args,
        Box::new(closure_jscomp::remove_cast_nodes::RemoveCastNodes::new()),
    )
}
// port: RemoveWeakSources#RemoveWeakSources
fn remove_weak_sources(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    pass(
        &args,
        Box::new(closure_jscomp::remove_weak_sources::RemoveWeakSources::new()),
    )
}

// port: ConcretizeStaticInheritanceForInlining#ConcretizeStaticInheritanceForInlining
fn concretize_static_inheritance_for_inlining(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let compiler = compiler(&args)?;
    let pass = closure_jscomp::concretize_static_inheritance_for_inlining::ConcretizeStaticInheritanceForInlining::new(
        &compiler.borrow(),
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
