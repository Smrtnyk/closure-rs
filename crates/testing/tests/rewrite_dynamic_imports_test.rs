/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/RewriteDynamicImportsTest.java.

//! Rust unit test of RewriteDynamicImportsTest.java that calls `getProcessor(compiler).process`
//! directly instead of a hooked CompilerTestCase method, so the corpus has no record of it. The
//! recorded tests of the class replay in `es_modules_records.rs`; getProcessor is the corpus helper
//! `RewriteDynamicImportsTest_Helpers.GetProcessor`
//! (`crates/testing/src/replay/rewrite_dynamic_imports_test_helpers.rs`).
use closure_jscomp::{
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    gather_module_metadata::GatherModuleMetadata,
    modules::module_map_creator::ModuleMapCreator,
    source_file::SourceFile,
};
use closure_rhino::fx_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, GENERATED_EXTERNS_NAME, GENERATED_SRC_NAME},
    replay::{
        registry::Registry,
        replay_dsl::{Ctx, DslValue},
        replay_values::object,
        rewrite_dynamic_imports_test_helpers,
    },
    testing::test_externs_builder::TestExternsBuilder,
};
use std::sync::Arc;

/// The fields of RewriteDynamicImportsTest that aliasExternInjectedSimpleImport sets and reads.
struct RewriteDynamicImportsTest {
    harness: CompilerTestCase,
    language: Option<LanguageMode>,
    language_in: Option<LanguageMode>,
    /// The holder of `dynamicImportAlias` and `chunkOutputType` that getProcessor reads.
    fields: DslValue,
    ctx: Ctx,
}

impl RewriteDynamicImportsTest {
    // port: RewriteDynamicImportsTest#RewriteDynamicImportsTest
    fn new() -> Self {
        let mut ctx = Ctx::new(
            "RewriteDynamicImportsTest".into(),
            object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        );
        // private @Nullable String dynamicImportAlias = "imprt_";
        // private ChunkOutputType chunkOutputType = ChunkOutputType.GLOBAL_NAMESPACE;
        let fields = rewrite_dynamic_imports_test_helpers::holder(&mut ctx, vec![]).unwrap();
        let mut test = Self {
            harness: CompilerTestCase::new(TestExternsBuilder::new().add_promise().build()),
            language: None,
            language_in: None,
            fields,
            ctx,
        };
        test.set_up();
        test
    }

    // port: RewriteDynamicImportsTest#setUp
    fn set_up(&mut self) {
        self.harness.set_up();
        self.harness.enable_create_module_map().unwrap();
        self.harness.enable_type_info_validation().unwrap();
    }

    // port: RewriteDynamicImportsTest#getOptions
    fn get_options(&self) -> CompilerOptions {
        let mut options = self.harness.get_options().unwrap();
        options.set_pretty_print(true);
        if let Some(language) = self.language {
            options.set_language(language);
        }
        if let Some(language_in) = self.language_in {
            options.set_language_in(language_in);
        }
        options
    }

    fn set_dynamic_import_alias(&mut self, alias: &str) {
        let DslValue::Object(holder) = &self.fields else {
            unreachable!("the holder is an object");
        };
        holder
            .borrow_mut()
            .fields
            .insert("dynamicImportAlias".into(), DslValue::String(alias.into()));
    }

    // port: RewriteDynamicImportsTest#getProcessor
    fn get_processor(&mut self, compiler: DslValue) -> DslValue {
        let get_processor = rewrite_dynamic_imports_test_helpers::get_processor_init(
            &mut self.ctx,
            vec![self.fields.clone()],
        )
        .unwrap();
        rewrite_dynamic_imports_test_helpers::get_processor(
            &mut self.ctx,
            vec![get_processor, compiler],
        )
        .unwrap()
    }
}

// port: RewriteDynamicImportsTest#aliasExternInjectedSimpleImport
#[test]
fn alias_extern_injected_simple_import() {
    let mut t = RewriteDynamicImportsTest::new();
    t.language = Some(LanguageMode::ECMASCRIPT_2015);
    t.language_in = Some(LanguageMode::ECMASCRIPT_NEXT);
    t.set_dynamic_import_alias("import");

    // "import" is not a valid JS identifier name and will not parse.
    // Instead, we have to manually check the AST for the expected name.
    let compiler = t.harness.create_compiler().unwrap();
    let options = t.get_options();
    compiler.borrow_mut().init(
        &[Arc::new(SourceFile::from_code(GENERATED_EXTERNS_NAME, ""))],
        &[Arc::new(SourceFile::from_code(
            GENERATED_SRC_NAME,
            "var external = 'url'; const nsPromise = import(external);",
        ))],
        options,
    );
    compiler.borrow_mut().parse_inputs();
    {
        let mut c = compiler.borrow_mut();
        let externs_root = c.get_externs_root().unwrap();
        let js_root = c.get_js_root().unwrap();
        GatherModuleMetadata::new(
            /* processCommonJsModules= */ false,
            ResolutionMode::BROWSER,
        )
        .process(&mut c, externs_root, js_root);
        let module_metadata_map = c.get_module_metadata_map().cloned().unwrap();
        ModuleMapCreator::new(module_metadata_map).process(&mut c, externs_root, js_root);
        assert!(c.get_errors().is_empty(), "{:?}", c.get_errors());
    }
    let (externs, root) = {
        let c = compiler.borrow();
        let externs_and_js = c.get_root().unwrap();
        (
            externs_and_js.get_first_child(&c).unwrap(),
            externs_and_js.get_last_child(&c).unwrap(),
        )
    };
    compiler
        .borrow_mut()
        .before_pass("RewriteDynamicImportsTest");
    let DslValue::Native(pass) = t.get_processor(DslValue::Compiler(compiler.clone())) else {
        panic!("getProcessor returns a CompilerPass");
    };
    pass.borrow_mut()
        .process(&mut compiler.borrow_mut(), externs, root)
        .unwrap();

    let c = compiler.borrow();
    let synthetic_externs = externs.get_first_child(&c);
    assert!(synthetic_externs.is_some());
    let synthetic_externs = synthetic_externs.unwrap();
    assert!(synthetic_externs.is_script(&c));
    let injected_alias = synthetic_externs.get_first_child(&c);
    assert!(injected_alias.is_some());
    let injected_alias = injected_alias.unwrap();
    assert!(injected_alias.is_function(&c));
    assert!(injected_alias.get_first_child(&c).unwrap().is_name(&c));
    assert_eq!(
        injected_alias.get_first_child(&c).unwrap().get_string(&c),
        "import"
    );
}
