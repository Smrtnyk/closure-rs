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
//   test/com/google/javascript/jscomp/CompilerTestCase.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Faithful CompilerTestCase harness. Compiler-owned operations use jscomp_api and harness_passes.
use crate::jscomp_api::CompilerHarnessAccess;
use crate::{
    derived::ExpectedPipeline,
    harness_passes as passes,
    jscomp_api::{
        CheckLevel, Compiler, CompilerOptions, DiagnosticGroup, DiagnosticType, JSChunk, JSError,
        SourceFile, SourceKind,
    },
    replay::{
        options_fields,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::{decode, source_file},
    },
    throwable::{Throwable, assert_that, check_state},
    value::Value,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    js_string::JsString,
    node::{JsDocComparison, NodeId, RecursionMode, SideEffectComparison, TypeComparison},
    testing::node_subject::assert_node,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, LazyLock},
};
// The NodeSubject#isEqualToInternal assertions, which name Closure's Rhino-derived NodeSubject
// (MPL-1.1 / GPL-2.0-or-later), are in their own file.
#[path = "compiler_test_case_rhino.rs"]
mod rhino;
pub use rhino::tree_equal;

// port: CompilerTestCase#LINE_JOINER
const LINE_JOINER: &str = "\n";
pub const GENERATED_SRC_NAME: &str = "testcode";
pub const GENERATED_EXTERNS_NAME: &str = "externs";
// port: CompilerTestCase#VAR_CHECK_EXTERNS (static initializer)
pub static VAR_CHECK_EXTERNS: LazyLock<Result<JsString, Throwable>> =
    LazyLock::new(CompilerTestCase::var_check_externs);
// port: CompilerTestCase#MINIMAL_EXTERNS (static initializer)
pub static MINIMAL_EXTERNS: LazyLock<Result<JsString, Throwable>> =
    LazyLock::new(CompilerTestCase::minimal_externs);
// port: CompilerTestCase#DEFAULT_EXTERNS (static initializer)
pub static DEFAULT_EXTERNS: LazyLock<Result<JsString, Throwable>> =
    LazyLock::new(CompilerTestCase::default_externs);
pub struct CompilerTestCase {
    pub compare_as_tree: bool,
    pub parse_type_info: bool,
    pub compare_js_doc: bool,
    pub allow_sourceless_warnings: bool,
    pub closure_pass_enabled: bool,
    pub closure_pass_enabled_for_expected: bool,
    pub process_common_js_modules: bool,
    pub rewrite_closure_code: bool,
    pub rewrite_modules_after_typechecking: bool,
    pub rewrite_closure_provides: bool,
    pub type_check_enabled: bool,
    pub replace_types_with_colors: bool,
    pub multistage_compilation: bool,
    pub run_type_check_after_processing: bool,
    pub gather_extern_properties_enabled: bool,
    pub create_module_map: bool,
    pub normalize_enabled: bool,
    pub normalize_expected_output_enabled: bool,
    pub polymer_pass: bool,
    pub rewrite_es_modules_enabled: bool,
    pub transpile_enabled: bool,
    pub infer_consts: bool,
    pub check_access_controls: bool,
    pub check_ast_change_marking: bool,
    pub expect_parse_warnings_in_this_test: bool,
    pub compute_side_effects: bool,
    pub annotate_source_info: bool,
    pub assume_static_inheritance_is_not_used: bool,
    pub allow_externs_changes: bool,
    pub ast_validation_enabled: bool,
    pub type_info_validation_enabled: bool,
    pub set_up_ran: bool,
    pub debug_logging_enabled: bool,
    pub default_externs_inputs: Vec<Arc<SourceFile>>,
    pub libraries_to_inject: IndexSet<String>,
    pub declared_accessors: IndexMap<String, DslValue>,
    pub last_compiler: Option<CompilerHandle>,
    pub accepted_language: DslValue,
    pub language_out: DslValue,
    pub browser_featureset_year: Option<i32>,
    pub module_resolution_mode: DslValue,
    pub parse_js_doc_documentation: DslValue,
    pub ignored_warnings: IndexSet<&'static DiagnosticType>,
    pub webpack_modules_by_id: IndexMap<String, String>,
    pub generic_name_replacements: IndexMap<String, String>,
}
pub trait CompilerTestCaseHooks {
    // port: CompilerTestCase#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable>;
    // port: CompilerTestCase#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        harness.get_options_with_coding_convention(|| self.get_coding_convention())
    }
    // port: CompilerTestCase#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        1
    }
    // port: CompilerTestCase#getName
    fn get_name(&self) -> String {
        "CompilerTestCase".into()
    }
    // port: CompilerTestCase#createCompiler
    fn create_compiler(&mut self, harness: &CompilerTestCase) -> Result<CompilerHandle, Throwable> {
        harness.create_compiler()
    }
    // port: CompilerTestCase#getCodingConvention
    fn get_coding_convention(&self) -> Result<DslValue, Throwable> {
        passes::google_coding_convention()
    }
    // port: ReplayDsl.Ctx#Ctx
    fn ctx(&mut self) -> &mut Ctx;
}
#[derive(Clone)]
pub struct FlatSources {
    pub sources: Vec<Arc<SourceFile>>,
}
pub struct ChunkSources {
    pub chunks: Vec<JSChunk>,
}
pub enum Sources {
    Flat(FlatSources),
    Chunks(ChunkSources),
    EncodedChunks(Vec<crate::record::Chunk>),
}
#[derive(Clone)]
pub struct Expected {
    pub expected: Option<Vec<Arc<SourceFile>>>,
    pub same: bool,
}
#[derive(Clone)]
pub struct Externs {
    pub externs: Vec<Arc<SourceFile>>,
}
pub struct NamedPredicate<T: ?Sized> {
    pub delegate: Rc<dyn Fn(&T) -> bool>,
    pub name: String,
}
impl<T: ?Sized> NamedPredicate<T> {
    // port: CompilerTestCase.NamedPredicate#of
    pub fn of(delegate: impl Fn(&T) -> bool + 'static, name: String) -> Self {
        Self::new(Rc::new(delegate), name)
    }
    // port: CompilerTestCase.NamedPredicate#NamedPredicate
    pub fn new(delegate: Rc<dyn Fn(&T) -> bool>, name: String) -> Self {
        Self { delegate, name }
    }
    // port: CompilerTestCase.NamedPredicate#apply
    pub fn apply(&self, arg: &T) -> bool {
        (self.delegate)(arg)
    }
}
impl<T: ?Sized> std::fmt::Display for NamedPredicate<T> {
    // port: CompilerTestCase.NamedPredicate#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}
pub struct ErrorDiagnostic(pub Diagnostic);
impl ErrorDiagnostic {
    // port: CompilerTestCase.ErrorDiagnostic#ErrorDiagnostic
    pub fn new(diagnostic: &'static DiagnosticType) -> Self {
        Self(Diagnostic::new(CheckLevel::ERROR, diagnostic))
    }
}
pub struct WarningDiagnostic(pub Diagnostic);
impl WarningDiagnostic {
    // port: CompilerTestCase.WarningDiagnostic#WarningDiagnostic
    pub fn new(diagnostic: &'static DiagnosticType) -> Self {
        Self(Diagnostic::new(CheckLevel::WARNING, diagnostic))
    }
}
#[derive(Clone)]
pub struct Diagnostic {
    pub level: CheckLevel,
    pub diagnostic: &'static DiagnosticType,
    pub message_predicate: Option<Rc<NamedPredicate<str>>>,
    pub line: i32,
    pub charno: i32,
    pub length: i32,
}
pub enum TestPart {
    Externs(Externs),
    Sources(Sources),
    Expected(Expected),
    Diagnostic(Diagnostic),
    Postcondition(DslValue),
}
pub type Postcondition = DslValue;
impl CompilerTestCase {
    // port: CompilerTestCase#VAR_CHECK_EXTERNS (initializer expression)
    pub fn var_check_externs() -> Result<JsString, Throwable> {
        let mut out = String::new();
        for symbol in passes::var_check_required_symbols()? {
            out.push_str(&format!("var {symbol};\n"));
        }
        Ok(JsString::from(out))
    }
    // port: CompilerTestCase#MINIMAL_EXTERNS (initializer expression)
    pub fn minimal_externs() -> Result<JsString, Throwable> {
        Self::build_externs(&[
            "addArray",
            "addIterable",
            "addObject",
            "addUndefined",
            "addFunction",
            "addString",
        ])
    }
    // port: CompilerTestCase#DEFAULT_EXTERNS (initializer expression)
    pub fn default_externs() -> Result<JsString, Throwable> {
        Self::build_externs(&[
            "addArray",
            "addIterable",
            "addObject",
            "addUndefined",
            "addFunction",
            "addString",
            "addPromise",
            "addClosureExterns",
            "addITemplateArray",
        ])
    }
    // port: CompilerTestCase#DEFAULT_EXTERNS / MINIMAL_EXTERNS (TestExternsBuilder chain)
    fn build_externs(methods: &[&str]) -> Result<JsString, Throwable> {
        let mut builder = passes::test_externs_builder()?;
        for method in methods {
            builder = crate::unit_test_utils::externs_builder_call(&builder, method, vec![])?;
        }
        match crate::unit_test_utils::externs_builder_call(&builder, "build", vec![])? {
            DslValue::String(s) => Ok(s),
            _ => Err(Throwable::HarnessError(
                "TestExternsBuilder.build must return a String".into(),
            )),
        }
    }
    // port: CompilerTestCase#CompilerTestCase(String)
    pub fn new(externs: impl Into<JsString>) -> Self {
        Self {
            compare_as_tree: false,
            parse_type_info: false,
            compare_js_doc: false,
            allow_sourceless_warnings: false,
            closure_pass_enabled: false,
            closure_pass_enabled_for_expected: false,
            process_common_js_modules: false,
            rewrite_closure_code: false,
            rewrite_modules_after_typechecking: true,
            rewrite_closure_provides: false,
            type_check_enabled: false,
            replace_types_with_colors: false,
            multistage_compilation: false,
            run_type_check_after_processing: false,
            gather_extern_properties_enabled: false,
            create_module_map: false,
            normalize_enabled: false,
            normalize_expected_output_enabled: false,
            polymer_pass: false,
            rewrite_es_modules_enabled: false,
            transpile_enabled: false,
            infer_consts: false,
            check_access_controls: false,
            check_ast_change_marking: false,
            expect_parse_warnings_in_this_test: false,
            compute_side_effects: false,
            annotate_source_info: false,
            assume_static_inheritance_is_not_used: false,
            allow_externs_changes: false,
            ast_validation_enabled: false,
            type_info_validation_enabled: false,
            set_up_ran: false,
            debug_logging_enabled: false,
            default_externs_inputs: vec![Arc::new(SourceFile::from_code(
                GENERATED_EXTERNS_NAME,
                externs,
            ))],
            libraries_to_inject: IndexSet::<_>::default(),
            declared_accessors: IndexMap::<_, _>::default(),
            last_compiler: None,
            accepted_language: DslValue::Null,
            language_out: DslValue::Null,
            browser_featureset_year: None,
            module_resolution_mode: DslValue::Null,
            parse_js_doc_documentation: DslValue::Null,
            ignored_warnings: IndexSet::<_>::default(),
            webpack_modules_by_id: IndexMap::<_, _>::default(),
            generic_name_replacements: IndexMap::<_, _>::default(),
        }
    }
    // port: CompilerTestCase#setUp
    pub fn set_up(&mut self) {
        self.accepted_language = DslValue::Enum {
            class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
            name: "UNSUPPORTED".into(),
        };
        self.module_resolution_mode = DslValue::Enum {
            class: "com.google.javascript.jscomp.deps.ModuleLoader$ResolutionMode".into(),
            name: "BROWSER".into(),
        };
        self.parse_js_doc_documentation = DslValue::Enum {
            class: "com.google.javascript.jscomp.parsing.Config$JsDocParsing".into(),
            name: "TYPES_ONLY".into(),
        };
        self.allow_externs_changes = false;
        self.allow_sourceless_warnings = false;
        self.ast_validation_enabled = true;
        self.type_info_validation_enabled = false;
        self.check_access_controls = false;
        self.check_ast_change_marking = true;
        self.closure_pass_enabled = false;
        self.closure_pass_enabled_for_expected = false;
        self.compare_as_tree = true;
        self.compare_js_doc = true;
        self.compute_side_effects = false;
        self.annotate_source_info = false;
        self.expect_parse_warnings_in_this_test = false;
        self.gather_extern_properties_enabled = false;
        self.infer_consts = false;
        self.language_out = DslValue::Enum {
            class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
            name: "NO_TRANSPILE".into(),
        };
        self.browser_featureset_year = None;
        self.multistage_compilation = false;
        self.normalize_enabled = false;
        self.normalize_expected_output_enabled = false;
        self.parse_type_info = false;
        self.polymer_pass = false;
        self.process_common_js_modules = false;
        self.rewrite_closure_code = false;
        self.rewrite_modules_after_typechecking = true;
        self.run_type_check_after_processing = false;
        self.rewrite_es_modules_enabled = false;
        self.transpile_enabled = false;
        self.type_check_enabled = false;
        self.assume_static_inheritance_is_not_used = true;
        self.set_up_ran = true;
    }
    // port: CompilerTestCase#tearDown
    pub fn tear_down(&mut self) {
        self.set_up_ran = false;
    }
    // port: CompilerTestCase#enableParseTypeInfo
    pub fn enable_parse_type_info(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.parse_type_info = true;
        Ok(())
    }
    // port: CompilerTestCase#disableCompareJsDoc
    pub fn disable_compare_js_doc(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.compare_js_doc = false;
        Ok(())
    }
    // port: CompilerTestCase#enableRunTypeCheckAfterProcessing
    pub fn enable_run_type_check_after_processing(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.run_type_check_after_processing = true;
        Ok(())
    }
    // port: CompilerTestCase#allowSourcelessWarnings
    pub fn allow_sourceless_warnings(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.allow_sourceless_warnings = true;
        Ok(())
    }
    // port: CompilerTestCase#enableInferConsts
    pub fn enable_infer_consts(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.infer_consts = true;
        Ok(())
    }
    // port: CompilerTestCase#enableCheckAccessControls
    pub fn enable_check_access_controls(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.check_access_controls = true;
        Ok(())
    }
    // port: CompilerTestCase#allowExternsChanges
    pub fn allow_externs_changes(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.allow_externs_changes = true;
        Ok(())
    }
    // port: CompilerTestCase#enablePolymerPass
    pub fn enable_polymer_pass(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.polymer_pass = true;
        Ok(())
    }
    // port: CompilerTestCase#enableTypeCheck
    pub fn enable_type_check(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.type_check_enabled = true;
        self.enable_create_module_map()?;
        Ok(())
    }
    // port: CompilerTestCase#disableTypeCheck
    pub fn disable_type_check(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.type_check_enabled = false;
        Ok(())
    }
    // port: CompilerTestCase#replaceTypesWithColors
    pub fn replace_types_with_colors(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.replace_types_with_colors = true;
        Ok(())
    }
    // port: CompilerTestCase#enableCreateModuleMap
    pub fn enable_create_module_map(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.create_module_map = true;
        Ok(())
    }
    // port: CompilerTestCase#enableMultistageCompilation
    pub fn enable_multistage_compilation(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.multistage_compilation = true;
        Ok(())
    }
    // port: CompilerTestCase#disableMultistageCompilation
    pub fn disable_multistage_compilation(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.multistage_compilation = false;
        Ok(())
    }
    // port: CompilerTestCase#disableValidateAstChangeMarking
    pub fn disable_validate_ast_change_marking(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.check_ast_change_marking = false;
        Ok(())
    }
    // port: CompilerTestCase#enableClosurePass
    pub fn enable_closure_pass(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.closure_pass_enabled = true;
        self.enable_create_module_map()?;
        Ok(())
    }
    // port: CompilerTestCase#enableClosurePassForExpected
    pub fn enable_closure_pass_for_expected(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.closure_pass_enabled_for_expected = true;
        Ok(())
    }
    // port: CompilerTestCase#enableProcessCommonJsModules
    pub fn enable_process_common_js_modules(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.process_common_js_modules = true;
        Ok(())
    }
    // port: CompilerTestCase#enableRewriteClosureCode
    pub fn enable_rewrite_closure_code(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.rewrite_closure_code = true;
        self.enable_create_module_map()?;
        Ok(())
    }
    // port: CompilerTestCase#disableRewriteClosureCode
    pub fn disable_rewrite_closure_code(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.rewrite_closure_code = false;
        Ok(())
    }
    // port: CompilerTestCase#enableRewriteModulesAfterTypechecking
    pub fn enable_rewrite_modules_after_typechecking(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.enable_rewrite_closure_code()?;
        self.rewrite_modules_after_typechecking = true;
        Ok(())
    }
    // port: CompilerTestCase#disableRewriteModulesAfterTypechecking
    pub fn disable_rewrite_modules_after_typechecking(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.rewrite_modules_after_typechecking = false;
        Ok(())
    }
    // port: CompilerTestCase#enableRewriteClosureProvides
    pub fn enable_rewrite_closure_provides(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.rewrite_closure_provides = true;
        self.enable_create_module_map()?;
        Ok(())
    }
    // port: CompilerTestCase#enableNormalize
    pub fn enable_normalize(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.normalize_enabled = true;
        self.enable_multistage_compilation()?;
        Ok(())
    }
    // port: CompilerTestCase#enableNormalizeExpectedOutput
    pub fn enable_normalize_expected_output(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        check_state(
            self.normalize_enabled,
            "Enabled normalize on output, but not input.",
        )?;
        self.normalize_expected_output_enabled = true;
        Ok(())
    }
    // port: CompilerTestCase#enableTranspile
    pub fn enable_transpile(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.transpile_enabled = true;
        Ok(())
    }
    // port: CompilerTestCase#disableNormalize
    pub fn disable_normalize(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.normalize_enabled = false;
        Ok(())
    }
    // port: CompilerTestCase#enableRewriteEsModules
    pub fn enable_rewrite_es_modules(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.rewrite_es_modules_enabled = true;
        Ok(())
    }
    // port: CompilerTestCase#enableComputeSideEffects
    pub fn enable_compute_side_effects(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.compute_side_effects = true;
        Ok(())
    }
    // port: CompilerTestCase#disableComputeSideEffects
    pub fn disable_compute_side_effects(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.compute_side_effects = false;
        Ok(())
    }
    // port: CompilerTestCase#enableSourceInformationAnnotator
    pub fn enable_source_information_annotator(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.annotate_source_info = true;
        Ok(())
    }
    // port: CompilerTestCase#enableGatherExternProperties
    pub fn enable_gather_extern_properties(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.gather_extern_properties_enabled = true;
        Ok(())
    }
    // port: CompilerTestCase#disableAstValidation
    pub fn disable_ast_validation(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.ast_validation_enabled = false;
        Ok(())
    }
    // port: CompilerTestCase#enableTypeInfoValidation
    pub fn enable_type_info_validation(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.type_info_validation_enabled = true;
        Ok(())
    }
    // port: CompilerTestCase#disableTypeInfoValidation
    pub fn disable_type_info_validation(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.type_info_validation_enabled = false;
        Ok(())
    }
    // port: CompilerTestCase#disableCompareAsTree
    pub fn disable_compare_as_tree(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.compare_as_tree = false;
        Ok(())
    }
    // port: CompilerTestCase#setExpectParseWarningsInThisTest
    pub fn set_expect_parse_warnings_in_this_test(&mut self) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.expect_parse_warnings_in_this_test = true;
        Ok(())
    }
    // port: CompilerTestCase#disableGenericNameReplacements
    pub fn disable_generic_name_replacements(&mut self) -> Result<(), Throwable> {
        self.generic_name_replacements.clear();
        Ok(())
    }
    // port: Preconditions#checkState (CompilerTestCase configuration)
    fn check_set_up(&self) -> Result<(), Throwable> {
        check_state(
            self.set_up_ran,
            "Attempted to configure before running setUp().",
        )
    }
    // port: ReplayValues#setField (CompilerTestCase fields)
    pub fn restore_field(
        &mut self,
        name: &str,
        value: &Value,
        class_map: &IndexMap<String, String>,
    ) -> Result<(), Throwable> {
        match name {
            "compareAsTree" => {
                self.compare_as_tree = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "parseTypeInfo" => {
                self.parse_type_info = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "compareJsDoc" => {
                self.compare_js_doc = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "allowSourcelessWarnings" => {
                self.allow_sourceless_warnings = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "closurePassEnabled" => {
                self.closure_pass_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "closurePassEnabledForExpected" => {
                self.closure_pass_enabled_for_expected = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "processCommonJsModules" => {
                self.process_common_js_modules = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "rewriteClosureCode" => {
                self.rewrite_closure_code = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "rewriteModulesAfterTypechecking" => {
                self.rewrite_modules_after_typechecking = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "rewriteClosureProvides" => {
                self.rewrite_closure_provides = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "typeCheckEnabled" => {
                self.type_check_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "replaceTypesWithColors" => {
                self.replace_types_with_colors = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "multistageCompilation" => {
                self.multistage_compilation = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "runTypeCheckAfterProcessing" => {
                self.run_type_check_after_processing = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "gatherExternPropertiesEnabled" => {
                self.gather_extern_properties_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "createModuleMap" => {
                self.create_module_map = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "normalizeEnabled" => {
                self.normalize_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "normalizeExpectedOutputEnabled" => {
                self.normalize_expected_output_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "polymerPass" => {
                self.polymer_pass = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "rewriteEsModulesEnabled" => {
                self.rewrite_es_modules_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "transpileEnabled" => {
                self.transpile_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "inferConsts" => {
                self.infer_consts = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "checkAccessControls" => {
                self.check_access_controls = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "checkAstChangeMarking" => {
                self.check_ast_change_marking = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "expectParseWarningsInThisTest" => {
                self.expect_parse_warnings_in_this_test = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "computeSideEffects" => {
                self.compute_side_effects = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "annotateSourceInfo" => {
                self.annotate_source_info = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "assumeStaticInheritanceIsNotUsed" => {
                self.assume_static_inheritance_is_not_used = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "allowExternsChanges" => {
                self.allow_externs_changes = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "astValidationEnabled" => {
                self.ast_validation_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "typeInfoValidationEnabled" => {
                self.type_info_validation_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }

            "debugLoggingEnabled" => {
                self.debug_logging_enabled = value.as_bool().ok_or_else(|| {
                    Throwable::HarnessError(format!("invalid harness boolean {name}"))
                })?
            }
            "acceptedLanguage" => {
                self.accepted_language = decode(value, "java.lang.Object", class_map)?
            }
            "languageOut" => self.language_out = decode(value, "java.lang.Object", class_map)?,
            "moduleResolutionMode" => {
                self.module_resolution_mode = decode(value, "java.lang.Object", class_map)?
            }
            "parseJsDocDocumentation" => {
                self.parse_js_doc_documentation = decode(value, "java.lang.Object", class_map)?
            }
            "lastCompiler" | "setUpRan" | "__currentRec" | "__recCompiler"
            | "__recPostCompiler" | "__recDepth" => {}
            "browserFeaturesetYear" => {
                self.browser_featureset_year = if matches!(value, Value::Null) {
                    None
                } else {
                    Some(value.as_i32().ok_or_else(|| {
                        Throwable::HarnessError("invalid browserFeaturesetYear".into())
                    })?)
                }
            }
            "defaultExternsInputs" => {
                let Value::List { items, .. } = value else {
                    return Err(Throwable::HarnessError(
                        "defaultExternsInputs not a list".into(),
                    ));
                };
                self.default_externs_inputs = items
                    .iter()
                    .map(|v| {
                        if let Value::SourceFile(f) = v {
                            Ok(source_file(f))
                        } else {
                            Err(Throwable::HarnessError("extern not a SourceFile".into()))
                        }
                    })
                    .collect::<Result<_, _>>()?;
            }
            "ignoredWarnings" => {
                let Value::Set { items, .. } = value else {
                    return Err(Throwable::HarnessError("ignoredWarnings not a set".into()));
                };
                self.ignored_warnings = items
                    .iter()
                    .map(|v| match decode(v, "java.lang.Object", class_map)? {
                        DslValue::DiagnosticType(t) => Ok(t),
                        _ => Err(Throwable::HarnessError(
                            "ignored warning not a DiagnosticType".into(),
                        )),
                    })
                    .collect::<Result<_, _>>()?;
            }
            "librariesToInject" => {
                let Value::Set { items, .. } = value else {
                    return Err(Throwable::HarnessError(
                        "librariesToInject not a set".into(),
                    ));
                };
                self.libraries_to_inject = items
                    .iter()
                    .map(|v| {
                        if let Value::String(s) = v {
                            Ok(s.to_string_lossy())
                        } else {
                            Err(Throwable::HarnessError("library not a String".into()))
                        }
                    })
                    .collect::<Result<_, _>>()?;
            }
            "webpackModulesById" | "genericNameReplacements" => {
                let Value::Map { entries, .. } = value else {
                    return Err(Throwable::HarnessError("harness map not a map".into()));
                };
                let map = entries
                    .iter()
                    .map(|(k, v)| {
                        if let (Value::String(k), Value::String(v)) = (k, v) {
                            Ok((k.to_string_lossy(), v.to_string_lossy()))
                        } else {
                            Err(Throwable::HarnessError(
                                "harness map not String/String".into(),
                            ))
                        }
                    })
                    .collect::<Result<_, _>>()?;
                if name == "webpackModulesById" {
                    self.webpack_modules_by_id = map;
                } else {
                    self.generic_name_replacements = map;
                }
            }
            "declaredAccessors" => {
                let Value::Map { entries, .. } = value else {
                    return Err(Throwable::HarnessError(
                        "declaredAccessors not a map".into(),
                    ));
                };
                self.declared_accessors = entries
                    .iter()
                    .map(|(k, v)| {
                        if let Value::String(k) = k {
                            Ok((
                                k.to_string_lossy(),
                                decode(v, "java.lang.Object", class_map)?,
                            ))
                        } else {
                            Err(Throwable::HarnessError("accessor key not String".into()))
                        }
                    })
                    .collect::<Result<_, _>>()?;
            }
            _ => {
                return Err(Throwable::HarnessError(format!(
                    "unknown CompilerTestCase field {name}"
                )));
            }
        }
        Ok(())
    }
    // port: CompilerTestCase#enableDebugLogging
    pub fn enable_debug_logging(&mut self, value: bool) {
        self.debug_logging_enabled = value;
    }
    // port: CompilerTestCase#ignoreWarnings(DiagnosticType...)
    pub fn ignore_warnings(&mut self, types: &[&'static DiagnosticType]) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.ignored_warnings.extend(types.iter().copied());
        Ok(())
    }
    // port: CompilerTestCase#ignoreWarnings(DiagnosticGroup...)
    pub fn ignore_warning_groups(
        &mut self,
        groups: &[Arc<DiagnosticGroup>],
    ) -> Result<(), Throwable> {
        self.check_set_up()?;
        for group in groups {
            self.ignored_warnings
                .extend(group.get_types().iter().copied());
        }
        Ok(())
    }
    // port: CompilerTestCase#setAcceptedLanguage
    pub fn set_accepted_language(&mut self, lang: DslValue) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.set_language(lang.clone(), lang)
    }
    // port: CompilerTestCase#setLanguage
    pub fn set_language(&mut self, lang_in: DslValue, lang_out: DslValue) -> Result<(), Throwable> {
        self.accepted_language = lang_in;
        self.set_language_out(lang_out)
    }
    // port: CompilerTestCase#setLanguageOut
    pub fn set_language_out(&mut self, lang: DslValue) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.language_out = lang;
        Ok(())
    }
    // port: CompilerTestCase#setBrowserFeaturesetYear
    pub fn set_browser_featureset_year(&mut self, year: Option<i32>) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.browser_featureset_year = year;
        Ok(())
    }
    // port: CompilerTestCase#setModuleResolutionMode
    pub fn set_module_resolution_mode(&mut self, mode: DslValue) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.module_resolution_mode = mode;
        Ok(())
    }
    // port: CompilerTestCase#setJsDocumentation
    pub fn set_js_documentation(&mut self, mode: DslValue) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.parse_js_doc_documentation = mode;
        Ok(())
    }
    // port: CompilerTestCase#setAssumeStaticInheritanceIsNotUsed
    pub fn set_assume_static_inheritance_is_not_used(
        &mut self,
        value: bool,
    ) -> Result<(), Throwable> {
        self.check_set_up()?;
        self.assume_static_inheritance_is_not_used = value;
        Ok(())
    }
    // port: CompilerTestCase#declareAccessor
    pub fn declare_accessor(&mut self, name: String, kind: DslValue) -> Result<(), Throwable> {
        self.check_set_up()?;
        check_state(
            matches!(&kind,DslValue::Enum{name,..} if name!="NORMAL"),
            "Kind should be a getter and/or setter.",
        )?;
        self.declared_accessors.insert(name, kind);
        Ok(())
    }
    // port: CompilerTestCase#setWebpackModulesById
    pub fn set_webpack_modules_by_id(&mut self, map: IndexMap<String, String>) {
        self.webpack_modules_by_id = map;
    }
    // port: CompilerTestCase#setGenericNameReplacements
    pub fn set_generic_name_replacements(&mut self, map: IndexMap<String, String>) {
        self.generic_name_replacements = map;
    }
    // port: CompilerTestCase#ensureLibraryInjected
    pub fn ensure_library_injected(&mut self, name: String) {
        self.libraries_to_inject.insert(name);
    }
    // port: CompilerTestCase#getLastCompiler
    pub fn get_last_compiler(&self) -> Option<CompilerHandle> {
        self.last_compiler.clone()
    }
    // port: CompilerTestCase#getGatheredExternProperties
    pub fn get_gathered_extern_properties(&self) -> Result<Option<Vec<String>>, Throwable> {
        check_state(
            self.gather_extern_properties_enabled,
            "Must enable gatherExternProperties",
        )?;
        Ok(self
            .last_compiler
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("no lastCompiler".into()))?
            .borrow()
            .get_extern_properties()
            .map(|p| p.iter().cloned().collect()))
    }
    // port: CompilerTestCase#getOptions
    pub fn get_options(&self) -> Result<CompilerOptions, Throwable> {
        self.get_options_with_coding_convention(passes::google_coding_convention)
    }
    // port: CompilerTestCase#getOptions (getCodingConvention hook)
    pub fn get_options_with_coding_convention(
        &self,
        coding_convention: impl FnOnce() -> Result<DslValue, Throwable>,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options = CompilerOptions::new();
        options.set_language_in(options_fields::language_mode(&self.accepted_language)?);
        options.set_emit_use_strict(false);
        options.set_language_out(options_fields::language_mode(&self.language_out)?);
        if let Some(year) = self.browser_featureset_year {
            options.set_browser_featureset_year(year);
        }
        options.set_module_resolution_mode(options_fields::resolution_mode(
            &self.module_resolution_mode,
        )?);
        options.set_parse_js_doc_documentation(options_fields::jsdoc_parsing(
            &self.parse_js_doc_documentation,
        )?);
        options
            .set_assume_static_inheritance_is_not_used(self.assume_static_inheritance_is_not_used);
        options.set_preserve_type_annotations(true);
        options.set_assume_getters_are_pure(false);
        options.set_check_symbols(true);
        for group in [
            "INVALID_CASTS",
            "MISPLACED_MSG_ANNOTATION",
            "MISSING_PROPERTIES",
        ] {
            options.set_warning_level(passes::diagnostic_group(group)?, CheckLevel::WARNING);
        }
        if !self.ignored_warnings.is_empty() {
            options.set_warning_level(
                Arc::new(DiagnosticGroup::new(
                    &self.ignored_warnings.iter().copied().collect::<Vec<_>>(),
                )),
                CheckLevel::OFF,
            );
        }
        options.set_coding_convention(passes::decode_coding_convention(&coding_convention()?)?);
        options.set_polymer_version(Some(1));
        if self.debug_logging_enabled {
            crate::compiler_test_case_utils::set_debug_log_directory_on(&mut options);
        }
        options.set_runtime_library_mode(closure_jscomp::js::runtime_js_lib_manager::RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS);
        Ok(options)
    }
    // port: CompilerTestCase#createCompiler
    pub fn create_compiler(&self) -> Result<CompilerHandle, Throwable> {
        let mut c = Compiler::new();
        c.set_allowable_features(
            options_fields::language_mode(&self.accepted_language)?.to_feature_set(),
        );
        if !self.webpack_modules_by_id.is_empty() {
            c.init_webpack_map(self.webpack_modules_by_id.clone());
        }
        c.set_prefer_regex_parser(false);
        Ok(Rc::new(RefCell::new(c)))
    }
    // port: CompilerTestCase#createAndInitializeCompiler
    pub fn create_and_initialize_compiler(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: &Externs,
        inputs: &Sources,
    ) -> Result<CompilerHandle, Throwable> {
        let c = hooks.create_compiler(self)?;
        let mut options = hooks.get_options(self)?;
        match inputs {
            Sources::Flat(f) => {
                options.set_check_types(self.parse_type_info || self.type_check_enabled);
                c.borrow_mut().init(&externs.externs, &f.sources, options);
            }
            Sources::Chunks(ch) => {
                let o = hooks.get_options(self)?;
                c.borrow_mut()
                    .init_chunks(&externs.externs, ch.chunks.clone(), o);
            }
            Sources::EncodedChunks(records) => {
                let chunks = crate::replay::replay_values::chunks(records)?;
                let o = hooks.get_options(self)?;
                c.borrow_mut().init_chunks(&externs.externs, chunks, o);
            }
        }
        Ok(c)
    }
    // port: CompilerTestCase#testInternal(Externs,Sources,Expected,List,List)
    #[expect(
        clippy::too_many_arguments,
        reason = "Java harness overload and expected pipeline"
    )]
    pub fn test_internal(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: &Externs,
        inputs: &Sources,
        expected: Option<&Expected>,
        diagnostics: &[Diagnostic],
        postconditions: &[Postcondition],
        pipeline: Option<&ExpectedPipeline>,
    ) -> Result<(), Throwable> {
        let c = self.create_and_initialize_compiler(hooks, externs, inputs)?;
        self.last_compiler = Some(c.clone());
        if self.type_check_enabled {
            passes::add_native_properties(&mut c.borrow_mut())?;
        }
        self.test_internal_with_compiler(
            hooks,
            c,
            externs,
            inputs,
            expected,
            diagnostics,
            postconditions,
            pipeline,
        )
    }
    // port: CompilerTestCase#testInternal(Compiler,Externs,Sources,Expected,List,List)
    #[expect(clippy::too_many_arguments, reason = "Java harness overload")]
    fn test_internal_with_compiler(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        mut c: CompilerHandle,
        externs: &Externs,
        inputs: &Sources,
        expected: Option<&Expected>,
        diagnostics: &[Diagnostic],
        postconditions: &[Postcondition],
        pipeline: Option<&ExpectedPipeline>,
    ) -> Result<(), Throwable> {
        let mut generic_name_mapping = IndexMap::<_, _>::default();
        let expected = if let Some(e) = expected {
            if e.same {
                Some(from_sources(inputs)?)
            } else {
                Some(e.clone())
            }
        } else {
            None
        };
        let expected = if !self.generic_name_replacements.is_empty() {
            if let (Some(e), Sources::Flat(f)) = (&expected, inputs) {
                if e.expected.is_some() {
                    Some(Expected {
                        expected: Some(
                            crate::unit_test_utils::update_generic_var_names_in_expected_files(
                                f,
                                e,
                                &self.generic_name_replacements,
                                &mut generic_name_mapping,
                            )?,
                        ),
                        same: false,
                    })
                } else {
                    expected
                }
            } else {
                expected
            }
        } else {
            expected
        };
        let expected_errors = diagnostics
            .iter()
            .filter(|d| d.level == CheckLevel::ERROR)
            .cloned()
            .collect::<Vec<_>>();
        let expected_warnings = diagnostics
            .iter()
            .filter(|d| d.level == CheckLevel::WARNING)
            .cloned()
            .collect::<Vec<_>>();
        check_state(
            expected_errors.is_empty() || expected_warnings.is_empty(),
            "Cannot expect both errors and warnings.",
        )?;
        check_state(
            expected_errors.is_empty()
                || expected
                    .as_ref()
                    .and_then(|e| e.expected.as_ref())
                    .is_none(),
            "Cannot expect both errors and compiled output.",
        )?;
        check_state(
            self.set_up_ran,
            "CompilerTestCase.setUp not run: call super.setUp() from overrides.",
        )?;
        c.borrow_mut().add_recent_change_handler();
        let Some(mut root) = c.borrow_mut().parse_inputs() else {
            contains_exactly(
                &c.borrow().get_errors(),
                &expected_errors,
                false,
                "parse errors",
            )?;
            return verify_postconditions(postconditions, c, hooks.ctx());
        };
        if !self.expect_parse_warnings_in_this_test {
            assert_that(
                c.borrow().get_warnings().is_empty(),
                "Unexpected parser warning(s)",
            )?;
        } else {
            assert_that(
                c.borrow().get_warning_count() > 0,
                "Expected parser warning(s)",
            )?;
        }
        let mut externs_root = root.get_first_child(&c.borrow()).unwrap();
        let mut main_root = root.get_last_child(&c.borrow()).unwrap();
        if self.annotate_source_info {
            passes::source_information_annotator(&mut c.borrow_mut(), externs_root, main_root)?;
        }
        if self.create_module_map {
            passes::gather_module_metadata(
                &mut c.borrow_mut(),
                externs_root,
                main_root,
                false,
                "BROWSER",
            )?;
            passes::module_map_creator(&mut c.borrow_mut(), externs_root, main_root)?;
        }
        if self.ast_validation_enabled {
            passes::ast_validator(&mut c.borrow_mut(), externs_root, root, true, "NONE")?;
        }
        let clone_compiler = c.clone();
        let root_clone = root.clone_tree(&mut c.borrow_mut());
        let externs_root_clone = root_clone.get_first_child(&c.borrow()).unwrap();
        let main_root_clone = root_clone.get_last_child(&c.borrow()).unwrap();
        let num_repetitions = hooks.get_num_repetitions();
        let mut error_managers = vec![];
        let mut aggregate_warnings = vec![];
        let mut has_code_changed = false;
        for i in 0..num_repetitions {
            if c.borrow().get_error_count() != 0 {
                continue;
            }
            c.borrow_mut()
                .set_error_manager(crate::jscomp_api::black_hole_error_manager());
            if self.polymer_pass && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::polymer_pass(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if self.rewrite_closure_code && i == 0 {
                passes::check_closure_imports(&mut c.borrow_mut(), externs_root, main_root)?;
                passes::scoped_aliases(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if self.rewrite_closure_code && !self.rewrite_modules_after_typechecking && i == 0 {
                passes::closure_rewrite_module(
                    &mut c.borrow_mut(),
                    externs_root,
                    main_root,
                    false,
                )?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if self.closure_pass_enabled && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::process_closure_primitives(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if self.process_common_js_modules && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::process_common_js_modules(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if (self.rewrite_es_modules_enabled || self.transpile_enabled) && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::rewrite_es_modules(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            let inject_libraries_from_typed_asts =
                self.multistage_compilation || self.replace_types_with_colors;
            if !self.libraries_to_inject.is_empty() && i == 0 && !inject_libraries_from_typed_asts {
                self.inject_libraries(&c)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if !self.run_type_check_after_processing && self.type_check_enabled && i == 0 {
                create_type_check(&c, externs_root, main_root)?;
            }
            if self.rewrite_closure_code && self.rewrite_modules_after_typechecking && i == 0 {
                passes::closure_rewrite_module(&mut c.borrow_mut(), externs_root, main_root, true)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if self.infer_consts && i == 0 {
                passes::infer_consts(&mut c.borrow_mut(), externs_root, main_root)?;
            }
            if self.gather_extern_properties_enabled && i == 0 {
                passes::gather_extern_properties(
                    &mut c.borrow_mut(),
                    externs_root,
                    main_root,
                    "CHECK_AND_OPTIMIZE",
                )?;
            }
            if i == 0 {
                if self.multistage_compilation {
                    if let Sources::Flat(f) = inputs {
                        c.borrow_mut().reset_recent_change();
                        let new_compiler =
                            crate::compiler_test_case_utils::multistage_serialize_and_deserialize(
                                hooks,
                                self,
                                &mut c.borrow_mut(),
                                &externs.externs,
                                &f.sources,
                            )?;
                        c = new_compiler;
                        root = c.borrow().get_root().unwrap();
                        externs_root = c.borrow().get_externs_root().unwrap();
                        main_root = c.borrow().get_js_root().unwrap();
                        self.last_compiler = Some(c.clone());
                        has_code_changed |= c.borrow().has_code_changed();
                    }
                } else if self.replace_types_with_colors {
                    c.borrow_mut().reset_recent_change();
                    passes::remove_cast_nodes(&mut c.borrow_mut(), externs_root, main_root)?;
                    passes::convert_types_to_colors(
                        &mut c.borrow_mut(),
                        externs_root,
                        main_root,
                        true,
                    )?;
                    c.borrow_mut().set_life_cycle_stage(
                        crate::jscomp_api::LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC,
                    );
                    has_code_changed |= c.borrow().has_code_changed();
                }
                if !self.libraries_to_inject.is_empty() && inject_libraries_from_typed_asts {
                    self.inject_libraries(&c)?;
                    has_code_changed |= c.borrow().has_code_changed();
                }
            }
            if self.rewrite_closure_provides && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::process_closure_provides_and_requires(
                    &mut c.borrow_mut(),
                    externs_root,
                    main_root,
                    false,
                )?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            if self.transpile_enabled && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::transpile_to_es5(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            } else if self.normalize_enabled && i == 0 {
                normalize_actual_code(&c, externs_root, main_root)?;
            }
            self.update_accessor_summary(&c, main_root)?;
            if self.compute_side_effects && i == 0 {
                c.borrow_mut().reset_recent_change();
                passes::pure_function_identifier(&mut c.borrow_mut(), externs_root, main_root)?;
                has_code_changed |= c.borrow().has_code_changed();
            }
            c.borrow_mut().reset_recent_change();
            let change_verifier = if self.check_ast_change_marking {
                Some(passes::change_verifier_snapshot(
                    &mut c.borrow_mut(),
                    main_root,
                )?)
            } else {
                None
            };
            c.borrow_mut().before_pass(&hooks.get_name());
            let processor = hooks.get_processor(c.clone())?;
            crate::replay::replay_dsl::process(
                &processor,
                c.clone(),
                externs_root,
                main_root,
                hooks.ctx(),
            )?;
            if let Some(snapshot) = change_verifier {
                passes::check_recorded_changes(&mut c.borrow_mut(), main_root, &snapshot)?;
            }
            self.verify_accessor_summary(&c, main_root)?;
            if self.run_type_check_after_processing && self.type_check_enabled && i == 0 {
                create_type_check(&c, externs_root, main_root)?;
            }
            if self.ast_validation_enabled && !c.borrow().has_halting_errors() {
                let mode = if self.type_info_validation_enabled {
                    if c.borrow().has_optimization_colors() {
                        "COLOR"
                    } else {
                        "JSTYPE"
                    }
                } else {
                    "NONE"
                };
                passes::ast_validator(&mut c.borrow_mut(), externs_root, root, true, mode)?;
            }
            passes::source_info_check(&mut c.borrow_mut(), externs_root, main_root)?;
            if self.check_access_controls {
                passes::check_access_controls(&mut c.borrow_mut(), externs_root, main_root)?;
            }
            has_code_changed |= c.borrow().has_code_changed();
            let warnings = c.borrow().get_warnings();
            aggregate_warnings.extend(warnings.clone());
            error_managers.push(warnings);
            if self.normalize_enabled {
                passes::verify_constants(&mut c.borrow_mut(), externs_root, main_root, true)?;
            }
        }
        if expected_errors.is_empty() {
            assert_that(
                c.borrow().get_errors().is_empty(),
                "Unexpected compile errors",
            )?;
            self.validate_warnings(
                &c.borrow(),
                &expected_warnings,
                &error_managers,
                &aggregate_warnings,
                num_repetitions,
            )?;
            if self.normalize_enabled {
                let has_typechecking_run = c.borrow().has_type_checking_run();
                c.borrow_mut().set_type_checking_has_run(false);
                normalize_actual_code(&clone_compiler, externs_root_clone, main_root_clone)?;
                c.borrow_mut()
                    .set_type_checking_has_run(has_typechecking_run);
            }
            if self.check_ast_change_marking || !self.allow_externs_changes {
                self.validate_code_change_reporting(
                    &c,
                    main_root,
                    externs_root,
                    main_root_clone,
                    externs_root_clone,
                    has_code_changed,
                    &clone_compiler,
                )?;
            }
            if let Some(files) = expected.as_ref().and_then(|e| e.expected.as_ref()) {
                let expected_root = self.parse_expected_js(hooks, &c, files, pipeline)?;
                if self.compare_as_tree {
                    self.compare_expected_to_actual_as_tree(
                        &c,
                        main_root,
                        expected_root,
                        &generic_name_mapping,
                    )?;
                } else {
                    self.compare_expected_to_actual_as_strings(&c, main_root, files)?;
                }
            }
            self.validate_normalization_invariants(&c, root)?;
        } else {
            let errors = c.borrow().get_errors();
            contains_exactly(&errors, &expected_errors, false, "compile errors")?;
            for error in errors {
                self.validate_source_location(&error)?;
                assert_that(
                    !error
                        .description()
                        .as_bytes()
                        .windows(3)
                        .any(|w| w[0] == b'{' && w[1].is_ascii_digit() && w[2] == b'}'),
                    "Some placeholders in the error message were not replaced",
                )?;
            }
        }
        verify_postconditions(postconditions, c, hooks.ctx())
    }
    // port: CompilerTestCase#validateWarnings
    pub fn validate_warnings(
        &self,
        compiler: &Compiler,
        expected: &[Diagnostic],
        managers: &[Vec<JSError>],
        aggregate: &[JSError],
        num_repetitions: i32,
    ) -> Result<(), Throwable> {
        if expected.is_empty() {
            assert_that(
                aggregate.is_empty(),
                format!(
                    "aggregate warnings: {}",
                    aggregate
                        .iter()
                        .map(|warning| passes::format_warning(compiler, warning))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            )?;
        } else {
            assert_that(
                aggregate.len() == (num_repetitions as usize) * expected.len(),
                format!(
                    "There should be {} warnings, repeated {num_repetitions} time(s). Warnings: \n{}",
                    expected.len(),
                    aggregate
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(LINE_JOINER)
                ),
            )?;
            for (i, warnings) in managers.iter().enumerate() {
                contains_exactly(
                    warnings,
                    expected,
                    false,
                    &format!("compile warnings from repetition {}", i + 1),
                )?;
                for warning in warnings {
                    self.validate_source_location(warning)?;
                }
            }
        }
        Ok(())
    }
    // port: CompilerTestCase#validateSourceLocation
    pub fn validate_source_location(&self, error: &JSError) -> Result<(), Throwable> {
        if !self.allow_sourceless_warnings {
            let source = error.source_name().filter(|s| !s.is_empty());
            assert_that(
                source.is_some(),
                format!("Missing source file name in warning: {error}"),
            )?;
            assert_that(
                self.last_compiler
                    .as_ref()
                    .is_some_and(|c| c.borrow().get_script_node(source.unwrap()).is_some()),
                format!("No SCRIPT node found for warning: {error}"),
            )?;
            assert_that(
                error.lineno() != -1,
                format!("Missing line number in warning: {error}"),
            )?;
            assert_that(
                error.charno() != -1,
                format!("Missing char number in warning: {error}"),
            )?;
        }
        Ok(())
    }
    // port: CompilerTestCase#validateCodeChangeReporting
    #[expect(
        clippy::too_many_arguments,
        reason = "Java roots retain their owning compiler"
    )]
    fn validate_code_change_reporting(
        &self,
        c: &CompilerHandle,
        main: NodeId,
        externs: NodeId,
        main_clone: NodeId,
        externs_clone: NodeId,
        reported: bool,
        clone_compiler: &CompilerHandle,
    ) -> Result<(), Throwable> {
        let ast = c.borrow();
        let clone_ast = clone_compiler.borrow();
        let equiv = |a: NodeId, b: NodeId| {
            a.is_equivalent_to_across_with_options(
                &clone_ast,
                &ast,
                b,
                RecursionMode::DEEP_NO_SHADOW,
                TypeComparison::IGNORE,
                JsDocComparison::IGNORE,
                SideEffectComparison::COMPARE,
            )
        };
        let code_change = !equiv(main_clone, main);
        let externs_change = !equiv(externs_clone, externs);
        if externs_change && !self.allow_externs_changes {
            tree_equal_across(
                &ast,
                externs,
                &clone_ast,
                externs_clone,
                false,
                &IndexMap::<_, _>::default(),
            )?;
        }
        if self.check_ast_change_marking {
            // Java formats both trees eagerly; that has no effect there, so the message is built
            // only on failure. The clone's JSTypes are the compiler's (Java's cloneTree shares
            // them), so both trees print their types through `c`'s registry.
            let passed = reported == (code_change || externs_change);
            drop(ast);
            drop(clone_ast);
            assert_that(
                passed,
                if passed {
                    String::new()
                } else if code_change || externs_change {
                    let mut compiler = c.borrow_mut();
                    let original = if Rc::ptr_eq(c, clone_compiler) {
                        crate::node_printing::to_string_tree(&mut compiler, None, main_clone)
                    } else {
                        let clone_ast = clone_compiler.borrow();
                        crate::node_printing::to_string_tree(
                            &mut compiler,
                            Some(&clone_ast),
                            main_clone,
                        )
                    };
                    format!(
                        "compiler.reportCodeChange() should have been called.\nOriginal: {}\nNew: {}",
                        original,
                        crate::node_printing::to_string_tree(&mut compiler, None, main)
                    )
                } else {
                    "compiler.reportCodeChange() was called even though nothing changed".into()
                },
            )?;
        }
        Ok(())
    }
    // port: CompilerTestCase#compareExpectedToActualAsTree
    fn compare_expected_to_actual_as_tree(
        &self,
        c: &CompilerHandle,
        main: NodeId,
        expected: (CompilerHandle, NodeId),
        mapping: &IndexMap<String, String>,
    ) -> Result<(), Throwable> {
        tree_equal_across(
            &c.borrow(),
            main,
            &expected.0.borrow(),
            expected.1,
            self.compare_js_doc,
            mapping,
        )
    }
    // port: CompilerTestCase#compareExpectedToActualAsStrings
    fn compare_expected_to_actual_as_strings(
        &self,
        c: &CompilerHandle,
        main: NodeId,
        expected: &[Arc<SourceFile>],
    ) -> Result<(), Throwable> {
        let actual = normalize_string_comparison(&passes::to_source(&mut c.borrow_mut(), main)?);
        let mut want = Vec::new();
        for file in expected {
            want.extend_from_slice(
                file.get_code()
                    .map_err(|_| Throwable::Exception {
                        class: "java.lang.RuntimeException".into(),
                        message: Some("failed to get source code".into()),
                    })?
                    .as_units(),
            );
        }
        assert_that(actual.as_units() == want, "compiled source differs")
    }
    // port: CompilerTestCase#validateNormalizationInvariants
    fn validate_normalization_invariants(
        &self,
        c: &CompilerHandle,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let main = root.get_last_child(&c.borrow()).unwrap();
        let clone = root.clone_tree(&mut c.borrow_mut());
        let e = clone.get_first_child(&c.borrow()).unwrap();
        let m = clone.get_last_child(&c.borrow()).unwrap();
        tree_equal(&c.borrow(), m, main, false)?;
        if self.normalize_enabled {
            passes::normalize(&mut c.borrow_mut(), e, m, true)?;
            tree_equal(&c.borrow(), m, main, false)?;
        }
        Ok(())
    }
    // port: CompilerTestCase#updateAccessorSummary
    fn update_accessor_summary(&self, c: &CompilerHandle, main: NodeId) -> Result<(), Throwable> {
        let root = main.get_parent(&c.borrow()).unwrap();
        let mut union = passes::gather_getter_and_setter_properties(&mut c.borrow_mut(), root)?;
        union.extend(self.declared_accessors.clone());
        passes::set_accessor_summary(&mut c.borrow_mut(), union)?;
        Ok(())
    }
    // port: CompilerTestCase#verifyAccessorSummary
    fn verify_accessor_summary(&self, c: &CompilerHandle, main: NodeId) -> Result<(), Throwable> {
        let Some(summary) = passes::accessor_summary(&c.borrow())? else {
            return Ok(());
        };
        let root = main.get_parent(&c.borrow()).unwrap();
        let gathered = passes::gather_getter_and_setter_properties(&mut c.borrow_mut(), root)?;
        assert_that(
            gathered
                .iter()
                .all(|(k, v)| summary.get(k).is_some_and(|s| s.equals(v))),
            "Pass created new getters/setters without calling GatherGetterAndSetterProperties.update",
        )
    }
    // port: CompilerTestCase#parseExpectedJs(List<SourceFile>)
    fn parse_expected_js(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        _actual: &CompilerHandle,
        inputs: &[Arc<SourceFile>],
        pipeline: Option<&ExpectedPipeline>,
    ) -> Result<(CompilerHandle, NodeId), Throwable> {
        let c = hooks.create_compiler(self)?;
        let options = hooks.get_options(self)?;
        c.borrow_mut()
            .init(&self.default_externs_inputs, inputs, options);
        let root = c.borrow_mut().parse_inputs();
        assert_that(
            root.is_some(),
            format!(
                "Unexpected parse error(s): {}",
                c.borrow()
                    .get_errors()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(LINE_JOINER)
            ),
        )?;
        let root = root.unwrap();
        let e = root.get_first_child(&c.borrow()).unwrap();
        let m = e.get_next(&c.borrow()).unwrap();
        let remove = pipeline.map_or(
            self.replace_types_with_colors || self.multistage_compilation,
            |p| p.remove_casts_expected,
        );
        let closure = pipeline.map_or(
            self.closure_pass_enabled && self.closure_pass_enabled_for_expected,
            |p| p.closure_pass_for_expected,
        );
        let rewrite = pipeline.map_or(self.rewrite_closure_code, |p| {
            p.closure_rewrite_module_for_expected
        });
        let provides = pipeline.map_or(
            self.rewrite_closure_provides && self.closure_pass_enabled_for_expected,
            |p| p.closure_provides_for_expected,
        );
        let transpile = pipeline.map_or(self.transpile_enabled, |p| p.transpile_expected);
        let normalize = pipeline.map_or(
            self.normalize_enabled && self.normalize_expected_output_enabled,
            |p| p.normalize_expected,
        );
        if remove {
            passes::remove_cast_nodes(&mut c.borrow_mut(), e, m)?;
        }
        if closure && !c.borrow().has_errors() {
            passes::gather_module_metadata(&mut c.borrow_mut(), e, m, false, "BROWSER")?;
            passes::process_closure_primitives(&mut c.borrow_mut(), e, m)?;
        }
        if rewrite {
            passes::closure_rewrite_module(&mut c.borrow_mut(), e, m, false)?;
            passes::scoped_aliases(&mut c.borrow_mut(), e, m)?;
        }
        if provides && !c.borrow().has_errors() {
            passes::process_closure_provides_and_requires(&mut c.borrow_mut(), e, m, false)?;
        }
        if transpile && !c.borrow().has_errors() {
            passes::rewrite_es_modules(&mut c.borrow_mut(), e, m)?;
            passes::transpile_to_es5(&mut c.borrow_mut(), e, m)?;
        } else if normalize && !c.borrow().has_errors() {
            normalize_actual_code(&c, e, m)?;
        }
        Ok((c, m))
    }
    // port: CompilerTestCase#ensureLibraryInjected (repetition)
    fn inject_libraries(&self, c: &CompilerHandle) -> Result<(), Throwable> {
        c.borrow_mut().reset_recent_change();
        for name in &self.libraries_to_inject {
            passes::ensure_library_injected(&mut c.borrow_mut(), name, true)?;
        }
        Ok(())
    }
    // port: CompilerTestCase#testExternChanges
    pub fn test_extern_changes(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: &Externs,
        inputs: &Sources,
        expected: &Expected,
        warnings: &[Diagnostic],
        pipeline: Option<&ExpectedPipeline>,
    ) -> Result<(), Throwable> {
        let c = hooks.create_compiler(self)?;
        let options = hooks.get_options(self)?;
        match inputs {
            Sources::Flat(f) => c.borrow_mut().init(&externs.externs, &f.sources, options),
            Sources::Chunks(ch) => c.borrow_mut().init_chunks(
                &externs.externs,
                ch.chunks.clone(),
                hooks.get_options(self)?,
            ),
            Sources::EncodedChunks(ch) => c.borrow_mut().init_chunks(
                &externs.externs,
                crate::replay::replay_values::chunks(ch)?,
                hooks.get_options(self)?,
            ),
        };
        self.test_extern_changes_internal(hooks, c, expected, Some(warnings), pipeline)
    }
    // port: CompilerTestCase#testExternChangesInternal
    pub fn test_extern_changes_internal(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        c: CompilerHandle,
        expected: &Expected,
        warnings: Option<&[Diagnostic]>,
        pipeline: Option<&ExpectedPipeline>,
    ) -> Result<(), Throwable> {
        c.borrow_mut().parse_inputs();
        let e = c.borrow().get_externs_root().unwrap();
        let m = c.borrow().get_js_root().unwrap();
        if self.create_module_map {
            passes::gather_module_metadata(&mut c.borrow_mut(), e, m, false, "BROWSER")?;
            passes::module_map_creator(&mut c.borrow_mut(), e, m)?;
        }
        assert_that(c.borrow().get_errors().is_empty(), "Unexpected errors")?;
        let expected_root = self.parse_expected_js(
            hooks,
            &c,
            expected
                .expected
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("expected externs missing".into()))?,
            pipeline,
        )?;
        assert_that(c.borrow().get_errors().is_empty(), "Unexpected errors")?;
        c.borrow_mut().before_pass(&hooks.get_name());
        let p = hooks.get_processor(c.clone())?;
        crate::replay::replay_dsl::process(&p, c.clone(), e, m, hooks.ctx())?;
        if self.compare_as_tree {
            if e.has_more_than_one_child(&c.borrow()) {
                let mut child = e.get_first_child(&c.borrow());
                while let Some(current) = child {
                    if !current.has_children(&c.borrow()) {
                        current.detach(&mut c.borrow_mut());
                    }
                    child = current.get_next(&c.borrow());
                }
            }
            check_state(e.is_root(&c.borrow()), "")?;
            expected_root.1.detach(&mut expected_root.0.borrow_mut());
            tree_equal_across(
                &c.borrow(),
                e,
                &expected_root.0.borrow(),
                expected_root.1,
                self.compare_js_doc,
                &IndexMap::<_, _>::default(),
            )?;
        } else {
            let actual = passes::to_source(&mut c.borrow_mut(), e)?;
            assert_that(
                actual
                    == passes::to_source_across(
                        &c.borrow(),
                        &expected_root.0.borrow(),
                        expected_root.1,
                    ),
                "extern source differs",
            )?;
        }
        let Some(warnings) = warnings else {
            return Ok(());
        };
        let got = c.borrow().get_warnings();
        let warning_message = got
            .iter()
            .map(|w| format!("{}\n", w.description()))
            .collect::<String>();
        assert_that(
            got.len() == warnings.len(),
            format!(
                "There should be {} warnings. {warning_message}",
                warnings.len()
            ),
        )?;
        for (a, b) in got.iter().zip(warnings) {
            assert_that(a.get_type() == b.diagnostic, &warning_message)?;
        }
        Ok(())
    }
    // port: CompilerTestCase#test(TestPart...)
    pub fn test(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        parts: Vec<TestPart>,
    ) -> Result<(), Throwable> {
        let (mut externs, mut sources, mut expected) = (None, None, None);
        let mut diagnostics = vec![];
        let mut posts = vec![];
        for part in parts {
            match part {
                TestPart::Externs(v) => {
                    check_state(externs.is_none(), "")?;
                    externs = Some(v);
                }
                TestPart::Sources(v) => {
                    check_state(sources.is_none(), "")?;
                    sources = Some(v);
                }
                TestPart::Expected(v) => {
                    check_state(expected.is_none(), "")?;
                    expected = Some(v);
                }
                TestPart::Diagnostic(v) => diagnostics.push(v),
                TestPart::Postcondition(v) => posts.push(v),
            }
        }
        let externs = externs.unwrap_or_else(|| Externs::new(self.default_externs_inputs.clone()));
        self.test_internal(
            hooks,
            &externs,
            &sources.ok_or_else(|| Throwable::HarnessError("no Sources test part".into()))?,
            expected.as_ref(),
            &diagnostics,
            &posts,
            None,
        )
    }
    // port: CompilerTestCase#testSame(TestPart...)
    pub fn test_same(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        mut parts: Vec<TestPart>,
    ) -> Result<(), Throwable> {
        parts.push(TestPart::Expected(Expected {
            expected: None,
            same: true,
        }));
        self.test(hooks, parts)
    }
}
impl Externs {
    // port: CompilerTestCase.Externs#Externs
    pub fn new(files: Vec<Arc<SourceFile>>) -> Self {
        for file in &files {
            file.set_kind(SourceKind::EXTERN);
        }
        Self { externs: files }
    }
}
impl FlatSources {
    // port: CompilerTestCase.FlatSources#FlatSources
    pub fn new(sources: Vec<Arc<SourceFile>>) -> Self {
        Self { sources }
    }
}
impl ChunkSources {
    // port: CompilerTestCase.ChunkSources#ChunkSources
    pub fn new(chunks: Vec<JSChunk>) -> Self {
        Self { chunks }
    }
}
impl Expected {
    // port: CompilerTestCase.Expected#Expected
    pub fn new(expected: Option<Vec<Arc<SourceFile>>>) -> Self {
        Self {
            expected,
            same: false,
        }
    }
}
impl CompilerTestCase {
    // port: CompilerTestCase#srcs(String)
    pub fn srcs(code: impl Into<JsString>) -> Sources {
        Sources::Flat(FlatSources::new(vec![Arc::new(SourceFile::from_code(
            GENERATED_SRC_NAME,
            code,
        ))]))
    }
    // port: CompilerTestCase#srcs(String...)
    pub fn srcs_strings(code: &[JsString]) -> Sources {
        Self::srcs_files(create_sources(GENERATED_SRC_NAME, code))
    }
    // port: CompilerTestCase#srcs(List<SourceFile>) / srcs(SourceFile...)
    pub fn srcs_files(files: Vec<Arc<SourceFile>>) -> Sources {
        Sources::Flat(FlatSources::new(files))
    }
    // port: CompilerTestCase#srcs(JSChunk...)
    pub fn srcs_chunks(chunks: Vec<JSChunk>) -> Sources {
        Sources::Chunks(ChunkSources::new(chunks))
    }
    // port: CompilerTestCase#expected(String)
    pub fn expected(code: impl Into<JsString>) -> Expected {
        Expected::new(Some(vec![Arc::new(SourceFile::from_code(
            GENERATED_SRC_NAME,
            code,
        ))]))
    }
    // port: CompilerTestCase#expected(String...)
    pub fn expected_strings(code: &[JsString]) -> Expected {
        Expected::new(Some(create_sources(GENERATED_SRC_NAME, code)))
    }
    // port: CompilerTestCase#expected(List<SourceFile>) / expected(SourceFile...)
    pub fn expected_files(files: Vec<Arc<SourceFile>>) -> Expected {
        Expected::new(Some(files))
    }
    // port: CompilerTestCase#expected(JSChunk[])
    pub fn expected_chunks(chunks: &[JSChunk]) -> Result<Expected, Throwable> {
        let mut codes = vec![];
        for chunk in chunks {
            for source in chunk.get_inputs() {
                codes.push(source.get_code().map_err(|e| Throwable::Exception {
                    class: "java.lang.RuntimeException".into(),
                    message: Some(format!("ouch: {e}")),
                })?);
            }
        }
        Ok(Self::expected_strings(&codes))
    }
    // port: CompilerTestCase#externs(String)
    pub fn externs(code: impl Into<JsString>) -> Externs {
        Externs::new(vec![Arc::new(SourceFile::from_code(
            GENERATED_EXTERNS_NAME,
            code,
        ))])
    }
    // port: CompilerTestCase#externs(String...)
    pub fn externs_strings(code: &[JsString]) -> Externs {
        Externs::new(create_sources(GENERATED_EXTERNS_NAME, code))
    }
    // port: CompilerTestCase#externs(List<SourceFile>)
    pub fn externs_files(files: Vec<Arc<SourceFile>>) -> Externs {
        Externs::new(files)
    }
    // port: CompilerTestCase#externs(SourceFile...)
    pub fn externs_file_array(files: &[Arc<SourceFile>]) -> Result<Externs, Throwable> {
        use crate::jscomp_api::StaticSourceFile;
        let copies = files
            .iter()
            .map(|f| {
                Ok(Arc::new(SourceFile::from_code(
                    f.get_name(),
                    f.get_code().map_err(|e| Throwable::Exception {
                        class: "java.lang.RuntimeException".into(),
                        message: Some(e.to_string()),
                    })?,
                )))
            })
            .collect::<Result<_, Throwable>>()?;
        Ok(Externs::new(copies))
    }
    // port: CompilerTestCase#warning / WarningDiagnostic#WarningDiagnostic
    pub fn warning(type_: &'static DiagnosticType) -> Diagnostic {
        WarningDiagnostic::new(type_).0
    }
    // port: CompilerTestCase#error / ErrorDiagnostic#ErrorDiagnostic
    pub fn error(type_: &'static DiagnosticType) -> Diagnostic {
        ErrorDiagnostic::new(type_).0
    }
    // port: CompilerTestCase#postcondition
    pub fn postcondition(postcondition: Postcondition) -> Postcondition {
        postcondition
    }
    // port: CompilerTestCase#test(String,String)
    pub fn test_strings(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        expected: impl Into<JsString>,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(Self::srcs(js)),
                TestPart::Expected(Self::expected(expected)),
            ],
        )
    }
    // port: CompilerTestCase#test(String,String,Diagnostic)
    pub fn test_strings_diagnostic(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        expected: impl Into<JsString>,
        diagnostic: Diagnostic,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Externs(Externs::new(self.default_externs_inputs.clone())),
                TestPart::Sources(Self::srcs(js)),
                TestPart::Expected(Self::expected(expected)),
                TestPart::Diagnostic(diagnostic),
            ],
        )
    }
    // port: CompilerTestCase#testError(String,DiagnosticType)
    pub fn test_error(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        error: &'static DiagnosticType,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(Self::srcs(js)),
                TestPart::Diagnostic(Self::error(error)),
            ],
        )
    }
    // port: CompilerTestCase#testError(String,DiagnosticType,String)
    pub fn test_error_message(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        error: &'static DiagnosticType,
        description: &str,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(Self::srcs(js)),
                TestPart::Diagnostic(Self::error(error).with_message(description)?),
            ],
        )
    }
    // port: CompilerTestCase#testError(Sources,Diagnostic)
    pub fn test_error_sources(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        sources: Sources,
        error: Diagnostic,
    ) -> Result<(), Throwable> {
        assert_that(
            error.level == CheckLevel::ERROR,
            "expected ERROR diagnostic",
        )?;
        self.test(
            hooks,
            vec![TestPart::Sources(sources), TestPart::Diagnostic(error)],
        )
    }
    // port: CompilerTestCase#testError(Externs,Sources,Diagnostic)
    pub fn test_error_externs(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: Externs,
        sources: Sources,
        error: Diagnostic,
    ) -> Result<(), Throwable> {
        assert_that(
            error.level == CheckLevel::ERROR,
            "expected ERROR diagnostic",
        )?;
        self.test(
            hooks,
            vec![
                TestPart::Externs(externs),
                TestPart::Sources(sources),
                TestPart::Diagnostic(error),
            ],
        )
    }
    // port: CompilerTestCase#testError(Sources,DiagnosticType)
    pub fn test_error_sources_type(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        sources: Sources,
        error: &'static DiagnosticType,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(sources),
                TestPart::Diagnostic(Self::error(error)),
            ],
        )
    }
    // port: CompilerTestCase#testWarning(String,DiagnosticType)
    pub fn test_warning(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        warning: &'static DiagnosticType,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(Self::srcs(js)),
                TestPart::Diagnostic(Self::warning(warning)),
            ],
        )
    }
    // port: CompilerTestCase#testWarning(Sources,Diagnostic)
    pub fn test_warning_sources(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        sources: Sources,
        warning: Diagnostic,
    ) -> Result<(), Throwable> {
        assert_that(
            warning.level == CheckLevel::WARNING,
            "expected WARNING diagnostic",
        )?;
        self.test(
            hooks,
            vec![TestPart::Sources(sources), TestPart::Diagnostic(warning)],
        )
    }
    // port: CompilerTestCase#testWarning(Externs,Sources,Diagnostic)
    pub fn test_warning_externs(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: Externs,
        sources: Sources,
        warning: Diagnostic,
    ) -> Result<(), Throwable> {
        assert_that(
            warning.level == CheckLevel::WARNING,
            "expected WARNING diagnostic",
        )?;
        self.test(
            hooks,
            vec![
                TestPart::Externs(externs),
                TestPart::Sources(sources),
                TestPart::Diagnostic(warning),
            ],
        )
    }
    // port: CompilerTestCase#testWarning(Sources,DiagnosticType)
    pub fn test_warning_sources_type(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        sources: Sources,
        warning: &'static DiagnosticType,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(sources),
                TestPart::Diagnostic(Self::warning(warning)),
            ],
        )
    }
    // port: CompilerTestCase#testWarning(String,DiagnosticType,String)
    pub fn test_warning_message(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        warning: &'static DiagnosticType,
        description: &str,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Sources(Self::srcs(js)),
                TestPart::Diagnostic(Self::warning(warning).with_message(description)?),
            ],
        )
    }
    // port: CompilerTestCase#testWarning(Externs,Sources,DiagnosticType,String)
    pub fn test_warning_externs_message(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: Externs,
        sources: Sources,
        warning: &'static DiagnosticType,
        description: &str,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![
                TestPart::Externs(externs),
                TestPart::Sources(sources),
                TestPart::Diagnostic(Self::warning(warning).with_message(description)?),
            ],
        )
    }
    // port: CompilerTestCase#testNoWarning(String)
    pub fn test_no_warning(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
    ) -> Result<(), Throwable> {
        self.test(hooks, vec![TestPart::Sources(Self::srcs(js))])
    }
    // port: CompilerTestCase#testNoWarning(Sources)
    pub fn test_no_warning_sources(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        sources: Sources,
    ) -> Result<(), Throwable> {
        self.test(hooks, vec![TestPart::Sources(sources)])
    }
    // port: CompilerTestCase#testNoWarning(Externs,Sources)
    pub fn test_no_warning_externs(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        externs: Externs,
        sources: Sources,
    ) -> Result<(), Throwable> {
        self.test(
            hooks,
            vec![TestPart::Externs(externs), TestPart::Sources(sources)],
        )
    }
    // port: CompilerTestCase#testSame(String)
    pub fn test_same_string(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
    ) -> Result<(), Throwable> {
        let js = js.into();
        self.test_strings(hooks, js.clone(), js)
    }
    // port: CompilerTestCase#testSame(String,DiagnosticType)
    pub fn test_same_warning(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        js: impl Into<JsString>,
        warning: &'static DiagnosticType,
    ) -> Result<(), Throwable> {
        let js = js.into();
        self.test_strings_diagnostic(hooks, js.clone(), js, Self::warning(warning))
    }
    // port: CompilerTestCase#testExternChanges(Sources,Expected,Diagnostic...)
    pub fn test_extern_changes_default(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        inputs: &Sources,
        expected: &Expected,
        warnings: &[Diagnostic],
    ) -> Result<(), Throwable> {
        self.test_extern_changes(
            hooks,
            &Externs::new(self.default_externs_inputs.clone()),
            inputs,
            expected,
            warnings,
            None,
        )
    }
    // port: CompilerTestCase#parseExpectedJs(String)
    pub fn parse_expected_js_string(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        actual: &CompilerHandle,
        code: impl Into<JsString>,
    ) -> Result<(CompilerHandle, NodeId), Throwable> {
        self.parse_expected_js(
            hooks,
            actual,
            Self::expected(code).expected.as_ref().unwrap(),
            None,
        )
    }
    // port: CompilerTestCase#parseExpectedJs(Expected)
    pub fn parse_expected_js_expected(
        &mut self,
        hooks: &mut impl CompilerTestCaseHooks,
        actual: &CompilerHandle,
        expected: &Expected,
    ) -> Result<(CompilerHandle, NodeId), Throwable> {
        self.parse_expected_js(
            hooks,
            actual,
            expected
                .expected
                .as_ref()
                .ok_or_else(|| Throwable::Exception {
                    class: "java.lang.NullPointerException".into(),
                    message: None,
                })?,
            None,
        )
    }
    // port: CompilerTestCase#findQualifiedNameNodes
    pub fn find_qualified_name_nodes(
        ast: &closure_rhino::node::Ast,
        name: &JsString,
        root: NodeId,
    ) -> Vec<NodeId> {
        fn visit(
            ast: &closure_rhino::node::Ast,
            name: &JsString,
            node: NodeId,
            matches: &mut Vec<NodeId>,
        ) {
            // port: CompilerTestCase#findQualifiedNameNodes (NodeUtil.visitPostOrder)
            for child in node.children(ast) {
                visit(ast, name, child, matches);
            }
            if node.matches_qualified_name(ast, name.clone()) {
                matches.push(node);
            }
        }
        let mut matches = vec![];
        visit(ast, name, root, &mut matches);
        matches
    }
    // port: CompilerTestCase#findQualifiedNameNode
    pub fn find_qualified_name_node(
        ast: &closure_rhino::node::Ast,
        name: &JsString,
        root: NodeId,
    ) -> Result<NodeId, Throwable> {
        Self::find_qualified_name_nodes(ast, name, root)
            .first()
            .copied()
            .ok_or_else(|| Throwable::Exception {
                class: "java.lang.IndexOutOfBoundsException".into(),
                message: Some("Index 0 out of bounds for length 0".into()),
            })
    }
    // port: CompilerTestCase#findDefinition
    #[expect(clippy::collapsible_match, reason = "Java switch control flow")]
    pub fn find_definition(compiler: &Compiler, name: &JsString) -> Option<NodeId> {
        use closure_rhino::token::Token;
        let mut next = compiler
            .get_js_root()
            .unwrap()
            .get_first_first_child(compiler);
        while let Some(node) = next {
            match node.get_token(compiler) {
                Token::FUNCTION => {
                    if node.get_first_child(compiler).unwrap().get_string(compiler) == *name {
                        return Some(node);
                    }
                }
                Token::VAR | Token::CONST | Token::LET => {
                    let first = node.get_first_child(compiler).unwrap();
                    if first.get_string(compiler) == *name {
                        return Some(first);
                    }
                }
                Token::EXPR_RESULT => {
                    let first = node.get_first_child(compiler).unwrap();
                    if first.is_assign(compiler) {
                        let lhs = first.get_first_child(compiler).unwrap();
                        if lhs.matches_qualified_name(compiler, name.clone()) {
                            return Some(lhs);
                        }
                    }
                }
                _ => {}
            }
            next = node.get_next(compiler);
        }
        None
    }
    // port: CompilerTestCase#makePassFactory
    pub fn make_pass_factory(
        name: String,
        factory: Rc<crate::replay::replay_dsl::Lambda>,
    ) -> DslValue {
        let (native, token) =
            crate::replay::native_factories::create(name.clone(), factory.clone());
        DslValue::PassFactory {
            name,
            factory,
            native,
            token,
        }
    }
}
// port: CompilerTestCase#createSources(String,List<String>)
pub fn create_sources(name: &str, sources: &[JsString]) -> Vec<Arc<SourceFile>> {
    sources
        .iter()
        .enumerate()
        .map(|(i, s)| Arc::new(SourceFile::from_code(&format!("{name}{i}"), s.clone())))
        .collect()
}
impl Diagnostic {
    // port: CompilerTestCase.Diagnostic#Diagnostic
    pub fn new(level: CheckLevel, diagnostic: &'static DiagnosticType) -> Self {
        Self {
            level,
            diagnostic,
            message_predicate: None,
            line: -1,
            charno: -1,
            length: -1,
        }
    }
    // port: CompilerTestCase.Diagnostic#formatDiff
    pub fn format_diff(&self, error: &JSError) -> Option<String> {
        if self.diagnostic != error.get_type() {
            return Some(format!(
                "diagnostic type <{}> did not match <{}>",
                error.get_type().key,
                self.diagnostic.key
            ));
        }
        if let Some(p) = &self.message_predicate
            && !p.apply(error.description())
        {
            return Some(format!("message <{}> was not <{p}>", error.description()));
        }
        for (name, want, got) in [
            ("line", self.line, error.lineno()),
            ("charno", self.charno, error.charno()),
            ("length", self.length, error.length()),
        ] {
            if want != -1 && got != want {
                return Some(format!("{name} <{got}> did not match <{want}>"));
            }
        }
        None
    }
    // port: CompilerTestCase.Diagnostic#withMessage
    pub fn with_message(mut self, expected: &str) -> Result<Self, Throwable> {
        check_state(self.message_predicate.is_none(), "")?;
        let expected = java_trim(expected).to_owned();
        let name = format!("\"{expected}\"");
        self.message_predicate = Some(Rc::new(NamedPredicate::of(
            move |actual: &str| java_trim(actual) == expected,
            name,
        )));
        self.line = -1;
        self.charno = -1;
        self.length = -1;
        Ok(self)
    }
    // port: CompilerTestCase.Diagnostic#withMessageContaining
    pub fn with_message_containing(mut self, substring: &str) -> Result<Self, Throwable> {
        check_state(self.message_predicate.is_none(), "")?;
        let substring = substring.to_owned();
        let name = format!("containing \"{substring}\"");
        self.message_predicate = Some(Rc::new(NamedPredicate::of(
            move |message: &str| message.contains(&substring),
            name,
        )));
        self.line = -1;
        self.charno = -1;
        self.length = -1;
        Ok(self)
    }
    // port: CompilerTestCase.Diagnostic#withLocation
    pub fn with_location(mut self, line: i32, charno: i32, length: i32) -> Self {
        self.line = line;
        self.charno = charno;
        self.length = length;
        self
    }
}
impl std::fmt::Display for Diagnostic {
    // port: CompilerTestCase.Diagnostic#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.diagnostic.key)?;
        if let Some(predicate) = &self.message_predicate {
            write!(f, " with message {predicate}")?;
        }
        Ok(())
    }
}
// port: String#trim
pub fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}
// port: CompilerTestCase#DIAGNOSTIC_CORRESPONDENCE (Truth containsExactlyElementsIn)
pub fn contains_exactly(
    actual: &[JSError],
    expected: &[Diagnostic],
    in_order: bool,
    label: &str,
) -> Result<(), Throwable> {
    contains_exactly_by(
        actual,
        expected,
        in_order,
        |a, b| b.format_diff(a).is_none(),
        label,
    )
}
// port: Correspondence#containsExactlyElementsIn (one-to-one matching, including duplicates)
pub fn contains_exactly_by<A, B>(
    actual: &[A],
    expected: &[B],
    in_order: bool,
    matches: impl Fn(&A, &B) -> bool,
    label: &str,
) -> Result<(), Throwable> {
    assert_that(
        actual.len() == expected.len(),
        format!(
            "{label}: expected {} diagnostics, got {}",
            expected.len(),
            actual.len()
        ),
    )?;
    if in_order {
        return assert_that(
            actual.iter().zip(expected).all(|(a, b)| matches(a, b)),
            format!("{label}: diagnostics differ in order"),
        );
    }
    let edges = actual
        .iter()
        .map(|a| expected.iter().map(|b| matches(a, b)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut assigned = vec![None; expected.len()];
    // A broad predicate cannot greedily consume the sole match of a later narrow predicate.
    for a in 0..actual.len() {
        let mut seen = vec![false; expected.len()];
        assert_that(
            pair(a, &edges, &mut assigned, &mut seen),
            format!("{label}: no one-to-one diagnostic pairing"),
        )?;
    }
    Ok(())
}
// port: Correspondence#containsExactlyElementsIn (bipartite pairing)
fn pair(a: usize, edges: &[Vec<bool>], assigned: &mut [Option<usize>], seen: &mut [bool]) -> bool {
    for b in 0..assigned.len() {
        if !edges[a][b] || seen[b] {
            continue;
        }
        seen[b] = true;
        if assigned[b].is_none() || pair(assigned[b].unwrap(), edges, assigned, seen) {
            assigned[b] = Some(a);
            return true;
        }
    }
    false
}
// port: CompilerTestCase#compareExpectedToActualAsStrings (replaceAll(" +\n", "\n"))
pub fn normalize_string_comparison(source: &JsString) -> JsString {
    let mut out = Vec::new();
    for c in source.as_units() {
        if *c == 10 {
            while out.last() == Some(&32) {
                out.pop();
            }
        }
        out.push(*c);
    }
    JsString::from_units(out)
}
// port: CompilerTestCase#createTypeCheck
fn create_type_check(c: &CompilerHandle, externs: NodeId, root: NodeId) -> Result<(), Throwable> {
    // Java builds the TypeCheck (which fetches the TypeValidator) before
    // setTypeCheckingHasRun(true); the seam keeps that order.
    passes::semantic_reverse_abstract_interpreter(&mut c.borrow_mut(), externs, root)?;
    passes::type_check(&mut c.borrow_mut(), externs, root, false)
}
// port: CompilerTestCase#normalizeActualCode
fn normalize_actual_code(
    c: &CompilerHandle,
    externs: NodeId,
    root: NodeId,
) -> Result<(), Throwable> {
    passes::normalize(&mut c.borrow_mut(), externs, root, false)
}
// port: CompilerTestCase.Postcondition#verify
fn verify_postconditions(
    posts: &[DslValue],
    c: CompilerHandle,
    ctx: &mut Ctx,
) -> Result<(), Throwable> {
    ctx.compiler = Some(c.clone());
    if !posts.is_empty() {
        ctx.postcondition_compiler = Some(c.clone());
    }
    for post in posts {
        if let DslValue::Native(o) = post {
            o.borrow_mut()
                .call("verify", vec![DslValue::Compiler(c.clone())])?;
        } else if let DslValue::Lambda(l) = post {
            crate::replay::replay_dsl::invoke_lambda(l, vec![DslValue::Compiler(c.clone())], ctx)?;
        } else {
            return Err(Throwable::Unported(format!("{}#verify", post.class_name())));
        }
    }
    Ok(())
}
// port: CompilerTestCase#createPrettyPrinter
fn create_pretty_printer(
    compiler: &Compiler,
) -> impl Fn(&closure_rhino::node::Ast, NodeId) -> String + use<> {
    let options = compiler.get_options().clone();
    move |ast, node| {
        closure_jscomp::code_printer::Builder::new(node)
            .set_compiler_options(&options)
            .set_pretty_print(true)
            .build(ast)
            .to_string_lossy()
    }
}
// port: ReplayValues#setField (option assignment by declared type)
pub fn set_option(o: &mut CompilerOptions, name: &str, value: &DslValue) -> Result<(), Throwable> {
    let raw = crate::replay::replay_values::encode(value)?;
    options_fields::set_field(
        o,
        name,
        &Value::from_json(&raw, "$").map_err(|e| Throwable::HarnessError(e.to_string()))?,
        &IndexMap::<_, _>::default(),
    )
}
// port: CompilerTestCase#fromSources
fn from_sources(srcs: &Sources) -> Result<Expected, Throwable> {
    Ok(Expected {
        expected: Some(match srcs {
            Sources::Flat(f) => f.sources.clone(),
            Sources::EncodedChunks(ch) => ch
                .iter()
                .flat_map(|c| c.inputs.iter().map(source_file))
                .collect(),
            Sources::Chunks(ch) => CompilerTestCase::expected_chunks(&ch.chunks)?
                .expected
                .unwrap(),
        }),
        same: false,
    })
}

// port: CompilerTestCase#compareExpectedToActualAsTree
pub fn tree_equal_across(
    actual_compiler: &Compiler,
    actual: NodeId,
    expected_ast: &closure_rhino::node::Ast,
    expected: NodeId,
    jsdoc: bool,
    mapping: &IndexMap<String, String>,
) -> Result<(), Throwable> {
    let subject = assert_node(actual)
        .using_serializer(create_pretty_printer(actual_compiler))
        .with_generic_name_replacements(mapping.clone());
    rhino::is_equal_to_internal_across(&subject, actual_compiler, expected_ast, expected, jsdoc)
}

#[cfg(test)]
mod tests {
    use super::*;
    // port: CompilerTestCase#validateCodeChangeReporting
    #[test]
    fn code_change_messages_include_the_original_and_new_trees() {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        let actual = Rc::new(RefCell::new(Compiler::new()));
        let original = Rc::new(RefCell::new(Compiler::new()));
        let main = actual.borrow_mut().new_string("after");
        let main_clone = original.borrow_mut().new_string("before");
        let externs = actual
            .borrow_mut()
            .new_node(closure_rhino::token::Token::ROOT);
        let externs_clone = original
            .borrow_mut()
            .new_node(closure_rhino::token::Token::ROOT);
        let failure = harness
            .validate_code_change_reporting(
                &actual,
                main,
                externs,
                main_clone,
                externs_clone,
                false,
                &original,
            )
            .unwrap_err();
        assert_eq!(failure, Throwable::Assertion { message: "compiler.reportCodeChange() should have been called.\nOriginal: STRINGLIT before\n\nNew: STRINGLIT after\n".into() });
        main.set_string(&mut actual.borrow_mut(), "before");
        let failure = harness
            .validate_code_change_reporting(
                &actual,
                main,
                externs,
                main_clone,
                externs_clone,
                true,
                &original,
            )
            .unwrap_err();
        assert_eq!(
            failure,
            Throwable::Assertion {
                message: "compiler.reportCodeChange() was called even though nothing changed"
                    .into()
            }
        );
    }
}
