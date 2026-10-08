/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/GatherModuleMetadataTest.java.

//! Port of GatherModuleMetadataTest. CompilerTestCase's test/testSame/testError are reproduced
//! with the direct Compiler API (as in whitespace_wrap_goog_modules_test.rs): one empty externs
//! file "externs", languageIn UNSUPPORTED (CompilerTestCase's default accepted language),
//! languageOut ECMASCRIPT5 (setUp), one source named "testcode" or several named "testcode<i>".
use closure_jscomp::{
    Compiler,
    closure_primitive_errors::{
        DUPLICATE_MODULE, DUPLICATE_NAMESPACE, DUPLICATE_NAMESPACE_AND_MODULE,
    },
    closure_rewrite_module::INVALID_MODULE_ID_ARG,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    dependency_options::DependencyOptions,
    deps::module_loader::ResolutionMode,
    diagnostic_type::DiagnosticType,
    es6_rewrite_scripts_to_modules::Es6RewriteScriptsToModules,
    gather_module_metadata::{
        GatherModuleMetadata, INVALID_MAYBE_REQUIRE, INVALID_NAMESPACE_OR_MODULE_ID,
        INVALID_REQUIRE_DYNAMIC, INVALID_SET_TEST_ONLY, INVALID_TOGGLE_USAGE,
    },
    module_identifier::ModuleIdentifier,
    modules::module_metadata_map::{ModuleMetadata, ModuleMetadataMap, ModuleType},
    source_file::SourceFile,
};
use closure_rhino::js_string::JsString;
use std::sync::Arc;

struct GatherModuleMetadataTest {
    rewrite_scripts_to_modules: bool,
    sort_only: bool,
    entry_points: Vec<ModuleIdentifier>,
    accepted_language: LanguageMode,
    last_compiler: Option<Compiler>,
}

impl GatherModuleMetadataTest {
    // port: GatherModuleMetadataTest#setUp
    fn set_up() -> Self {
        Self {
            entry_points: Vec::new(),
            rewrite_scripts_to_modules: false,
            sort_only: false,
            accepted_language: LanguageMode::UNSUPPORTED,
            last_compiler: None,
        }
    }

    fn run(&mut self, srcs: &[&str]) -> Vec<&'static str> {
        let files: Vec<(String, &str)> = if srcs.len() == 1 {
            vec![("testcode".to_string(), srcs[0])]
        } else {
            srcs.iter()
                .enumerate()
                .map(|(i, s)| (format!("testcode{i}"), *s))
                .collect()
        };
        let files: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), *c)).collect();
        self.run_files(&files)
            .into_iter()
            .map(|(key, _)| key)
            .collect()
    }

    fn run_files(&mut self, files: &[(&str, &str)]) -> Vec<(&'static str, String)> {
        self.run_files_with_externs("", files)
    }

    // port: GatherModuleMetadataTest#getProcessor
    fn run_files_with_externs(
        &mut self,
        externs_code: &str,
        files: &[(&str, &str)],
    ) -> Vec<(&'static str, String)> {
        let mut options = CompilerOptions::new();
        options.set_language_in(self.accepted_language);
        options.set_language_out(LanguageMode::ECMASCRIPT5);
        // port: GatherModuleMetadataTest#getOptions
        if !self.entry_points.is_empty() {
            assert!(
                !self.sort_only,
                "sortOnly must be false if entry points are provided."
            );
            options.set_dependency_options(DependencyOptions::prune_for_entry_points(
                self.entry_points.clone(),
            ));
        } else if self.sort_only {
            options.set_dependency_options(DependencyOptions::sort_only());
        }
        let inputs: Vec<Arc<SourceFile>> = files
            .iter()
            .map(|(name, code)| Arc::new(SourceFile::from_code(name, *code)))
            .collect();
        let mut compiler = Compiler::new();
        compiler.init(
            &[Arc::new(SourceFile::from_code("externs", externs_code))],
            &inputs,
            options,
        );
        compiler.parse_inputs().unwrap();
        assert!(compiler.get_errors().is_empty(), "parse errors");
        let externs = compiler.get_externs_root().unwrap();
        let root = compiler.get_js_root().unwrap();
        if self.rewrite_scripts_to_modules {
            Es6RewriteScriptsToModules::new().process(&mut compiler, externs, root);
        }
        GatherModuleMetadata::new(true, ResolutionMode::BROWSER).process(
            &mut compiler,
            externs,
            root,
        );
        let mut found: Vec<(&'static str, String)> = compiler
            .get_errors()
            .iter()
            .map(|e| (e.get_type().key, e.get_description().to_string()))
            .collect();
        found.extend(
            compiler
                .get_warnings()
                .iter()
                .map(|e| (e.get_type().key, e.get_description().to_string())),
        );
        self.last_compiler = Some(compiler);
        found
    }

    /// `test(srcs, error(type).withMessageContaining(text))`.
    fn test_error_with_message_containing(
        &mut self,
        srcs: &[&str],
        error: &DiagnosticType,
        text: &str,
    ) {
        let found = self.run(srcs);
        assert_eq!(found, vec![error.key]);
        let description = &self.last_compiler.as_ref().unwrap().get_errors()[0];
        assert!(
            description.get_description().contains(text),
            "{}",
            description.get_description()
        );
    }

    fn test_same(&mut self, srcs: &[&str]) {
        assert_eq!(self.run(srcs), Vec::<&str>::new());
    }

    fn test_error(&mut self, srcs: &[&str], error: &DiagnosticType) {
        assert_eq!(self.run(srcs), vec![error.key]);
    }

    // port: GatherModuleMetadataTest#metadataMap
    fn metadata_map(&self) -> &ModuleMetadataMap {
        self.last_compiler
            .as_ref()
            .unwrap()
            .get_module_metadata_map()
            .unwrap()
    }

    fn by_namespace(&self, namespace: &str) -> Arc<ModuleMetadata> {
        self.metadata_map()
            .get_modules_by_goog_namespace()
            .get(&JsString::from(namespace))
            .unwrap()
            .clone()
    }

    fn by_path(&self, path: &str) -> Arc<ModuleMetadata> {
        self.metadata_map()
            .get_modules_by_path()
            .get(path)
            .unwrap()
            .clone()
    }

    fn namespace_keys(&self) -> Vec<String> {
        self.metadata_map()
            .get_modules_by_goog_namespace()
            .keys()
            .map(JsString::to_string_lossy)
            .collect()
    }
}

/// `assertThat(multiset).containsExactly(...)` (order-insensitive, counts respected).
fn strings(set: &closure_jscomp::modules::module_metadata_map::Multiset<JsString>) -> Vec<String> {
    let mut v: Vec<String> = set.iter().map(JsString::to_string_lossy).collect();
    v.sort();
    v
}

fn exactly(items: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}

fn keys_exactly(keys: Vec<String>, items: &[&str]) {
    let mut keys = keys;
    keys.sort();
    assert_eq!(keys, exactly(items));
}

// port: GatherModuleMetadataTest#testGoogProvide
#[test]
fn test_goog_provide() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.provide('my.provide');"]);
    keys_exactly(t.namespace_keys(), &["my.provide"]);
    assert!(
        t.metadata_map()
            .get_modules_by_path()
            .contains_key("testcode")
    );
    let m = t.by_namespace("my.provide");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.provide"]));
    assert!(m.is_goog_provide());
}

// port: GatherModuleMetadataTest#testGoogProvideWithGoogDeclaredInOtherFile
#[test]
fn test_goog_provide_with_goog_declared_in_other_file() {
    // Closure's base.js declare the global goog. It should be ignored when scanning the provide'd
    // file. Only local variables named goog should cause the pass to back off.
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["var goog;", "goog.provide('my.provide');"]);
    keys_exactly(t.namespace_keys(), &["my.provide"]);
    let m = t.by_namespace("my.provide");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.provide"]));
    assert!(m.is_goog_provide());
}

// port: GatherModuleMetadataTest#testLocalGoogIsIgnored
#[test]
fn test_local_goog_is_ignored() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["function bar(goog) { goog.provide('my.provide'); }"]);
    keys_exactly(t.namespace_keys(), &[]);
    assert!(!t.by_path("testcode").uses_closure());
}

// port: GatherModuleMetadataTest#testMultipleGoogProvide
#[test]
fn test_multiple_goog_provide() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.provide('my.first.provide'); goog.provide('my.second.provide');"]);
    keys_exactly(
        t.namespace_keys(),
        &["my.first.provide", "my.second.provide"],
    );
    assert!(
        t.metadata_map()
            .get_modules_by_path()
            .contains_key("testcode")
    );
    let m = t.by_namespace("my.first.provide");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["my.first.provide", "my.second.provide"])
    );
    assert!(m.is_goog_provide());

    let m = t.by_namespace("my.second.provide");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["my.first.provide", "my.second.provide"])
    );
    assert!(m.is_goog_provide());

    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["my.first.provide", "my.second.provide"])
    );
    assert!(m.is_goog_provide());
}

// port: GatherModuleMetadataTest#testProvideNamespaceValidation
#[test]
fn test_provide_namespace_validation() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(&["goog.provide('');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(&["goog.provide(' ');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(&["goog.provide('a..b');"], &INVALID_NAMESPACE_OR_MODULE_ID);

    t.test_error(
        &["goog.provide('\u{0101}');"],
        &INVALID_NAMESPACE_OR_MODULE_ID,
    );
    t.test_same(&["goog.provide('a');"]);

    t.test_same(&["goog.provide('a.class');"]);
    t.test_error(
        &["goog.provide('class.a');"],
        &INVALID_NAMESPACE_OR_MODULE_ID,
    );

    t.accepted_language = LanguageMode::ECMASCRIPT3;
    t.test_error(
        &["goog.provide('a.class');"],
        &INVALID_NAMESPACE_OR_MODULE_ID,
    );
    t.test_error(
        &["goog.provide('class.a');"],
        &INVALID_NAMESPACE_OR_MODULE_ID,
    );
}

// port: GatherModuleMetadataTest#testGoogModule
#[test]
fn test_goog_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.module('my.module');"]);
    keys_exactly(t.namespace_keys(), &["my.module"]);
    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(!m.is_goog_provide());
    assert!(m.is_goog_module());
    assert!(m.is_non_legacy_goog_module());
    assert!(!m.is_legacy_goog_module());
}

// port: GatherModuleMetadataTest#testModuleIdValidation
#[test]
fn test_module_id_validation() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(&["goog.module();"], &INVALID_MODULE_ID_ARG);
    t.test_error(&["goog.module('');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(&["goog.module(' ');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(&["goog.module('a..b');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(&["goog.module('a. .b');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(&["goog.module('a.-.b');"], &INVALID_NAMESPACE_OR_MODULE_ID);

    t.test_error(&["goog.module('0');"], &INVALID_NAMESPACE_OR_MODULE_ID);
    t.test_error(
        &["goog.module('\u{0101}');"],
        &INVALID_NAMESPACE_OR_MODULE_ID,
    );

    t.test_same(&["goog.module('a');"]);
    t.test_same(&["goog.module('a0');"]);
    t.test_same(&["goog.module('$');"]);
}

// port: GatherModuleMetadataTest#testGoogModuleWithDefaultExport
#[test]
fn test_goog_module_with_default_export() {
    // exports = 0; on its own is CommonJS!
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.module('my.module'); exports = 0;"]);
    keys_exactly(t.namespace_keys(), &["my.module"]);
    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(!m.is_goog_provide());
    assert!(m.is_goog_module());
    assert!(m.is_non_legacy_goog_module());
    assert!(!m.is_legacy_goog_module());
}

// port: GatherModuleMetadataTest#testLegacyGoogModule
#[test]
fn test_legacy_goog_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.module('my.module'); goog.module.declareLegacyNamespace();"]);
    keys_exactly(t.namespace_keys(), &["my.module"]);
    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(!m.is_goog_provide());
    assert!(m.is_goog_module());
    assert!(!m.is_non_legacy_goog_module());
    assert!(m.is_legacy_goog_module());
}

fn assert_load_module_script(t: &GatherModuleMetadataTest) {
    let m = t.by_path("testcode");
    assert!(m.goog_namespaces().is_empty());
    assert!(m.is_non_provide_script());
}

// port: GatherModuleMetadataTest#testLoadModule
#[test]
fn test_load_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&[r"goog.loadModule(function(exports) {
  goog.module('my.module');
  return exports;
});
"]);

    keys_exactly(t.namespace_keys(), &["my.module"]);

    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());

    assert_load_module_script(&t);
}

// port: GatherModuleMetadataTest#testLoadModuleLegacyNamespace
#[test]
fn test_load_module_legacy_namespace() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&[r"goog.loadModule(function(exports) {
  goog.module('my.module');
  goog.module.declareLegacyNamespace();
  return exports;
});
"]);

    keys_exactly(t.namespace_keys(), &["my.module"]);

    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(m.is_legacy_goog_module());
    assert!(m.path().is_none());

    assert_load_module_script(&t);
}

// port: GatherModuleMetadataTest#testLoadModuleUseStrict
#[test]
fn test_load_module_use_strict() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&[r"goog.loadModule(function(exports) {
  'use strict';
  goog.module('with.strict');
  return exports;
});
"]);

    keys_exactly(t.namespace_keys(), &["with.strict"]);

    let m = t.by_namespace("with.strict");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["with.strict"]));
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());

    assert_load_module_script(&t);
}

// port: GatherModuleMetadataTest#testMultipleGoogModuleCallsInLoadModule
#[test]
fn test_multiple_goog_module_calls_in_load_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&[r"goog.loadModule(function(exports) {
  goog.module('multiple.calls.c0');
  goog.module('multiple.calls.c1');
  return exports;
});
"]);

    keys_exactly(
        t.namespace_keys(),
        &["multiple.calls.c0", "multiple.calls.c1"],
    );

    let m = t.by_namespace("multiple.calls.c0");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["multiple.calls.c0", "multiple.calls.c1"])
    );
    assert!(Arc::ptr_eq(&t.by_namespace("multiple.calls.c1"), &m));
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());

    assert_load_module_script(&t);
}

// port: GatherModuleMetadataTest#testMultipleGoogLoadModules
#[test]
fn test_multiple_goog_load_modules() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&[r"goog.loadModule(function(exports) {
  goog.module('multiple.calls.c0');
  return exports;
});

goog.loadModule(function(exports) {
  goog.module('multiple.calls.c1');
  return exports;
});
"]);

    keys_exactly(
        t.namespace_keys(),
        &["multiple.calls.c0", "multiple.calls.c1"],
    );

    let m = t.by_namespace("multiple.calls.c0");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["multiple.calls.c0"])
    );
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());

    let m = t.by_namespace("multiple.calls.c1");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["multiple.calls.c1"])
    );
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());

    assert_load_module_script(&t);
}

const BUNDLE_LOAD_MODULES: &str = r"goog.provide('some.provide');

goog.provide('some.other.provide');

goog.loadModule(function(exports) {
  goog.module('multiple.calls.c0');
  return exports;
});

goog.loadModule(function(exports) {
  goog.module('multiple.calls.c1');
  return exports;
});
";

fn assert_bundle(t: &GatherModuleMetadataTest, uses_closure: bool) {
    keys_exactly(
        t.namespace_keys(),
        &[
            "some.provide",
            "some.other.provide",
            "multiple.calls.c0",
            "multiple.calls.c1",
        ],
    );

    let m = t.by_namespace("multiple.calls.c0");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["multiple.calls.c0"])
    );
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());
    assert_eq!(m.uses_closure(), uses_closure);

    let m = t.by_namespace("multiple.calls.c1");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["multiple.calls.c1"])
    );
    assert!(m.is_non_legacy_goog_module());
    assert!(m.path().is_none());
    assert_eq!(m.uses_closure(), uses_closure);

    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.goog_namespaces()),
        exactly(&["some.provide", "some.other.provide"])
    );
    assert!(m.is_goog_provide());
    assert_eq!(m.uses_closure(), uses_closure);
}

// port: GatherModuleMetadataTest#testBundleGoogLoadModuleAndProvides
#[test]
fn test_bundle_goog_load_module_and_provides() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&[BUNDLE_LOAD_MODULES]);
    assert_bundle(&t, true);
}

// port: GatherModuleMetadataTest#testBundleGoogLoadModuleAndProvidesWithGoogDefined
#[test]
fn test_bundle_goog_load_module_and_provides_with_goog_defined() {
    let mut t = GatherModuleMetadataTest::set_up();
    let src = format!("/** @provideGoog */\nvar goog = {{}};\n\n{BUNDLE_LOAD_MODULES}");
    t.test_same(&[&src]);
    assert_bundle(&t, false);
}

// port: GatherModuleMetadataTest#testEs6Module
#[test]
fn test_es6_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["export var x;"]);
    keys_exactly(t.namespace_keys(), &[]);
    assert!(
        t.metadata_map()
            .get_modules_by_path()
            .contains_key("testcode")
    );
    let m = t.by_path("testcode");
    assert!(m.goog_namespaces().is_empty());
    assert!(m.is_es6_module());
}

// port: GatherModuleMetadataTest#testEs6ModuleDeclareModuleId
#[test]
fn test_es6_module_declare_module_id() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["export var x; goog.declareModuleId('my.module');"]);
    keys_exactly(t.namespace_keys(), &["my.module"]);
    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(m.is_es6_module());
    assert!(!m.is_goog_module());
}

// port: GatherModuleMetadataTest#testEs6ModuleDeclareModuleIdImportedGoog
#[test]
fn test_es6_module_declare_module_id_imported_goog() {
    let mut t = GatherModuleMetadataTest::set_up();
    let found = t.run_files(&[
        ("goog.js", ""),
        (
            "testcode",
            "import * as goog from './goog.js';\nexport var x;\ngoog.declareModuleId('my.module');\n",
        ),
    ]);
    assert!(found.is_empty(), "{found:?}");
    keys_exactly(t.namespace_keys(), &["my.module"]);
    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(m.is_es6_module());
    assert!(!m.is_goog_module());
}

// port: GatherModuleMetadataTest#testCommonJsModule
#[test]
fn test_common_js_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["exports = 0;"]);
    assert!(t.metadata_map().get_modules_by_goog_namespace().is_empty());
    let m = t.by_path("testcode");
    assert!(m.is_common_js());
}

// port: GatherModuleMetadataTest#testDuplicateProvides
#[test]
fn test_duplicate_provides() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.provide('duplciated');", "goog.provide('duplciated');"],
        &DUPLICATE_NAMESPACE,
    );
}

// port: GatherModuleMetadataTest#testDuplicateProvidesInSameFile
#[test]
fn test_duplicate_provides_in_same_file() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.provide('duplciated');\ngoog.provide('duplciated');"],
        &DUPLICATE_NAMESPACE,
    );
}

// port: GatherModuleMetadataTest#testDuplicateProvideAndGoogModule
#[test]
fn test_duplicate_provide_and_goog_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.provide('duplciated');", "goog.module('duplciated');"],
        &DUPLICATE_NAMESPACE_AND_MODULE,
    );
    t.test_error(
        &["goog.module('duplciated');", "goog.provide('duplciated');"],
        &DUPLICATE_NAMESPACE_AND_MODULE,
    );
}

// port: GatherModuleMetadataTest#testDuplicateProvideAndEs6Module
#[test]
fn test_duplicate_provide_and_es6_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &[
            "goog.provide('duplciated');",
            "export {}; goog.declareModuleId('duplciated');",
        ],
        &DUPLICATE_NAMESPACE_AND_MODULE,
    );
    t.test_error(
        &[
            "export {}; goog.declareModuleId('duplciated');",
            "goog.provide('duplciated');",
        ],
        &DUPLICATE_NAMESPACE_AND_MODULE,
    );
}

// port: GatherModuleMetadataTest#testDuplicateGoogModules
#[test]
fn test_duplicate_goog_modules() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.module('duplciated');", "goog.module('duplciated');"],
        &DUPLICATE_MODULE,
    );
}

// port: GatherModuleMetadataTest#testDuplicateGoogAndEs6Module
#[test]
fn test_duplicate_goog_and_es6_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &[
            "goog.module('duplciated');",
            "export {}; goog.declareModuleId('duplciated');",
        ],
        &DUPLICATE_MODULE,
    );
    t.test_error(
        &[
            "export {}; goog.declareModuleId('duplciated');",
            "goog.module('duplciated');",
        ],
        &DUPLICATE_MODULE,
    );
}

// port: GatherModuleMetadataTest#testDuplicatEs6Modules
#[test]
fn test_duplicat_es6_modules() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &[
            "export {}; goog.declareModuleId('duplciated');",
            "export {}; goog.declareModuleId('duplciated');",
        ],
        &DUPLICATE_MODULE,
    );
    t.test_error(
        &[
            "export {}; goog.declareModuleId('duplciated');",
            "export {}; goog.declareModuleId('duplciated');",
        ],
        &DUPLICATE_MODULE,
    );
}

// port: GatherModuleMetadataTest#testDuplicateModuleWarningsIncludeFileName
#[test]
fn test_duplicate_module_warnings_include_file_name() {
    let mut t = GatherModuleMetadataTest::set_up();
    // duplicates in the same file
    t.test_error_with_message_containing(
        &["goog.provide('duplicated'); goog.provide('duplicated')"],
        &DUPLICATE_NAMESPACE,
        "testcode",
    );
    // duplicate of provide in earlier file
    t.test_error_with_message_containing(
        &["goog.provide('duplicated');", "goog.module('duplicated')"],
        &DUPLICATE_NAMESPACE_AND_MODULE,
        "testcode0",
    );
    // duplicate of module in earlier file
    t.test_error_with_message_containing(
        &["goog.module('duplicated');", "goog.module('duplicated')"],
        &DUPLICATE_MODULE,
        "testcode0",
    );
}

// port: GatherModuleMetadataTest#testUsesGlobalClosure
#[test]
fn test_uses_global_closure() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.isArray(foo);"]);
    assert!(t.by_path("testcode").uses_closure());
}

// port: GatherModuleMetadataTest#testUsesGlobalClosureNoFunctionCall
#[test]
fn test_uses_global_closure_no_function_call() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["var b = goog.isArray;"]);
    assert!(t.by_path("testcode").uses_closure());
}

// port: GatherModuleMetadataTest#testLocalGoogIsNotClosure
#[test]
fn test_local_goog_is_not_closure() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["function bar() { var goog; goog.isArray(foo); }"]);
    assert!(!t.by_path("testcode").uses_closure());
}

// port: GatherModuleMetadataTest#testGoogInSameScriptGoogIsNotClosure
#[test]
fn test_goog_in_same_script_goog_is_not_closure() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["/** @provideGoog */ var goog = {}; goog.isArray(foo);"]);
    assert!(!t.by_path("testcode").uses_closure());
}

// port: GatherModuleMetadataTest#testGoogInOtherScriptGoogIsClosure
#[test]
fn test_goog_in_other_script_goog_is_closure() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["/** @provideGoog */ var goog = {};", "goog.isArray(foo);"]);
    assert!(t.by_path("testcode1").uses_closure());
}

// port: GatherModuleMetadataTest#testImportedGoogIsClosure
#[test]
fn test_imported_goog_is_closure() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["import * as goog from '/goog.js'; goog.isArray(foo);"]);
    assert!(t.by_path("testcode").uses_closure());
}

// port: GatherModuleMetadataTest#testRequireType
#[test]
fn test_require_type() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.requireType('my.Type');"]);
    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.weakly_required_goog_namespaces()),
        exactly(&["my.Type"])
    );
}

// port: GatherModuleMetadataTest#testRequireDynamic
#[test]
fn test_require_dynamic() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["async function test() {await goog.requireDynamic('my.Type');}"]);
    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.dynamically_required_goog_namespaces()),
        exactly(&["my.Type"])
    );
}

// port: GatherModuleMetadataTest#testRequireDynamicWithIllegalArg
#[test]
fn test_require_dynamic_with_illegal_arg() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["async function test() {await goog.requireDynamic('my.Type','extra.Type');}"],
        &INVALID_REQUIRE_DYNAMIC,
    );
    t.test_error(
        &["async function test() {await goog.requireDynamic(42);}"],
        &INVALID_REQUIRE_DYNAMIC,
    );
}

// port: GatherModuleMetadataTest#testMaybeRequire
#[test]
fn test_maybe_require() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.maybeRequireFrameworkInternalOnlyDoNotCallOrElse('my.Type')"]);
    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.maybe_required_goog_namespaces()),
        exactly(&["my.Type"])
    );
}

// port: GatherModuleMetadataTest#testMaybeRequireWithIllegalArg
#[test]
fn test_maybe_require_with_illegal_arg() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.maybeRequireFrameworkInternalOnlyDoNotCallOrElse('my.Type','extra.Type');"],
        &INVALID_MAYBE_REQUIRE,
    );
    t.test_error(
        &["goog.maybeRequireFrameworkInternalOnlyDoNotCallOrElse(42);"],
        &INVALID_MAYBE_REQUIRE,
    );
}

// port: GatherModuleMetadataTest#testRequiredClosureNamespaces
#[test]
fn test_required_closure_namespaces() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.require('my.Type');"]);
    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.strongly_required_goog_namespaces()),
        exactly(&["my.Type"])
    );
}

// port: GatherModuleMetadataTest#testImport
#[test]
fn test_import() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["import '@spec!';"]);
    let m = t.by_path("testcode");
    assert_eq!(strings(m.es6_import_specifiers()), exactly(&["@spec!"]));
}

// port: GatherModuleMetadataTest#testExport
#[test]
fn test_export() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["export { name } from '@spec!';"]);
    let m = t.by_path("testcode");
    assert_eq!(strings(m.es6_import_specifiers()), exactly(&["@spec!"]));
}

// port: GatherModuleMetadataTest#testImportOrder
#[test]
fn test_import_order() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["import 'first'; export { name } from 'second'; import 'third';"]);
    let m = t.by_path("testcode");
    assert_eq!(
        strings(m.es6_import_specifiers()),
        exactly(&["first", "second", "third"])
    );
}

// port: GatherModuleMetadataTest#testSetTestOnly
#[test]
fn test_set_test_only() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.setTestOnly();"]);
    assert!(t.by_path("testcode").is_test_only());
}

// port: GatherModuleMetadataTest#testSetTestOnlyWithStringArg
#[test]
fn test_set_test_only_with_string_arg() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.setTestOnly('string');"]);
    assert!(t.by_path("testcode").is_test_only());
}

// port: GatherModuleMetadataTest#testSetTestOnlyWithExtraArg
#[test]
fn test_set_test_only_with_extra_arg() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.setTestOnly('string', 'string');"],
        &INVALID_SET_TEST_ONLY,
    );
    assert!(!t.by_path("testcode").is_test_only());
}

// port: GatherModuleMetadataTest#testSetTestOnlyWithInvalidArg
#[test]
fn test_set_test_only_with_invalid_arg() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(&["goog.setTestOnly(0);"], &INVALID_SET_TEST_ONLY);
    assert!(!t.by_path("testcode").is_test_only());
}

// port: GatherModuleMetadataTest#testGatherFromExterns
#[test]
fn test_gather_from_externs() {
    // js_lib will put data in externs for .i.js files.
    let mut t = GatherModuleMetadataTest::set_up();
    let found = t.run_files_with_externs(
        "export var x; goog.declareModuleId('my.module');",
        &[("testcode", "")],
    );
    assert!(found.is_empty(), "{found:?}");
    keys_exactly(t.namespace_keys(), &["my.module"]);
    let m = t.by_namespace("my.module");
    assert_eq!(strings(m.goog_namespaces()), exactly(&["my.module"]));
    assert!(m.is_es6_module());
    assert!(!m.is_goog_module());
}

const IMPORTED_SCRIPT_SRCS: [(&str, &str); 3] = [
    ("imported.js", "console.log('lol');"),
    ("notimported.js", "console.log('lol');"),
    ("module.js", "import './imported.js';"),
];

// port: GatherModuleMetadataTest#testImportedScript
#[test]
fn test_imported_script() {
    let mut t = GatherModuleMetadataTest::set_up();
    let found = t.run_files(&IMPORTED_SCRIPT_SRCS);
    assert!(found.is_empty(), "{found:?}");

    assert_eq!(t.by_path("imported.js").module_type(), ModuleType::SCRIPT);
    assert_eq!(
        t.by_path("notimported.js").module_type(),
        ModuleType::SCRIPT
    );
}

// port: GatherModuleMetadataTest#testImportedScriptWithScriptsToModules
#[test]
fn test_imported_script_with_scripts_to_modules() {
    // Default dependency options should still mark imported files as ES modules.
    let mut t = GatherModuleMetadataTest::set_up();
    t.rewrite_scripts_to_modules = true;

    let found = t.run_files(&IMPORTED_SCRIPT_SRCS);
    assert!(found.is_empty(), "{found:?}");

    assert_eq!(
        t.by_path("imported.js").module_type(),
        ModuleType::ES6_MODULE
    );
    assert_eq!(
        t.by_path("notimported.js").module_type(),
        ModuleType::SCRIPT
    );
}

// port: GatherModuleMetadataTest#testImportedScriptWithEntryPoint
#[test]
fn test_imported_script_with_entry_point() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.rewrite_scripts_to_modules = true;
    t.entry_points = vec![ModuleIdentifier::for_file("module.js")];

    let found = t.run_files(&IMPORTED_SCRIPT_SRCS);
    assert!(found.is_empty(), "{found:?}");

    assert_eq!(
        t.by_path("imported.js").module_type(),
        ModuleType::ES6_MODULE
    );
    // Pruned
    assert!(
        !t.metadata_map()
            .get_modules_by_path()
            .contains_key("notimported.js")
    );
}

// port: GatherModuleMetadataTest#testImportedScriptWithSortOnly
#[test]
fn test_imported_script_with_sort_only() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.rewrite_scripts_to_modules = true;
    t.sort_only = true;

    let found = t.run_files(&IMPORTED_SCRIPT_SRCS);
    assert!(found.is_empty(), "{found:?}");

    assert_eq!(
        t.by_path("imported.js").module_type(),
        ModuleType::ES6_MODULE
    );
    assert_eq!(
        t.by_path("notimported.js").module_type(),
        ModuleType::SCRIPT
    );
}

// port: GatherModuleMetadataTest#testDynamicImport
#[test]
fn test_dynamic_import() {
    let mut t = GatherModuleMetadataTest::set_up();
    let found = t.run_files(&[
        ("imported.js", "export default function() {};"),
        ("script.js", "import('./imported.js')"),
    ]);
    assert!(found.is_empty(), "{found:?}");

    let m = t.by_path("script.js");
    assert!(m.is_non_provide_script());
    assert_eq!(
        strings(m.es6_import_specifiers()),
        exactly(&["./imported.js"])
    );
}

// port: GatherModuleMetadataTest#testReadToggle
#[test]
fn test_read_toggle() {
    let mut t = GatherModuleMetadataTest::set_up();
    let found = t.run_files(&[(
        "foo$2etoggles.js",
        "goog.module('foo$2etoggles');
exports.TOGGLE_foo = goog.readToggleInternalDoNotCallDirectly('foo');
exports.TOGGLE_bar = goog.readToggleInternalDoNotCallDirectly('bar');
exports.TOGGLE_b_a_z = goog.readToggleInternalDoNotCallDirectly('b_a_z');
",
    )]);
    assert!(found.is_empty(), "{found:?}");
    let m = t.by_path("foo$2etoggles.js");
    assert!(m.read_toggles().is_empty());
}

// port: GatherModuleMetadataTest#testToggleModuleImportDestructured
#[test]
fn test_toggle_module_import_destructured() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["const {TOGGLE_foo, TOGGLE_b_a_z} = goog.require('foo$2etoggles');"]);
    let m = t.by_path("testcode");
    assert_eq!(strings(m.read_toggles()), exactly(&["foo", "b_a_z"]));
}

// port: GatherModuleMetadataTest#testToggleModuleImportDestructuredAliased
#[test]
fn test_toggle_module_import_destructured_aliased() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["const {TOGGLE_bar: baz} = goog.require('foo$2etoggles');"]);
    let m = t.by_path("testcode");
    assert_eq!(strings(m.read_toggles()), exactly(&["bar"]));
}

// port: GatherModuleMetadataTest#testToggleModuleImportAsModule
#[test]
fn test_toggle_module_import_as_module() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["const toggles = goog.require('foo$2etoggles');
function foo() {
  console.log(toggles.TOGGLE_foo);
  console.log(toggles.TOGGLE_b_a_z);
}
"]);
    let m = t.by_path("testcode");
    assert_eq!(strings(m.read_toggles()), exactly(&["foo", "b_a_z"]));
}

// port: GatherModuleMetadataTest#testToggleModuleGoogModuleGet
#[test]
fn test_toggle_module_goog_module_get() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["goog.provide('foo.soy.gencode');
goog.require('foo$2etoggles');
function foo() {
  console.log(goog.module.get('foo$2etoggles').TOGGLE_foo);
  console.log(goog.module.get('bar').TOGGLE_bar);
}
"]);
    let m = t.by_path("testcode");
    assert_eq!(strings(m.read_toggles()), exactly(&["foo"]));
}

// port: GatherModuleMetadataTest#testToggleModulegoogModuleGetInvalidUsage
#[test]
fn test_toggle_modulegoog_module_get_invalid_usage() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["goog.provide('foo.soy.gencode');
goog.require('foo$2etoggles');
const toggles = goog.module.get('foo$2etoggles');
"],
        &INVALID_TOGGLE_USAGE,
    );
}

// port: GatherModuleMetadataTest#testToggleModuleImportAsSideEffect
#[test]
fn test_toggle_module_import_as_side_effect() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(&["goog.require('foo$2etoggles');"], &INVALID_TOGGLE_USAGE);
}

// port: GatherModuleMetadataTest#testToggleModuleImportDestructuredWithInvalidName
#[test]
fn test_toggle_module_import_destructured_with_invalid_name() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["const {foo} = goog.require('foo$2etoggles');"],
        &INVALID_TOGGLE_USAGE,
    );
}

// port: GatherModuleMetadataTest#testToggleModuleImportAsModuleWithInvalidPropertyName
#[test]
fn test_toggle_module_import_as_module_with_invalid_property_name() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["const foo = goog.require('foo$2etoggles');
console.log(foo.bar);
"],
        &INVALID_TOGGLE_USAGE,
    );
}

// port: GatherModuleMetadataTest#testToggleModuleInvalidModuleUsage
#[test]
fn test_toggle_module_invalid_module_usage() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_error(
        &["const foo = goog.require('foo$2etoggles');
console.log(foo);
"],
        &INVALID_TOGGLE_USAGE,
    );
}

// port: GatherModuleMetadataTest#testToggleModuleIgnoreShadowedUsage
#[test]
fn test_toggle_module_ignore_shadowed_usage() {
    let mut t = GatherModuleMetadataTest::set_up();
    t.test_same(&["const toggles = goog.require('foo$2etoggles');
function foo() {
  const toggles = {};
  console.log(toggles.foo)
  console.log(toggles.TOGGLE_bar)
}
"]);
    let m = t.by_path("testcode");
    assert!(m.read_toggles().is_empty());
}
