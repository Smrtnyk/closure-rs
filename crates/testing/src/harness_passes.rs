/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/AccessorSummary.java,
//   src/com/google/javascript/jscomp/ChangeVerifier.java,
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/DiagnosticGroups.java,
//   src/com/google/javascript/jscomp/GatherGetterAndSetterProperties.java,
//   src/com/google/javascript/jscomp/GatherModuleMetadata.java,
//   src/com/google/javascript/jscomp/GoogleCodingConvention.java,
//   src/com/google/javascript/jscomp/RemoveCastNodes.java,
//   src/com/google/javascript/jscomp/SourceInformationAnnotator.java,
//   src/com/google/javascript/jscomp/SourceMap.java,
//   src/com/google/javascript/jscomp/TypeCheck.java,
//   src/com/google/javascript/jscomp/TypeInferencePass.java,
//   src/com/google/javascript/jscomp/ValidityCheck.java,
//   src/com/google/javascript/jscomp/js/RuntimeJsLibManager.java,
//   src/com/google/javascript/jscomp/parsing/parser/FeatureSet.java,
//   src/com/google/javascript/jscomp/serialization/ConvertTypesToColors.java,
//   src/com/google/javascript/jscomp/testing/TestExternsBuilder.java,
//   src/com/google/javascript/jscomp/type/SemanticReverseAbstractInterpreter.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/CompilerTestCaseUtils.java,
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java,
//   test/com/google/javascript/jscomp/TypeCheckTestCase.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replacement seams for the compiler passes run by the Java harness itself.
//! Each body is replaced by the real port when that Java class is ported.
use crate::{
    jscomp_api::{Compiler, CompilerOptions, JSChunk, SourceFile},
    replay::replay_dsl::DslValue,
    throwable::Throwable,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::sync::Arc;

// The JSTypeRegistry, JSTypeNative, JSTypeExpression, RecordTypeBuilder and BaseJSTypeTestCase
// calls of the harness, which name Closure's Rhino-derived classes (MPL-1.1 /
// GPL-2.0-or-later), are in their own file.
#[path = "harness_passes_rhino.rs"]
mod rhino;
pub use rhino::{
    add_native_properties, assert_types_equal, evaluate_type_expression, js_type_expression,
    record_type_builder,
};

// port: ChangeVerifier#snapshot
pub fn change_verifier_snapshot(
    compiler: &mut Compiler,
    root: NodeId,
) -> Result<closure_jscomp::change_verifier::ChangeVerifier, Throwable> {
    Ok(closure_jscomp::change_verifier::ChangeVerifier::new(compiler).snapshot(compiler, root))
}
// port: ChangeVerifier#checkRecordedChanges
pub fn check_recorded_changes(
    compiler: &mut Compiler,
    root: NodeId,
    snapshot: &closure_jscomp::change_verifier::ChangeVerifier,
) -> Result<(), Throwable> {
    snapshot.check_recorded_changes(compiler, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new ProcessCommonJSModules(compiler).process(externsRoot, mainRoot))
pub fn process_common_js_modules(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    closure_jscomp::compiler_pass::CompilerPass::process(
        &mut closure_jscomp::process_common_js_modules::ProcessCommonJSModules::new(),
        compiler,
        externs,
        root,
    );
    Ok(())
}

// port: GatherModuleMetadata#process
pub fn gather_module_metadata(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    process_common_js_modules: bool,
    resolution_mode: &str,
) -> Result<(), Throwable> {
    let resolution_mode =
        closure_jscomp::deps::module_loader::ResolutionMode::value_of(resolution_mode)
            .ok_or_else(|| Throwable::HarnessError("unknown ResolutionMode".into()))?;
    closure_jscomp::compiler_pass::CompilerPass::process(
        &mut closure_jscomp::gather_module_metadata::GatherModuleMetadata::new(
            process_common_js_modules,
            resolution_mode,
        ),
        compiler,
        externs,
        root,
    );
    Ok(())
}

// port: CompilerTestCase#rewriteEsModules
pub fn rewrite_es_modules(
    compiler: &mut Compiler,
    externs_root: NodeId,
    code_root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::{
        gather_module_metadata::GatherModuleMetadata,
        modules::module_map_creator::ModuleMapCreator, pass_factory::PassFactory,
        pass_list_builder::PassListBuilder, pass_names,
        preprocessor_symbol_table::CachedInstanceFactory,
        transpilation_passes::TranspilationPasses,
    };
    let options = compiler.get_options().clone();
    let mut factories = PassListBuilder::new(options.clone());

    let process_common_js_modules = options.get_process_common_js_modules();
    let module_resolution_mode = options.get_module_resolution_mode();
    factories.maybe_add(
        PassFactory::builder()
            .set_name(pass_names::GATHER_MODULE_METADATA)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(move |_x1| {
                Box::new(GatherModuleMetadata::new(
                    process_common_js_modules,
                    module_resolution_mode,
                ))
            }))
            .build(),
    );
    factories.maybe_add(
        PassFactory::builder()
            .set_name(pass_names::CREATE_MODULE_MAP)
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|x| {
                Box::new(ModuleMapCreator::new(closure_rhino::check_not_null!(
                    x.get_module_metadata_map().cloned()
                )))
            }))
            .build(),
    );
    TranspilationPasses::add_es6_module_pass(&mut factories, &CachedInstanceFactory::default());
    for factory in factories.build_including_unported() {
        if let Some(class) = factory.get_unported_class() {
            return Err(Throwable::Unported(format!(
                "com.google.javascript.jscomp.{class}"
            )));
        }
        factory
            .create(compiler)
            .process(compiler, externs_root, code_root);
    }
    Ok(())
}

// port: CompilerTestCase#VAR_CHECK_EXTERNS (VarCheck.REQUIRED_SYMBOLS)
pub fn var_check_required_symbols() -> Result<Vec<String>, Throwable> {
    Ok(closure_jscomp::var_check::REQUIRED_SYMBOLS
        .iter()
        .map(|symbol| symbol.to_string())
        .collect())
}

// port: CompilerTestCase#testInternal (new ModuleMapCreator(compiler, compiler.getModuleMetadataMap()).process(externsRoot, mainRoot))
pub fn module_map_creator(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    let module_metadata_map = compiler
        .get_module_metadata_map()
        .cloned()
        .ok_or_else(|| Throwable::HarnessError("module metadata map not set".into()))?;
    closure_jscomp::compiler_pass::CompilerPass::process(
        &mut closure_jscomp::modules::module_map_creator::ModuleMapCreator::new(
            module_metadata_map,
        ),
        compiler,
        externs,
        root,
    );
    Ok(())
}
// port: CompilerTestCase#normalizeActualCode (Normalize.builder(compiler).assertOnChange(..).build().process;
// assertOnChange false is Normalize#createNormalizeForOptimizations)
pub fn normalize(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    assert_on_change: bool,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::normalize::Normalize::builder(compiler)
        .assert_on_change(assert_on_change)
        .build()
        .process(compiler, externs, root);
    Ok(())
}
// port: ValidityCheck.VerifyConstants#process (new ValidityCheck.VerifyConstants(compiler, verifyDeclaredConstants))
pub fn verify_constants(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    verify_declared_constants: bool,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::validity_check::VerifyConstants::new(compiler, verify_declared_constants)
        .process(compiler, externs, root);
    Ok(())
}
// port: CompilerTestCase#transpileToEs5
pub fn transpile_to_es5(
    compiler: &mut Compiler,
    externs_root: NodeId,
    code_root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::{
        compiler_options::LanguageMode, pass_factory::PassFactory,
        pass_list_builder::PassListBuilder, transpilation_passes::TranspilationPasses,
    };
    // Java's PassListBuilder keeps a reference to these options, so the language modes set below
    // are what its conditions read; the Rust builder takes a copy made after setting them.
    let options = compiler.get_options_mut();
    options.set_language_in(LanguageMode::UNSUPPORTED);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    let options = compiler.get_options().clone();
    let mut factories = PassListBuilder::new(options.clone());

    TranspilationPasses::add_transpilation_runtime_libraries(&mut factories);
    TranspilationPasses::add_rewrite_polyfill_pass(&mut factories);
    // Transpilation requires normalization.
    factories.maybe_add(
        PassFactory::builder()
            .set_name(closure_jscomp::pass_names::NORMALIZE)
            .set_internal_factory(Arc::new(|abstract_compiler| {
                Box::new(closure_jscomp::normalize::Normalize::builder(abstract_compiler).build())
            }))
            .build(),
    );
    TranspilationPasses::add_transpilation_passes(&mut factories, &options);
    // We need to put back the original variable names where possible once transpilation is
    // complete. This matches the behavior in DefaultPassConfig. See comments there for further
    // explanation.
    factories.maybe_add(
        PassFactory::builder()
            .set_name("invertContextualRenaming")
            .set_internal_factory(Arc::new(
                closure_jscomp::make_declared_names_unique::MakeDeclaredNamesUnique::get_contextual_rename_inverter,
            ))
            .build(),
    );
    // Java's factories.build() includes every factory; an unported one panics on create and is
    // reported as unported.
    crate::replay::native_unported::capture(|| {
        for factory in factories.build_including_unported() {
            factory
                .create(compiler)
                .process(compiler, externs_root, code_root);
        }
    })
}

// port: RemoveCastNodes#process
pub fn remove_cast_nodes(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::remove_cast_nodes::RemoveCastNodes::new().process(compiler, externs, root);
    Ok(())
}

// port: ConvertTypesToColors#process (CompilerTestCase#testInternal: new ConvertTypesToColors(compiler, SerializationOptions.builder().setIncludeDebugInfo(..).build()))
pub fn convert_types_to_colors(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    include_debug_info: bool,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    use closure_jscomp::serialization::convert_types_to_colors::ConvertTypesToColors;
    use closure_jscomp::serialization::serialization_options::SerializationOptions;
    ConvertTypesToColors::new(
        SerializationOptions::builder()
            .set_include_debug_info(include_debug_info)
            .build(),
    )
    .process(compiler, externs, root);
    Ok(())
}

/// `throw new RuntimeException(e)` around an IOException of saveState/restoreState.
fn runtime_exception(e: std::io::Error) -> Throwable {
    Throwable::Exception {
        class: "java.lang.RuntimeException".into(),
        message: Some(format!("java.io.IOException: {e}")),
    }
}

// port: Compiler#saveState (CompilerTestCaseUtils#multistageSerializeAndDeserialize)
pub fn save_state(compiler: &mut Compiler, baos: &mut Vec<u8>) -> Result<(), Throwable> {
    compiler.save_state(baos).map_err(runtime_exception)
}

// port: Compiler#restoreState (CompilerTestCaseUtils#multistageSerializeAndDeserialize)
pub fn restore_state(compiler: &mut Compiler, bytes: &[u8]) -> Result<(), Throwable> {
    let mut bais = std::io::Cursor::new(bytes.to_vec());
    compiler.restore_state(&mut bais).map_err(runtime_exception)
}

// port: CompilerTestCase#testInternal (new InferConsts(compiler).process(externsRoot, mainRoot))
pub fn infer_consts(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::infer_consts::InferConsts::new(compiler).process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new SourceInfoCheck(compiler).process(externsRoot, mainRoot))
pub fn source_info_check(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::source_info_check::SourceInfoCheck::new(compiler)
        .process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new AstValidator(compiler, true).setTypeValidationMode(mode).validateRoot(root))
// port: TypeCheckTestCase#parseAndTypeCheckWithScope (new AstValidator(compiler).setTypeValidationMode(JSTYPE).process(externsNode, jsNode))
pub fn ast_validator(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    validate_script_features: bool,
    type_validation_mode: &str,
) -> Result<(), Throwable> {
    use closure_jscomp::ast_validator::{AstValidator, TypeInfoValidation};
    use closure_jscomp::compiler_pass::CompilerPass;
    let mode = match type_validation_mode {
        "JSTYPE" => TypeInfoValidation::JSTYPE,
        "COLOR" => TypeInfoValidation::COLOR,
        "NONE" => TypeInfoValidation::NONE,
        other => {
            return Err(Throwable::HarnessError(format!(
                "unknown TypeInfoValidation {other}"
            )));
        }
    };
    if validate_script_features {
        // CompilerTestCase validates the whole ROOT with script feature validation.
        AstValidator::new_with_script_features(compiler, true)
            .set_type_validation_mode(mode)
            .validate_root(compiler, root);
    } else {
        // TypeCheckTestCase validates externs and code with the one-argument constructor.
        AstValidator::new(compiler)
            .set_type_validation_mode(mode)
            .process(compiler, externs, root);
    }
    Ok(())
}

// port: CompilerTestCase#testInternal (new GatherExternProperties(compiler, mode).process(externsRoot, mainRoot))
pub fn gather_extern_properties(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    mode: &str,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    use closure_jscomp::gather_extern_properties::{GatherExternProperties, Mode};
    let mode = match mode {
        "CHECK" => Mode::CHECK,
        "OPTIMIZE" => Mode::OPTIMIZE,
        "CHECK_AND_OPTIMIZE" => Mode::CHECK_AND_OPTIMIZE,
        other => {
            return Err(Throwable::HarnessError(format!(
                "unknown GatherExternProperties.Mode {other}"
            )));
        }
    };
    GatherExternProperties::new(compiler, mode).process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (computeSideEffects: PureFunctionIdentifier.Driver)
pub fn pure_function_identifier(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    let mut mark = closure_jscomp::pure_function_identifier::Driver::new();
    mark.process(compiler, externs, root);
    Ok(())
}

/// The recorded class of `AccessorSummary.PropertyAccessKind` values.
const PROPERTY_ACCESS_KIND_CLASS: &str =
    "com.google.javascript.jscomp.AccessorSummary$PropertyAccessKind";

/// `PropertyAccessKind` as the harness's DSL value (the recorded enum encoding).
fn property_access_kind_value(
    kind: closure_jscomp::accessor_summary::PropertyAccessKind,
) -> DslValue {
    DslValue::Enum {
        class: PROPERTY_ACCESS_KIND_CLASS.into(),
        name: kind.to_string(),
    }
}

// port: PropertyAccessKind#valueOf
fn property_access_kind_of(
    value: &DslValue,
) -> Result<closure_jscomp::accessor_summary::PropertyAccessKind, Throwable> {
    use closure_jscomp::accessor_summary::PropertyAccessKind;
    let DslValue::Enum { class, name } = value else {
        return Err(Throwable::HarnessError(format!(
            "not a PropertyAccessKind: {}",
            value.class_name()
        )));
    };
    if !class.ends_with("PropertyAccessKind") {
        return Err(Throwable::HarnessError(format!(
            "not a PropertyAccessKind: {class}"
        )));
    }
    match name.as_str() {
        "NORMAL" => Ok(PropertyAccessKind::NORMAL),
        "GETTER_ONLY" => Ok(PropertyAccessKind::GETTER_ONLY),
        "SETTER_ONLY" => Ok(PropertyAccessKind::SETTER_ONLY),
        "GETTER_AND_SETTER" => Ok(PropertyAccessKind::GETTER_AND_SETTER),
        _ => Err(Throwable::HarnessError(format!(
            "unknown PropertyAccessKind {name}"
        ))),
    }
}

// port: GatherGetterAndSetterProperties#gather
pub fn gather_getter_and_setter_properties(
    compiler: &mut Compiler,
    root: NodeId,
) -> Result<IndexMap<String, DslValue>, Throwable> {
    use closure_jscomp::gather_getter_and_setter_properties::GatherGetterAndSetterProperties;
    Ok(GatherGetterAndSetterProperties::gather(compiler, root)
        .into_iter()
        .map(|(name, kind)| (name.to_string(), property_access_kind_value(kind)))
        .collect())
}

// port: AccessorSummary#create (+ Compiler#setAccessorSummary)
pub fn set_accessor_summary(
    compiler: &mut Compiler,
    summary: IndexMap<String, DslValue>,
) -> Result<(), Throwable> {
    let accessors = summary
        .iter()
        .map(|(name, kind)| {
            Ok((
                closure_rhino::js_string::JsString::from(name.as_str()),
                property_access_kind_of(kind)?,
            ))
        })
        .collect::<Result<_, Throwable>>()?;
    compiler.set_accessor_summary(Arc::new(
        closure_jscomp::accessor_summary::AccessorSummary::create(accessors),
    ));
    Ok(())
}

// port: AccessorSummary#getAccessors (of Compiler#getAccessorSummary; None when it is null)
pub fn accessor_summary(
    compiler: &Compiler,
) -> Result<Option<IndexMap<String, DslValue>>, Throwable> {
    Ok(compiler.get_accessor_summary().map(|summary| {
        summary
            .get_accessors()
            .iter()
            .map(|(name, kind)| (name.to_string(), property_access_kind_value(*kind)))
            .collect()
    }))
}

// port: RuntimeJsLibManager#ensureLibraryInjected
pub fn ensure_library_injected(
    compiler: &mut Compiler,
    resource_name: &str,
    force: bool,
) -> Result<(), Throwable> {
    let manager = compiler.get_runtime_js_lib_manager();
    manager
        .lock()
        .unwrap()
        .ensure_library_injected(compiler, resource_name, force);
    Ok(())
}

// port: RuntimeJsLibManager#getInjectedLibraries (NullPointerException before Compiler#init)
pub fn injected_libraries(compiler: &Compiler) -> Result<Vec<String>, Throwable> {
    match compiler.get_runtime_js_lib_manager_or_null() {
        Some(manager) => Ok(manager.lock().unwrap().get_injected_libraries()),
        None => Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        }),
    }
}

// port: Compiler#toSource(Node)
pub fn to_source(
    compiler: &mut Compiler,
    root: NodeId,
) -> Result<closure_rhino::js_string::JsString, Throwable> {
    Ok(compiler.to_source_for_node_utf16(root))
}
// port: Compiler#toSource()
pub fn to_source_all(
    compiler: &mut Compiler,
) -> Result<closure_rhino::js_string::JsString, Throwable> {
    Ok(compiler.to_source())
}
// port: Compiler#compileChunks
pub fn default_pass_config(
    compiler: &mut Compiler,
    externs: &[Arc<SourceFile>],
    chunks: &[JSChunk],
    options: CompilerOptions,
) -> Result<(), Throwable> {
    // Observe on the CompilerExecutor's actual thread. Nested compiler operations use the
    // same thread, so the observer remains scoped to this compilation even in parallel tests.
    compiler.run_in_compiler_thread(|compiler| {
        crate::replay::native_unported::capture(|| {
            compiler.compile_chunks(externs, chunks.to_vec(), options);
        })
    })
}
// port: CompilerTestCase#testExternChanges (Compiler.toSource across arenas)
pub fn to_source_across(
    compiler: &Compiler,
    ast: &closure_rhino::node::Ast,
    root: NodeId,
) -> closure_rhino::js_string::JsString {
    use closure_jscomp::code_printer::LicenseTracker;
    let mut tracker =
        closure_jscomp::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(compiler);
    let code = closure_jscomp::code_printer::Builder::new(root)
        .set_compiler_options(compiler.get_options())
        .set_tag_as_type_summary(compiler.get_options().should_generate_typed_externs())
        .set_tag_as_strict(false)
        .set_license_tracker(Some(&mut tracker))
        .build(ast);
    let mut output = closure_jscomp::compiler::CodeBuilder::default();
    for license in tracker.emit_licenses() {
        output.append("/*\n").append(license).append("*/\n");
    }
    output.append(code);
    output.to_js_string()
}
// port: DiagnosticGroups#getRegisteredGroups
pub fn diagnostic_group(name: &str) -> Result<Arc<crate::jscomp_api::DiagnosticGroup>, Throwable> {
    closure_jscomp::diagnostic_groups::DiagnosticGroups::for_name(name)
        .or_else(|| match name {
            "UNTRANSPILABLE_FEATURES" => {
                Some(closure_jscomp::diagnostic_groups::UNTRANSPILABLE_FEATURES.clone())
            }
            "MODULE_LOAD" => Some(closure_jscomp::diagnostic_groups::MODULE_LOAD.clone()),
            "MODULE_IMPORT" => Some(closure_jscomp::diagnostic_groups::MODULE_IMPORT.clone()),
            "GLOBAL_THIS" => Some(closure_jscomp::diagnostic_groups::GLOBAL_THIS.clone()),
            "DEPRECATED" => Some(closure_jscomp::diagnostic_groups::DEPRECATED.clone()),
            "UNDERSCORE" => Some(closure_jscomp::diagnostic_groups::UNDERSCORE.clone()),
            "VISIBILITY" => Some(closure_jscomp::diagnostic_groups::VISIBILITY.clone()),
            "ACCESS_CONTROLS" => Some(closure_jscomp::diagnostic_groups::ACCESS_CONTROLS.clone()),
            "NON_STANDARD_JSDOC" => {
                Some(closure_jscomp::diagnostic_groups::NON_STANDARD_JSDOC.clone())
            }
            "INVALID_CASTS" => Some(closure_jscomp::diagnostic_groups::INVALID_CASTS.clone()),
            "STRICT_MODULE_DEP_CHECK" => {
                Some(closure_jscomp::diagnostic_groups::STRICT_MODULE_DEP_CHECK.clone())
            }
            "VIOLATED_MODULE_DEP" => {
                Some(closure_jscomp::diagnostic_groups::VIOLATED_MODULE_DEP.clone())
            }
            "EXTERNS_VALIDATION" => {
                Some(closure_jscomp::diagnostic_groups::EXTERNS_VALIDATION.clone())
            }
            "UNKNOWN_DEFINES" => Some(closure_jscomp::diagnostic_groups::UNKNOWN_DEFINES.clone()),
            "DEFINE_WITHOUT_GOOG_DEFINE" => {
                Some(closure_jscomp::diagnostic_groups::DEFINE_WITHOUT_GOOG_DEFINE.clone())
            }
            "TWEAKS" => Some(closure_jscomp::diagnostic_groups::TWEAKS.clone()),
            "MISSING_OVERRIDE" => Some(closure_jscomp::diagnostic_groups::MISSING_OVERRIDE.clone()),
            "MISSING_PROPERTIES" => {
                Some(closure_jscomp::diagnostic_groups::MISSING_PROPERTIES.clone())
            }
            "GLOBALLY_MISSING_PROPERTIES" => {
                Some(closure_jscomp::diagnostic_groups::GLOBALLY_MISSING_PROPERTIES.clone())
            }
            "J2CL_CHECKS" => Some(closure_jscomp::diagnostic_groups::J2CL_CHECKS.clone()),
            "MISSING_RETURN" => Some(closure_jscomp::diagnostic_groups::MISSING_RETURN.clone()),
            "UNDEFINED_VARIABLES" => {
                Some(closure_jscomp::diagnostic_groups::UNDEFINED_VARIABLES.clone())
            }
            "DEBUGGER_STATEMENT_PRESENT" => {
                Some(closure_jscomp::diagnostic_groups::DEBUGGER_STATEMENT_PRESENT.clone())
            }
            "CHECK_REGEXP" => Some(closure_jscomp::diagnostic_groups::CHECK_REGEXP.clone()),
            "CHECK_TYPES" => Some(closure_jscomp::diagnostic_groups::CHECK_TYPES.clone()),
            "ES5_INHERITANCE_DIAGNOSTIC_GROUP" => {
                Some(closure_jscomp::diagnostic_groups::ES5_INHERITANCE_DIAGNOSTIC_GROUP.clone())
            }
            "CHECK_PROTOTYPAL_TYPES" => {
                Some(closure_jscomp::diagnostic_groups::CHECK_PROTOTYPAL_TYPES.clone())
            }
            "CHECK_STATIC_OVERRIDES" => {
                Some(closure_jscomp::diagnostic_groups::CHECK_STATIC_OVERRIDES.clone())
            }
            "TOO_MANY_TYPE_PARAMS" => {
                Some(closure_jscomp::diagnostic_groups::TOO_MANY_TYPE_PARAMS.clone())
            }
            "STRICT_MISSING_PROPERTIES" => {
                Some(closure_jscomp::diagnostic_groups::STRICT_MISSING_PROPERTIES.clone())
            }
            "STRICT_PRIMITIVE_OPERATORS" => {
                Some(closure_jscomp::diagnostic_groups::STRICT_PRIMITIVE_OPERATORS.clone())
            }
            "STRICT_CHECK_TYPES" => {
                Some(closure_jscomp::diagnostic_groups::STRICT_CHECK_TYPES.clone())
            }
            "REPORT_UNKNOWN_TYPES" => {
                Some(closure_jscomp::diagnostic_groups::REPORT_UNKNOWN_TYPES.clone())
            }
            "CHECK_VARIABLES" => Some(closure_jscomp::diagnostic_groups::CHECK_VARIABLES.clone()),
            "CHECK_USELESS_CODE" => {
                Some(closure_jscomp::diagnostic_groups::CHECK_USELESS_CODE.clone())
            }
            "CONST" => Some(closure_jscomp::diagnostic_groups::CONST.clone()),
            "CONSTANT_PROPERTY" => {
                Some(closure_jscomp::diagnostic_groups::CONSTANT_PROPERTY.clone())
            }
            "ACCESS_CONTROLS_CONST" => {
                Some(closure_jscomp::diagnostic_groups::ACCESS_CONTROLS_CONST.clone())
            }
            "TYPE_INVALIDATION" => {
                Some(closure_jscomp::diagnostic_groups::TYPE_INVALIDATION.clone())
            }
            "DUPLICATE_VARS" => Some(closure_jscomp::diagnostic_groups::DUPLICATE_VARS.clone()),
            "ES5_STRICT" => Some(closure_jscomp::diagnostic_groups::ES5_STRICT.clone()),
            "WEAK_MODULE_GET" => Some(closure_jscomp::diagnostic_groups::WEAK_MODULE_GET.clone()),
            "MISSING_PROVIDE" => Some(closure_jscomp::diagnostic_groups::MISSING_PROVIDE.clone()),
            "UNRECOGNIZED_TYPE_ERROR" => {
                Some(closure_jscomp::diagnostic_groups::UNRECOGNIZED_TYPE_ERROR.clone())
            }
            "DANGEROUS_UNRECOGNIZED_TYPE_ERROR" => {
                Some(closure_jscomp::diagnostic_groups::DANGEROUS_UNRECOGNIZED_TYPE_ERROR.clone())
            }
            "MISSING_REQUIRE" => Some(closure_jscomp::diagnostic_groups::MISSING_REQUIRE.clone()),
            "MISSING_SOURCES_WARNINGS" => {
                Some(closure_jscomp::diagnostic_groups::MISSING_SOURCES_WARNINGS.clone())
            }
            "EXTRA_REQUIRE" => Some(closure_jscomp::diagnostic_groups::EXTRA_REQUIRE.clone()),
            "DUPLICATE_MESSAGE" => {
                Some(closure_jscomp::diagnostic_groups::DUPLICATE_MESSAGE.clone())
            }
            "MESSAGE_DESCRIPTIONS" => {
                Some(closure_jscomp::diagnostic_groups::MESSAGE_DESCRIPTIONS.clone())
            }
            "MSG_CONVENTIONS" => Some(closure_jscomp::diagnostic_groups::MSG_CONVENTIONS.clone()),
            "MISPLACED_TYPE_ANNOTATION" => {
                Some(closure_jscomp::diagnostic_groups::MISPLACED_TYPE_ANNOTATION.clone())
            }
            "MISPLACED_MSG_ANNOTATION" => {
                Some(closure_jscomp::diagnostic_groups::MISPLACED_MSG_ANNOTATION.clone())
            }
            "MISPLACED_SUPPRESS" => {
                Some(closure_jscomp::diagnostic_groups::MISPLACED_SUPPRESS.clone())
            }
            "SUSPICIOUS_CODE" => Some(closure_jscomp::diagnostic_groups::SUSPICIOUS_CODE.clone()),
            "FUNCTION_PARAMS" => Some(closure_jscomp::diagnostic_groups::FUNCTION_PARAMS.clone()),
            "DEPRECATED_ANNOTATIONS" => {
                Some(closure_jscomp::diagnostic_groups::DEPRECATED_ANNOTATIONS.clone())
            }
            "UNUSED_LOCAL_VARIABLE" => {
                Some(closure_jscomp::diagnostic_groups::UNUSED_LOCAL_VARIABLE.clone())
            }
            "JSDOC_MISSING_TYPE" => {
                Some(closure_jscomp::diagnostic_groups::JSDOC_MISSING_TYPE.clone())
            }
            "TYPE_IMPORT_CODE_REFERENCES" => {
                Some(closure_jscomp::diagnostic_groups::TYPE_IMPORT_CODE_REFERENCES.clone())
            }
            "PARTIAL_ALIAS" => Some(closure_jscomp::diagnostic_groups::PARTIAL_ALIAS.clone()),
            "USE_OF_GOOG_PROVIDE" => {
                Some(closure_jscomp::diagnostic_groups::USE_OF_GOOG_PROVIDE.clone())
            }
            "LINT_VAR_DECLARATIONS" => {
                Some(closure_jscomp::diagnostic_groups::LINT_VAR_DECLARATIONS.clone())
            }
            "LINT_CHECKS" => Some(closure_jscomp::diagnostic_groups::LINT_CHECKS.clone()),
            "STRICT_MODULE_CHECKS" => {
                Some(closure_jscomp::diagnostic_groups::STRICT_MODULE_CHECKS.clone())
            }
            "ANALYZER_CHECKS" => Some(closure_jscomp::diagnostic_groups::ANALYZER_CHECKS.clone()),
            "CLOSURE_DEP_METHOD_USAGE_CHECKS" => {
                Some(closure_jscomp::diagnostic_groups::CLOSURE_DEP_METHOD_USAGE_CHECKS.clone())
            }
            "CLOSURE_CLASS_CHECKS" => {
                Some(closure_jscomp::diagnostic_groups::CLOSURE_CLASS_CHECKS.clone())
            }
            "MALFORMED_GOOG_MODULE" => {
                Some(closure_jscomp::diagnostic_groups::MALFORMED_GOOG_MODULE.clone())
            }
            "CONFORMANCE_VIOLATIONS" => {
                Some(closure_jscomp::diagnostic_groups::CONFORMANCE_VIOLATIONS.clone())
            }
            "LATE_PROVIDE" => Some(closure_jscomp::diagnostic_groups::LATE_PROVIDE.clone()),
            "DUPLICATE_NAMESPACES" => {
                Some(closure_jscomp::diagnostic_groups::DUPLICATE_NAMESPACES.clone())
            }
            "INVALID_DEFINES" => Some(closure_jscomp::diagnostic_groups::INVALID_DEFINES.clone()),
            "MISSING_POLYFILL" => Some(closure_jscomp::diagnostic_groups::MISSING_POLYFILL.clone()),
            "POLYMER" => Some(closure_jscomp::diagnostic_groups::POLYMER.clone()),
            "BOUNDED_GENERICS" => Some(closure_jscomp::diagnostic_groups::BOUNDED_GENERICS.clone()),
            "PARSING" => Some(closure_jscomp::diagnostic_groups::PARSING.clone()),
            "CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT" => Some(
                closure_jscomp::diagnostic_groups::CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT.clone(),
            ),
            "ARTIFICIAL_FUNCTION_PURITY_VALIDATION" => Some(
                closure_jscomp::diagnostic_groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION.clone(),
            ),
            _ => None,
        })
        .ok_or_else(|| Throwable::HarnessError(format!("unknown DiagnosticGroups member {name}")))
}
// port: GoogleCodingConvention#GoogleCodingConvention
pub fn google_coding_convention() -> Result<DslValue, Throwable> {
    Ok(DslValue::CodingConvention(Arc::new(
        closure_jscomp::google_coding_convention::GoogleCodingConvention::new(),
    )))
}
// port: ReplayValues#decode (CodingConvention)
pub fn decode_coding_convention(
    value: &DslValue,
) -> Result<Arc<dyn closure_jscomp::coding_convention::CodingConvention + Send + Sync>, Throwable> {
    match value {
        DslValue::CodingConvention(convention) => Ok(convention.clone()),
        DslValue::Object(o) => {
            let o = o.borrow();
            match o.class.as_str() {
                "com.google.javascript.jscomp.GoogleCodingConvention" => {
                    let next = o.fields.get("Proxy.nextConvention").ok_or_else(|| {
                        Throwable::HarnessError(
                            "GoogleCodingConvention has no Proxy.nextConvention".into(),
                        )
                    })?;
                    Ok(Arc::new(closure_jscomp::google_coding_convention::GoogleCodingConvention::with_convention(decode_coding_convention(next)?)))
                }
                "com.google.javascript.jscomp.ClosureCodingConvention" => {
                    let next = o.fields.get("Proxy.nextConvention").ok_or_else(|| {
                        Throwable::HarnessError(
                            "ClosureCodingConvention has no Proxy.nextConvention".into(),
                        )
                    })?;
                    Ok(Arc::new(closure_jscomp::closure_coding_convention::ClosureCodingConvention::with_convention(decode_coding_convention(next)?)))
                }
                "com.google.javascript.jscomp.CodingConventions$DefaultCodingConvention" => Ok(
                    Arc::new(closure_jscomp::coding_conventions::DefaultCodingConvention),
                ),
                class => Err(Throwable::Unported(class.into())),
            }
        }
        _ => Err(Throwable::HarnessError("expected CodingConvention".into())),
    }
}
// port: TestExternsBuilder#TestExternsBuilder
pub fn test_externs_builder() -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(std::rc::Rc::new(std::cell::RefCell::new(
        crate::testing::test_externs_builder::TestExternsBuilder::new(),
    ))))
}

struct NativeTypeExpression(closure_rhino::js_type_expression::JSTypeExpression);
impl crate::replay::replay_dsl::NativeObject for NativeTypeExpression {
    // port: ReplayDsl#invoke (JSTypeExpression runtime class)
    fn class_name(&self) -> &str {
        "com.google.javascript.rhino.JSTypeExpression"
    }
    // port: UnitRecorder#fields (JSTypeExpression getters)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([
            ("root".into(), DslValue::Node(self.0.get_root())),
            (
                "sourceName".into(),
                DslValue::String(self.0.get_source_name().into()),
            ),
        ]))
    }
    // port: ReplayDsl#invoke (native receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: SourceMap#appendTo
pub fn source_map_append_to(
    source_map: &mut closure_jscomp::source_map::SourceMap,
    file: &str,
) -> Result<closure_rhino::js_string::JsString, Throwable> {
    let mut output = String::new();
    source_map
        .append_to(&mut output, file)
        .expect("String writer");
    Ok(output.into())
}
// port: FeatureSet#has (UnitRecorder enum-order projection)
pub fn feature_names(features: &DslValue) -> Result<Vec<String>, Throwable> {
    use crate::replay::options_values::OptionValue;
    Ok(
        closure_parsing::parser::feature_set::FeatureSet::decode_value(features)?
            .get_features()
            .iter()
            .map(|feature| format!("{feature:?}"))
            .collect(),
    )
}

// port: SourceInformationAnnotator#create (CompilerTestCase: annotateSourceInfo traversal of mainRoot)
pub fn source_information_annotator(
    compiler: &mut Compiler,
    _externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    closure_jscomp::node_traversal::NodeTraversal::traverse(
        compiler,
        root,
        &mut closure_jscomp::source_information_annotator::SourceInformationAnnotator::create(),
    );
    Ok(())
}

// port: SemanticReverseAbstractInterpreter#SemanticReverseAbstractInterpreter
/// CompilerTestCase#createTypeCheck's `new SemanticReverseAbstractInterpreter(compiler.getTypeRegistry())`.
pub fn semantic_reverse_abstract_interpreter(
    compiler: &mut Compiler,
    _externs: NodeId,
    _root: NodeId,
) -> Result<(), Throwable> {
    closure_jscomp::semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter::new(
        compiler.get_type_registry(),
    );
    Ok(())
}

// port: CompilerTestCase#testInternal (new ProcessClosurePrimitives(compiler).process(externsRoot, mainRoot))
pub fn process_closure_primitives(
    compiler: &mut crate::jscomp_api::Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::process_closure_primitives::ProcessClosurePrimitives::new(compiler)
        .process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new ProcessClosureProvidesAndRequires(compiler, false).process(externsRoot, mainRoot))
pub fn process_closure_provides_and_requires(
    compiler: &mut crate::jscomp_api::Compiler,
    externs: NodeId,
    root: NodeId,
    preserve_goog_provides_and_requires: bool,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::process_closure_provides_and_requires::ProcessClosureProvidesAndRequires::new(
        compiler,
        preserve_goog_provides_and_requires,
    )
    .process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new ClosureRewriteModule(compiler, null, null or
// compiler.getTopScope()).process(externsRoot, mainRoot))
pub fn closure_rewrite_module(
    compiler: &mut crate::jscomp_api::Compiler,
    externs: NodeId,
    root: NodeId,
    use_top_scope: bool,
) -> Result<(), Throwable> {
    let global_typed_scope = if use_top_scope {
        compiler.get_top_scope()
    } else {
        None
    };
    closure_jscomp::closure_rewrite_module::ClosureRewriteModule::new(
        compiler,
        None,
        global_typed_scope,
    )
    .process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new CheckClosureImports(compiler, compiler.getModuleMetadataMap()).process(externsRoot, mainRoot))
pub fn check_closure_imports(
    compiler: &mut crate::jscomp_api::Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    // Java passes the nullable map; AbstractModuleCallback dereferences it.
    let module_metadata_map =
        closure_rhino::check_not_null!(compiler.get_module_metadata_map().cloned());
    closure_jscomp::check_closure_imports::CheckClosureImports::new(compiler, module_metadata_map)
        .process(compiler, externs, root);
    Ok(())
}

// port: CompilerTestCase#testInternal (ScopedAliases.builder(compiler).build().process(externsRoot, mainRoot))
pub fn scoped_aliases(
    compiler: &mut crate::jscomp_api::Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::scoped_aliases::ScopedAliases::builder()
        .build(compiler)
        .process(compiler, externs, root);
    Ok(())
}

// port: TypeInferencePass#inferAllScopes (new TypeInferencePass(compiler, compiler.getReverseAbstractInterpreter(), new TypedScopeCreator(compiler)).inferAllScopes(root.getParent()))
pub fn type_inference_pass(
    compiler: &mut crate::jscomp_api::Compiler,
    _externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    let scope_creator = closure_jscomp::typed_scope_creator::TypedScopeCreator::new(compiler);
    let reverse_interpreter = compiler.get_reverse_abstract_interpreter();
    let mut pass = closure_jscomp::type_inference_pass::TypeInferencePass::new(
        compiler,
        reverse_interpreter,
        scope_creator,
    );
    let global_root = root.get_parent(compiler).expect("NullPointerException");
    pass.infer_all_scopes(compiler, global_root);
    Ok(())
}

// port: CompilerTestCase#testInternal (new CheckAccessControls(compiler).process(externsRoot, mainRoot))
pub fn check_access_controls(
    compiler: &mut crate::jscomp_api::Compiler,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    use closure_jscomp::compiler_pass::CompilerPass;
    closure_jscomp::check_access_controls::CheckAccessControls::new(compiler)
        .process(compiler, externs, root);
    Ok(())
}

pub use crate::jscomp_api::polymer_pass;

/// `new SemanticReverseAbstractInterpreter(registry)` as a DSL value.
pub fn new_semantic_reverse_abstract_interpreter(
    registry: &DslValue,
) -> Result<DslValue, Throwable> {
    let compiler = registry_compiler(registry)?;
    let interpreter =
        closure_jscomp::semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter::new(
            borrow_compiler(&compiler)?.get_type_registry(),
        );
    Ok(DslValue::Native(std::rc::Rc::new(std::cell::RefCell::new(
        crate::replay::native_type_inference::NativeReverseAbstractInterpreter(interpreter),
    ))))
}

// port: CompilerTestCase#createTypeCheck (new TypeCheck(compiler, new SemanticReverseAbstractInterpreter(registry), registry); compiler.setTypeCheckingHasRun(true); check.processForTesting(externsRoot, mainRoot))
pub fn type_check(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    report_unknown_types: bool,
) -> Result<(), Throwable> {
    let interpreter =
        closure_jscomp::semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter::new(
            compiler.get_type_registry(),
        );
    let mut check = closure_jscomp::type_check::TypeCheck::new(compiler, interpreter)
        .report_unknown_types(report_unknown_types);
    compiler.set_type_checking_has_run(true);
    check.process_for_testing(compiler, Some(externs), root);
    Ok(())
}

// port: TypeCheck#processForTesting (the TypedScope it returns)
pub fn type_check_scope(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    report_unknown_types: bool,
) -> Result<closure_jscomp::typed_scope::TypedScope, Throwable> {
    type_check_typed_scope(compiler, externs, root, report_unknown_types)
}

// port: CompilerTestCase#validateWarnings (LightweightMessageFormatter with compiler excerpts)
pub fn format_warning(compiler: &Compiler, warning: &crate::jscomp_api::JSError) -> String {
    use crate::jscomp_api::{LightweightMessageFormatter, MessageFormatter, SourceExcerptProvider};
    // The formatter owns an Arc<dyn SourceExcerptProvider + 'static>. Capture its LINE request
    // through the real compiler getters in the same order: mapping first, then the mapped line.
    // No compiler or source formatting algorithm is duplicated here.
    let mapping = compiler.get_source_mapping(
        warning.source_name(),
        warning.get_line_number(),
        warning.charno(),
    );
    let mapped_name = mapping
        .as_ref()
        .map(|m| m.get_original_file().to_string_lossy());
    let name = mapped_name.as_deref().or(warning.source_name());
    let line = mapping
        .as_ref()
        .map_or(warning.get_line_number(), |m| m.get_line_number());
    let source = WarningLineExcerpt {
        line: name.and_then(|name| compiler.get_source_line(name, line)),
        mapping,
    };
    let source: Arc<dyn SourceExcerptProvider> = Arc::new(source);
    LightweightMessageFormatter::new(source).format_warning(compiler, warning)
}
struct WarningLineExcerpt {
    line: Option<String>,
    mapping: Option<crate::jscomp_api::OriginalMapping>,
}
impl crate::jscomp_api::SourceExcerptProvider for WarningLineExcerpt {
    // port: CompilerTestCase#validateWarnings (captured compiler LINE excerpt)
    fn get_source_line(&self, _name: Option<&str>, _line: i32) -> Option<String> {
        self.line.clone()
    }
    // port: CompilerTestCase#validateWarnings (LightweightMessageFormatter defaults to LINE)
    fn get_source_lines(
        &self,
        _name: Option<&str>,
        _line: i32,
        _length: i32,
    ) -> Option<Box<dyn crate::jscomp_api::Region>> {
        unreachable!("validateWarnings uses the LINE formatter")
    }
    // port: CompilerTestCase#validateWarnings (LightweightMessageFormatter defaults to LINE)
    fn get_source_region(
        &self,
        _name: Option<&str>,
        _line: i32,
    ) -> Option<Box<dyn crate::jscomp_api::Region>> {
        unreachable!("validateWarnings uses the LINE formatter")
    }
    // port: CompilerTestCase#validateWarnings (captured compiler source mapping)
    fn get_source_mapping(
        &self,
        _name: Option<&str>,
        _line: i32,
        _column: i32,
    ) -> Option<crate::jscomp_api::OriginalMapping> {
        self.mapping.clone()
    }
}

// ---- JSTypeRegistry (Compiler integration) ----
//
// The compiler owns its `JSTypeRegistry` together with the `Ast` the types' nodes live in, so the
// DSL values for the registry and its types keep the compiler and borrow it per call.

/// Borrows the compiler mutably for a registry call; a factory's live loan of the same compiler
/// is a harness error, not a panic.
fn borrow_compiler(
    compiler: &crate::replay::replay_dsl::CompilerHandle,
) -> Result<std::cell::RefMut<'_, Compiler>, Throwable> {
    compiler.try_borrow_mut().map_err(|_| {
        Throwable::HarnessError("JSTypeRegistry used while its Compiler is loaned out".into())
    })
}

// port: Compiler#getTypeRegistry (the registry and the Ast its types refer to)
pub fn with_type_registry<T>(
    compiler: &crate::replay::replay_dsl::CompilerHandle,
    action: impl FnOnce(
        &mut closure_jstype::JSTypeRegistry,
        &mut closure_rhino::node::Ast,
    ) -> Result<T, Throwable>,
) -> Result<T, Throwable> {
    let mut compiler = borrow_compiler(compiler)?;
    let (registry, ast) = compiler.get_type_registry_and_ast();
    action(registry, ast)
}

// port: Compiler#getTypeRegistry
pub fn compiler_type_registry(
    compiler: &crate::replay::replay_dsl::CompilerHandle,
) -> Result<DslValue, Throwable> {
    // Java creates the registry on first use.
    borrow_compiler(compiler)?.get_type_registry();
    Ok(native_type_registry(compiler.clone()))
}

/// `Compiler#getTypeRegistry()` reached through a factory's loan of the same compiler.
pub fn compiler_type_registry_borrowed(
    handle: &crate::replay::replay_dsl::CompilerHandle,
    compiler: &mut Compiler,
) -> DslValue {
    compiler.get_type_registry();
    native_type_registry(handle.clone())
}

fn native_type_registry(compiler: crate::replay::replay_dsl::CompilerHandle) -> DslValue {
    DslValue::Native(std::rc::Rc::new(std::cell::RefCell::new(
        NativeTypeRegistry(compiler),
    )))
}

/// The `JSTypeRegistry` DSL value: its owning compiler.
struct NativeTypeRegistry(crate::replay::replay_dsl::CompilerHandle);
impl crate::replay::replay_dsl::NativeObject for NativeTypeRegistry {
    // port: ReplayDsl#invoke (JSTypeRegistry runtime class)
    fn class_name(&self) -> &str {
        "com.google.javascript.rhino.jstype.JSTypeRegistry"
    }
    // port: ReplayDsl#invoke (JSTypeRegistry methods)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        registry_method(&self.0, method, args)
    }
    // port: ReplayDsl#invoke (native receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A `JSType` DSL value: the type's id in the registry of the compiler that owns it.
pub struct NativeJSType {
    compiler: crate::replay::replay_dsl::CompilerHandle,
    id: closure_jstype::TypeId,
}
impl crate::replay::replay_dsl::NativeObject for NativeJSType {
    // port: ReplayDsl#invoke (JSType runtime class)
    fn class_name(&self) -> &str {
        "com.google.javascript.rhino.jstype.JSType"
    }
    // port: ReplayDsl#invoke (JSType methods)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        use closure_jstype::prelude::*;
        let id = self.id;
        match (method, args.len()) {
            ("toString", 0) => with_type_registry(&self.compiler, |registry, ast| {
                Ok(DslValue::String(
                    id.to_string(registry, ast).as_str().into(),
                ))
            }),
            _ => Err(Throwable::Unported(format!(
                "com.google.javascript.rhino.jstype.JSType#{method}"
            ))),
        }
    }
    // port: ReplayDsl#invoke (native receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn js_type_value(
    compiler: &crate::replay::replay_dsl::CompilerHandle,
    id: closure_jstype::TypeId,
) -> DslValue {
    DslValue::Native(std::rc::Rc::new(std::cell::RefCell::new(NativeJSType {
        compiler: compiler.clone(),
        id,
    })))
}

/// The compiler behind a registry DSL value.
fn registry_compiler(
    registry: &DslValue,
) -> Result<crate::replay::replay_dsl::CompilerHandle, Throwable> {
    if let DslValue::Native(o) = registry.untyped()
        && let Some(r) = o
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeTypeRegistry>()
    {
        return Ok(r.0.clone());
    }
    Err(Throwable::HarnessError(format!(
        "not a JSTypeRegistry: {}",
        registry.class_name()
    )))
}

/// A `JSType` argument (`None` for null).
fn js_type_arg(value: &DslValue) -> Result<Option<closure_jstype::TypeId>, Throwable> {
    match value.untyped() {
        DslValue::Null => Ok(None),
        DslValue::Native(o) => o
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeJSType>()
            .map(|t| Some(t.id))
            .ok_or_else(|| {
                Throwable::HarnessError(format!("not a JSType: {}", value.class_name()))
            }),
        other => Err(Throwable::HarnessError(format!(
            "not a JSType: {}",
            other.class_name()
        ))),
    }
}

/// Varargs arrive spread or as one array/list.
fn spread_args(args: &[DslValue]) -> Vec<DslValue> {
    match args {
        [single] => match single.untyped() {
            DslValue::Array { items, .. } | DslValue::List(items) => items.clone(),
            _ => args.to_vec(),
        },
        _ => args.to_vec(),
    }
}

fn non_null(type_: Option<closure_jstype::TypeId>) -> Result<closure_jstype::TypeId, Throwable> {
    type_.ok_or_else(|| Throwable::Exception {
        class: "java.lang.NullPointerException".into(),
        message: None,
    })
}

// port: ReplayDsl#invoke (JSTypeRegistry methods used by CompilerTypeTestCase)
fn registry_method(
    compiler: &crate::replay::replay_dsl::CompilerHandle,
    method: &str,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let id = match method {
        "getNativeType" if args.len() == 1 => rhino::get_native_type(compiler, &args)?,
        "getNativeObjectType" if args.len() == 1 => rhino::get_native_object_type(compiler, &args)?,
        "getNativeFunctionType" if args.len() == 1 => {
            rhino::get_native_function_type(compiler, &args)?
        }
        "createUnionType" => rhino::create_union_type(compiler, &args)?,
        "createNullableType" if args.len() == 1 => rhino::create_nullable_type(compiler, &args)?,
        "createOptionalType" if args.len() == 1 => rhino::create_optional_type(compiler, &args)?,
        "createTemplatizedType" if !args.is_empty() => {
            rhino::create_templatized_type(compiler, &args)?
        }
        _ => {
            return Err(Throwable::Unported(format!(
                "com.google.javascript.rhino.jstype.JSTypeRegistry#{method}"
            )));
        }
    };
    Ok(js_type_value(compiler, id))
}

// port: CompilerTypeTestCase#initializeNewCompiler (JSTypeRegistry method dispatch)
pub fn type_registry_call(
    registry: &DslValue,
    method: &str,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    registry_method(&registry_compiler(registry)?, method, args)
}

/// The `RecordTypeBuilder` DSL value (`addProperty` returns the builder itself).
struct NativeRecordTypeBuilder {
    compiler: crate::replay::replay_dsl::CompilerHandle,
    builder: closure_jstype::record_type_builder::RecordTypeBuilder,
    this: std::rc::Weak<std::cell::RefCell<NativeRecordTypeBuilder>>,
}
impl crate::replay::replay_dsl::NativeObject for NativeRecordTypeBuilder {
    // port: ReplayDsl#invoke (RecordTypeBuilder runtime class)
    fn class_name(&self) -> &str {
        "com.google.javascript.rhino.jstype.RecordTypeBuilder"
    }
    // port: ReplayDsl#invoke (RecordTypeBuilder methods)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.as_slice()) {
            ("addProperty", [name, type_, property_node]) => {
                rhino::add_property(self, name, type_, property_node)
            }
            ("setSynthesized", [synthesized]) => rhino::set_synthesized(self, synthesized),
            ("build", []) => rhino::build(self),
            _ => Err(Throwable::Unported(format!(
                "com.google.javascript.rhino.jstype.RecordTypeBuilder#{method}"
            ))),
        }
    }
    // port: ReplayDsl#invoke (native receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A `TypedScope` DSL value returned by `TypeCheck#processForTesting`.
pub struct NativeTypedScope {
    pub compiler: crate::replay::replay_dsl::CompilerHandle,
    pub scope: closure_jscomp::typed_scope::TypedScope,
}

impl crate::replay::replay_dsl::NativeObject for NativeTypedScope {
    // port: ReplayDsl#invoke (TypedScope runtime class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.TypedScope"
    }
    // port: ReplayDsl#invoke (native receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The test harness's handle on a real `TypeCheck`.
pub struct TypeCheck {
    compiler: crate::replay::replay_dsl::CompilerHandle,
    check: closure_jscomp::type_check::TypeCheck,
}

impl TypeCheck {
    // port: TypeCheck#TypeCheck(AbstractCompiler,ReverseAbstractInterpreter,JSTypeRegistry)
    pub fn new(
        compiler: &crate::replay::replay_dsl::CompilerHandle,
        reverse: DslValue,
        _registry: &DslValue,
    ) -> Result<Self, Throwable> {
        let interpreter = match &reverse {
            DslValue::Native(native) => native
                .borrow_mut()
                .as_any_mut()
                .downcast_mut::<crate::replay::native_type_inference::NativeReverseAbstractInterpreter>()
                .map(|r| Arc::clone(&r.0)),
            _ => None,
        }
        .ok_or_else(|| {
            Throwable::HarnessError("expected a ReverseAbstractInterpreter value".into())
        })?;
        let check = closure_jscomp::type_check::TypeCheck::new(
            &mut *borrow_compiler(compiler)?,
            interpreter,
        );
        Ok(Self {
            compiler: compiler.clone(),
            check,
        })
    }
    // port: TypeCheck#reportUnknownTypes
    pub fn report_unknown_types(self, report: bool) -> Result<Self, Throwable> {
        Ok(Self {
            compiler: self.compiler,
            check: self.check.report_unknown_types(report),
        })
    }
    // port: TypeCheck#processForTesting
    pub fn process_for_testing(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<DslValue, Throwable> {
        let scope = self
            .check
            .process_for_testing(compiler, Some(externs), root);
        Ok(DslValue::Native(std::rc::Rc::new(std::cell::RefCell::new(
            NativeTypedScope {
                compiler: self.compiler.clone(),
                scope,
            },
        ))))
    }
}

fn type_check_typed_scope(
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    report_unknown_types: bool,
) -> Result<closure_jscomp::typed_scope::TypedScope, Throwable> {
    let interpreter =
        closure_jscomp::semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter::new(
            compiler.get_type_registry(),
        );
    let mut check = closure_jscomp::type_check::TypeCheck::new(compiler, interpreter)
        .report_unknown_types(report_unknown_types);
    Ok(check.process_for_testing(compiler, Some(externs), root))
}
