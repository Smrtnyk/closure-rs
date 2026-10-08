/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CheckMissingRequiresTest.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! Port of CheckMissingRequiresTest. CompilerTestCase's test/testSame with expected diagnostics are
//! reproduced with the direct Compiler API (as in gather_module_metadata_test.rs): one empty
//! externs file "externs", languageIn UNSUPPORTED and languageOut NO_TRANSPILE (CompilerTestCase's
//! defaults), the CompilerTestCase#getOptions warning levels, then getOptions' MISSING_REQUIRE
//! level. srcs(String...) names the sources "testcode<i>".
//!
//! The pass only reports diagnostics, so testSame's output comparison is not repeated here.
use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    check_missing_requires::{
        CheckMissingRequires, INCORRECT_NAMESPACE_ALIAS_REQUIRE,
        INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE, INDIRECT_NAMESPACE_REF_REQUIRE,
        INDIRECT_NAMESPACE_REF_REQUIRE_TYPE, MISSING_REQUIRE, MISSING_REQUIRE_FOR_GOOG_MODULE_GET,
        MISSING_REQUIRE_IN_GOOG_SCOPE, MISSING_REQUIRE_IN_PROVIDES_FILE, MISSING_REQUIRE_TYPE,
        MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE, MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE,
        NON_LEGACY_GOOG_MODULE_REFERENCE,
    },
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    deps::module_loader::{LOAD_WARNING, ResolutionMode},
    diagnostic_group::DiagnosticGroup,
    diagnostic_groups,
    diagnostic_type::DiagnosticType,
    gather_module_metadata::GatherModuleMetadata,
    js_error::JSError,
    source_file::SourceFile,
};
use std::sync::Arc;

/// `srcs(String...)` (every srcs call of this test passes the variadic form).
enum Srcs<'a> {
    Many(Vec<&'a str>),
}

impl<'a> Srcs<'a> {
    // port: CompilerTestCase#srcs(String...)
    fn many(srcs: &[&'a str]) -> Self {
        Srcs::Many(srcs.to_vec())
    }
    // port: CompilerTestCase#createSources
    fn files(&self) -> Vec<Arc<SourceFile>> {
        match self {
            Srcs::Many(srcs) => srcs
                .iter()
                .enumerate()
                .map(|(i, src)| Arc::new(SourceFile::from_code(&format!("testcode{i}"), *src)))
                .collect(),
        }
    }
}

enum MessagePredicate {
    Equals(String),
    Containing(String),
}

/// CompilerTestCase.Diagnostic.
struct Diagnostic {
    level: CheckLevel,
    diagnostic: &'static DiagnosticType,
    message_predicate: Option<MessagePredicate>,
    line: i32,
    charno: i32,
    length: i32,
}

// port: CompilerTestCase#warning(DiagnosticType)
fn warning(diagnostic: &'static DiagnosticType) -> Diagnostic {
    Diagnostic {
        level: CheckLevel::WARNING,
        diagnostic,
        message_predicate: None,
        line: -1,
        charno: -1,
        length: -1,
    }
}

// port: CompilerTestCase#error(DiagnosticType)
fn error(diagnostic: &'static DiagnosticType) -> Diagnostic {
    Diagnostic {
        level: CheckLevel::ERROR,
        ..warning(diagnostic)
    }
}

impl Diagnostic {
    // port: CompilerTestCase.Diagnostic#withMessage
    fn with_message(mut self, expected_raw: &str) -> Self {
        assert!(self.message_predicate.is_none());
        self.message_predicate = Some(MessagePredicate::Equals(expected_raw.trim().to_string()));
        self
    }
    // port: CompilerTestCase.Diagnostic#withMessageContaining
    fn with_message_containing(mut self, substring: &str) -> Self {
        assert!(self.message_predicate.is_none());
        self.message_predicate = Some(MessagePredicate::Containing(substring.to_string()));
        self
    }
    // port: CompilerTestCase.Diagnostic#withLocation
    fn with_location(mut self, line: i32, charno: i32, length: i32) -> Self {
        self.line = line;
        self.charno = charno;
        self.length = length;
        self
    }
    // port: CompilerTestCase.Diagnostic#matches
    fn matches(&self, error: &JSError) -> bool {
        if !std::ptr::eq(error.get_type(), self.diagnostic) {
            return false;
        }
        let message_matches = match &self.message_predicate {
            Some(MessagePredicate::Equals(expected)) => error.get_description().trim() == expected,
            Some(MessagePredicate::Containing(substring)) => {
                error.get_description().contains(substring.as_str())
            }
            None => true,
        };
        if !message_matches {
            return false;
        }
        if self.line != -1 && error.get_line_number() != self.line {
            return false;
        }
        if self.charno != -1 && error.charno() != self.charno {
            return false;
        }
        if self.length != -1 && error.length() != self.length {
            return false;
        }
        true
    }
}

struct CheckMissingRequiresTest {
    ignored_warnings: Vec<&'static DiagnosticType>,
}

impl CheckMissingRequiresTest {
    // port: CompilerTestCase#setUp
    fn set_up() -> Self {
        Self {
            ignored_warnings: Vec::new(),
        }
    }

    // port: CompilerTestCase#ignoreWarnings(DiagnosticType...)
    fn ignore_warnings(&mut self, warnings: &[&'static DiagnosticType]) {
        self.ignored_warnings.extend_from_slice(warnings);
    }

    // port: CheckMissingRequiresTest#getOptions
    fn get_options(&self) -> CompilerOptions {
        // port: CompilerTestCase#getOptions
        let mut options = CompilerOptions::new();
        options.set_language_in(LanguageMode::UNSUPPORTED);
        options.set_emit_use_strict(false);
        options.set_language_out(LanguageMode::NO_TRANSPILE);
        options.set_preserve_type_annotations(true);
        options.set_check_symbols(true);
        options.set_warning_level(
            diagnostic_groups::INVALID_CASTS.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISPLACED_MSG_ANNOTATION.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISSING_PROPERTIES.clone(),
            CheckLevel::WARNING,
        );
        if !self.ignored_warnings.is_empty() {
            options.set_warning_level(
                Arc::new(DiagnosticGroup::new(&self.ignored_warnings)),
                CheckLevel::OFF,
            );
        }
        options.set_warning_level(
            diagnostic_groups::MISSING_REQUIRE.clone(),
            CheckLevel::WARNING,
        );
        options
    }

    // port: CompilerTestCase#testInternal (CheckMissingRequiresTest#getProcessor)
    fn run(&mut self, srcs: &Srcs<'_>) -> Compiler {
        let mut compiler = Compiler::new();
        compiler.init(
            &[Arc::new(SourceFile::from_code("externs", ""))],
            &srcs.files(),
            self.get_options(),
        );
        compiler.parse_inputs().unwrap();
        assert!(compiler.get_errors().is_empty(), "parse errors");
        let externs = compiler.get_externs_root().unwrap();
        let root = compiler.get_js_root().unwrap();
        // port: CheckMissingRequiresTest#getProcessor
        GatherModuleMetadata::new(false, ResolutionMode::BROWSER).process(
            &mut compiler,
            externs,
            root,
        );
        let module_metadata_map = compiler.get_module_metadata_map().cloned().unwrap();
        CheckMissingRequires::new(&compiler, module_metadata_map).process(
            &mut compiler,
            externs,
            root,
        );
        compiler
    }

    // port: CompilerTestCase#test(Sources, Diagnostic...)
    fn test(&mut self, srcs: Srcs<'_>, diagnostics: &[Diagnostic]) {
        let compiler = self.run(&srcs);
        let check = |actual: Vec<JSError>, level: CheckLevel, kind: &str| {
            let expected: Vec<&Diagnostic> =
                diagnostics.iter().filter(|d| d.level == level).collect();
            let described: Vec<String> = actual
                .iter()
                .map(|e| format!("{}: {}", e.get_type().key, e.get_description()))
                .collect();
            assert_eq!(
                actual.len(),
                expected.len(),
                "unexpected {kind}(s): {described:?}"
            );
            for diagnostic in expected {
                assert!(
                    actual.iter().any(|e| diagnostic.matches(e)),
                    "no {kind} matches {} (line {}, charno {}, length {}): {:?}",
                    diagnostic.diagnostic.key,
                    diagnostic.line,
                    diagnostic.charno,
                    diagnostic.length,
                    actual
                        .iter()
                        .map(|e| format!(
                            "{} {}:{}+{} {}",
                            e.get_type().key,
                            e.get_line_number(),
                            e.charno(),
                            e.length(),
                            e.get_description()
                        ))
                        .collect::<Vec<_>>()
                );
            }
        };
        check(compiler.get_errors(), CheckLevel::ERROR, "error");
        check(compiler.get_warnings(), CheckLevel::WARNING, "warning");
    }

    // port: CompilerTestCase#testSame(Sources, Diagnostic...)
    fn test_same(&mut self, srcs: Srcs<'_>, diagnostics: &[Diagnostic]) {
        self.test(srcs, diagnostics);
    }

    // port: CheckMissingRequiresTest#checkNoWarning
    fn check_no_warning(&mut self, js: &[&str]) {
        self.test_same(Srcs::many(js), &[]);
    }

    fn check_warning(&mut self, diagnostic: &'static DiagnosticType, namespace: &str, js: &[&str]) {
        self.test_same(
            Srcs::many(js),
            &[warning(diagnostic).with_message_containing(&format!("'{namespace}'"))],
        );
    }

    // port: CheckMissingRequiresTest#checkRequireWarning
    fn check_require_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&MISSING_REQUIRE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkRequireTypeWarning
    fn check_require_type_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&MISSING_REQUIRE_TYPE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkRequireInProvidesFileWarning
    fn check_require_in_provides_file_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&MISSING_REQUIRE_IN_PROVIDES_FILE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkRequireTypeInProvidesFileWarning
    fn check_require_type_in_provides_file_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkIndirectNamespaceRefRequireWarning
    fn check_indirect_namespace_ref_require_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&INDIRECT_NAMESPACE_REF_REQUIRE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkIndirectNamespaceRefRequireTypeWarning
    fn check_indirect_namespace_ref_require_type_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&INDIRECT_NAMESPACE_REF_REQUIRE_TYPE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkIncorrectNamespaceAliasRequireWarning
    fn check_incorrect_namespace_alias_require_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&INCORRECT_NAMESPACE_ALIAS_REQUIRE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkIncorrectNamespaceAliasRequireTypeWarning
    fn check_incorrect_namespace_alias_require_type_warning(
        &mut self,
        namespace: &str,
        js: &[&str],
    ) {
        self.check_warning(&INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE, namespace, js);
    }

    // port: CheckMissingRequiresTest#checkRequireForGoogModuleGetWarning
    fn check_require_for_goog_module_get_warning(&mut self, namespace: &str, js: &[&str]) {
        self.check_warning(&MISSING_REQUIRE_FOR_GOOG_MODULE_GET, namespace, js);
    }
}

// port: CheckMissingRequiresTest#testNoWarning_existingRequire_withAlias
#[test]
fn test_no_warning_existing_require_with_alias() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
        "goog.module('test');\nconst Bar = goog.require('foo.Bar');\nlet x = new Bar();\n",
    ]);
}

// port: CheckMissingRequiresTest#testNoWarning_existingRequire_withDestructure
#[test]
fn test_no_warning_existing_require_with_destructure() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.module('foo.bar');\n/** @constructor */\nexports.Baz = function() {}\n",
        "goog.module('test');\nconst {Bar} = goog.require('foo.Bar');\nlet x = new Bar();\n",
    ]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_inProvide
#[test]
fn test_warning_missing_require_in_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_in_provides_file_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_inScript
#[test]
fn test_warning_missing_require_in_script() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_in_provides_file_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testMissingRequire_inEsModuleExport
#[test]
fn test_missing_require_in_es_module_export() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "let x = new foo.Bar();\nexport default 42;\n"]);
}

// port: CheckMissingRequiresTest#testMissingRequire_inEsModuleImport
#[test]
fn test_missing_require_in_es_module_import() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "import * as quux from './quux.js';\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_externs
#[test]
fn test_no_warning_missing_require_externs() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "/** @externs */\nvar foo = {}\n/** @constructor */\nfoo.Bar = function() {};\n",
        "goog.provide('test');\nlet x = new foo.Bar();\n",
    ]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_unknown
#[test]
fn test_no_warning_missing_require_unknown() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('test');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_sameProvide
#[test]
fn test_no_warning_missing_require_same_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.Bar');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_sameNestedProvide
#[test]
fn test_no_warning_missing_require_same_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.provide('foo');",
        "goog.provide('foo.Bar');\nlet x = new foo.Bar();\n",
    ]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_strongRef_withRequireType
#[test]
fn test_warning_missing_require_strong_ref_with_require_type() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_in_provides_file_warning(
        "a.b.C",
        &[
            "goog.provide('a.b.C');\n\n/** @constructor */\na.b.C = function() { };\n",
            "goog.requireType('a.b.C');\n\nnew a.b.C;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_withParentRequire_fromSameFile
#[test]
fn test_no_warning_missing_require_with_parent_require_from_same_file() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.provide('a.b');\ngoog.provide('a.b.C');\n",
        "goog.require('a.b');\n\nnew a.b.C;\n",
    ]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_withSiblingRequire_fromSameFile
#[test]
fn test_warning_missing_require_with_sibling_require_from_same_file() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_in_provides_file_warning(
        "a.b.D",
        &[
            "goog.provide('a.b');\ngoog.provide('a.b.C');\ngoog.provide('a.b.D');\n",
            "goog.require('a.b.C');\n\nnew a.b.D;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_withParentRequire_fromDifferentFile
#[test]
fn test_warning_missing_require_with_parent_require_from_different_file() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_in_provides_file_warning(
        "a.b.C",
        &[
            "goog.provide('a.b');",
            "goog.require('a.b');\ngoog.provide('a.b.C');\n",
            "goog.require('a.b');\n\nnew a.b.C;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_sameModule
#[test]
fn test_no_warning_missing_require_same_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.Bar');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_sameModule_nestedProvide
#[test]
fn test_no_warning_missing_require_same_module_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.bar');", "goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Baz = function() {}\nlet x = new foo.bar.Baz();\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_sameModuleWithLegacyNamespace_nestedProvide
#[test]
fn test_no_warning_missing_require_same_module_with_legacy_namespace_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.bar');", "goog.module('foo.bar.Baz');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports.Baz = function() {}\nlet x = new foo.bar.Baz();\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_existingRequireType_withAlias
#[test]
fn test_no_warning_existing_require_type_with_alias() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\nconst Bar = goog.requireType('foo.Bar');\n/** @type {!Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_existingRequireType_withDestructure
#[test]
fn test_no_warning_existing_require_type_with_destructure() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.bar');\n/** @constructor */\nexports.Baz = function() {}\n", "goog.module('test');\nconst {Bar} = goog.requireType('foo.Bar');\n/** @type {!Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_inProvide
#[test]
fn test_warning_missing_require_type_in_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_in_provides_file_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n/** @type {!foo.Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_inScript
#[test]
fn test_warning_missing_require_type_in_script() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_in_provides_file_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "/** @type {!foo.Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testMissingRequireType_inEsModuleExport
#[test]
fn test_missing_require_type_in_es_module_export() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.check_require_type_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "/** @type {!foo.Bar} */\nlet x;\nexport default 42;\n"]);
}

// port: CheckMissingRequiresTest#testMissingRequireType_inEsModuleImport
#[test]
fn test_missing_require_type_in_es_module_import() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.check_require_type_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "import * as quux from './quux.js';\n/** @type {!foo.Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forProvide
#[test]
fn test_warning_missing_require_for_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning(
        "foo.Bar",
        &[
            "goog.provide('foo.Bar');\n/** @constructor */\nfoo.Bar = function() {}\n",
            "goog.module('test');\nlet x = new foo.Bar();\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forProvide_usingProperty
#[test]
fn test_warning_missing_require_for_provide_using_property() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.bar", &["goog.provide('foo.bar');\nfoo.bar = {};\n/** @constructor */\nfoo.bar.Baz = function() {}\n", "goog.module('test');\nlet x = new foo.bar.Baz();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forNestedProvide
#[test]
fn test_warning_missing_require_for_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning(
        "foo.Bar",
        &[
            "goog.provide('foo');",
            "goog.provide('foo.Bar');\n/** @constructor */\nfoo.Bar = function() {}\n",
            "goog.module('test');\ngoog.require('foo');\nlet x = new foo.Bar();\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forNestedProvide_usingProperty
#[test]
fn test_warning_missing_require_for_nested_provide_using_property() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.bar", &["goog.provide('foo');", "goog.provide('foo.bar');\nfoo.bar = {};\n/** @constructor */\nfoo.bar.Baz = function() {}\n", "goog.module('test');\ngoog.require('foo');\nlet x = new foo.bar.Baz();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forModule
#[test]
fn test_warning_missing_require_for_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forModule_usingProperty
#[test]
fn test_warning_missing_require_for_module_using_property() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.bar", &["goog.module('foo.bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nfunction Baz() {}\nexports = { Baz }\n", "goog.module('test');\nlet x = new foo.bar.Baz();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forModuleWithLegacyNamespace
#[test]
fn test_warning_missing_require_for_module_with_legacy_namespace() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_forLateModule
#[test]
fn test_warning_missing_require_for_late_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning(
        "foo.Bar",
        &[
            "goog.module('test');\nlet x = new foo.Bar();\n",
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_existingRequire_standalone
#[test]
fn test_warning_existing_require_standalone() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\ngoog.require('foo.Bar');\nlet x = new foo.Bar();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forProvide
#[test]
fn test_warning_missing_require_type_for_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.provide('foo.Bar');\n/** @constructor */\nfoo.Bar = function() {}\n",
            "goog.module('test');\n/** @type {!foo.Bar} */ let x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forProvide_usingProperty
#[test]
fn test_warning_missing_require_type_for_provide_using_property() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning("foo.bar", &["goog.provide('foo.bar');\nfoo.bar = {};\n/** @constructor */\nfoo.bar.Baz = function() {}\n", "goog.module('test');\n/** @type {!foo.bar.Baz} */ let x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forNestedProvide
#[test]
fn test_warning_missing_require_type_for_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.provide('foo');",
            "goog.provide('foo.Bar');\n/** @constructor */\nfoo.Bar = function() {}\n",
            "goog.module('test');\ngoog.requireType('foo');\n/** @type {!foo.Bar} */ let x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forNestedProvide_usingProperty
#[test]
fn test_warning_missing_require_type_for_nested_provide_using_property() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning("foo.bar", &["goog.provide('foo');", "goog.provide('foo.bar');\nfoo.bar = {};\n/** @constructor */\nfoo.bar.Baz = function() {}\n", "goog.module('test');\ngoog.require('foo');\n/** @type {!foo.bar.Baz} */ let x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forModule
#[test]
fn test_warning_missing_require_type_for_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {!foo.Bar} */ let x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forModule_usingProperty
#[test]
fn test_warning_missing_require_type_for_module_using_property() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning("foo.bar", &["goog.module('foo.bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nfunction Baz() {}\nexports = { Baz }\n", "goog.module('test');\n/** @type {!foo.bar.Baz} */ let x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forModuleWithLegacyNamespace
#[test]
fn test_warning_missing_require_type_for_module_with_legacy_namespace() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\n/** @type {!foo.Bar} */ let x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_forLateModule
#[test]
fn test_warning_missing_require_type_for_late_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.provide('foo.Bar');\n/** @type {!foo.Bar} */ let x;\n",
            "goog.module('test');\n/** @type {!foo.Bar} */ let x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_existingRequireType_standalone
#[test]
fn test_warning_existing_require_type_standalone() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\ngoog.requireType('foo.Bar');\n/** @type {!foo.Bar} */ let x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_externs
#[test]
fn test_no_warning_missing_require_type_externs() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "/** @externs */\nvar foo = {}\n/** @constructor */\nfoo.Bar = function() {};\n",
        "goog.provide('test');\n/** @type {!foo.Bar} */\nlet x;\n",
    ]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_unknown
#[test]
fn test_no_warning_missing_require_type_unknown() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('test');\n/** @type {!foo.Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_sameProvide
#[test]
fn test_no_warning_missing_require_type_same_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.Bar');\n/** @type {!foo.Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_sameNestedProvide
#[test]
fn test_no_warning_missing_require_type_same_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.provide('foo');",
        "goog.provide('foo.Bar');\n/** @type {!foo.Bar} */\nlet x;\n",
    ]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_sameModule
#[test]
fn test_no_warning_missing_require_type_same_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.Bar');\n/** @type {!foo.Bar} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_sameModule_nestedProvide
#[test]
fn test_no_warning_missing_require_type_same_module_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.bar');", "goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Baz = function() {}\n/** @type {!foo.bar.Baz} */ let x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_nestedProvide
#[test]
fn test_warning_missing_require_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_incorrect_namespace_alias_require_warning("foo.bar.Baz", &["goog.provide('foo.bar');", "goog.provide('foo.bar.Baz');", "goog.module('another');\nconst {Baz} = goog.require('foo.bar');\nfunction ref(a) {};\nref(Baz);\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_nestedProvide
#[test]
fn test_warning_missing_require_type_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_incorrect_namespace_alias_require_type_warning("foo.bar.Baz", &["goog.provide('foo.bar');", "goog.provide('foo.bar.Baz');foo.bar.Baz = class {};", "goog.module('another');\nconst {Baz} = goog.requireType('foo.bar');\nlet /** !Baz */ x;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_nestedProvide
#[test]
fn test_no_warning_missing_require_type_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.bar');", "goog.provide('foo.bar.Baz');foo.bar.Baz = class {};", "goog.module('another');\nconst {Baz} = goog.requireType('foo.bar.Baz');\nlet /** !Baz */ x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_nestedModule
#[test]
fn test_warning_missing_require_nested_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_incorrect_namespace_alias_require_warning("foo.bar.Baz", &["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();", "goog.module('another');\nconst {Baz} = goog.require('foo.bar');\nfunction ref(a) {};\nref(Baz);\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_nestedModule
#[test]
fn test_warning_missing_require_type_nested_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_incorrect_namespace_alias_require_type_warning("foo.bar.Baz", &["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace(); exports = class {};", "goog.module('another');\nconst {Baz} = goog.requireType('foo.bar');\nlet /** Baz */ x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_destructure_nonLegacyModule_fromLegacyModule
#[test]
fn test_warning_destructure_non_legacy_module_from_legacy_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_incorrect_namespace_alias_require_warning("foo.bar.Baz", &["goog.module('foo.bar');\ngoog.module.declareLegacyNamespace();\n", "goog.module('foo.bar.Baz');\n/** @constructor */ exports = function() {};\n", "goog.module('test');\nconst {Baz} = goog.require('foo.bar');\nfunction ref(a) {}\nref(Baz);\n"]);
}

// port: CheckMissingRequiresTest#testWarning_destructure_nestedProvide_whenChildAlsoImported
#[test]
fn test_warning_destructure_nested_provide_when_child_also_imported() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_incorrect_namespace_alias_require_warning("foo.bar.Baz", &["goog.provide('foo.bar');", "goog.provide('foo.bar.Baz');", "goog.module('test');\nconst BarBaz = goog.require('foo.bar.Baz');\nconst {Baz} = goog.require('foo.bar');\nfunction ref(a) {}\nref(Baz);\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_nestedProvideIndirectRef
#[test]
fn test_warning_missing_require_nested_provide_indirect_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_indirect_namespace_ref_require_warning("foo.bar.Baz", &["goog.provide('foo.bar');", "goog.provide('foo.bar.Baz');", "goog.module('another');\nconst bar = goog.require('foo.bar');\nfunction ref(a) {};\nref(bar.Baz);\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_nestedProvideIndirectRef
#[test]
fn test_warning_missing_require_type_nested_provide_indirect_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_indirect_namespace_ref_require_type_warning("foo.bar.Baz", &["goog.provide('foo.bar');", "goog.provide('foo.bar.Baz');foo.bar.Baz = class {};", "goog.module('another');\nconst bar = goog.requireType('foo.bar');\nlet /** bar.Baz */ x;\n"]);
}

// port: CheckMissingRequiresTest#testIndirectNamespaceRef_errorLocation
#[test]
fn test_indirect_namespace_ref_error_location() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('goog.dom');", "goog.provide('goog.dom.TagName'); goog.dom.TagName = {B: 'b'};", "goog.module('test');\nconst dom = goog.require('goog.dom');\nconst x = dom.TagName.B;\n"]), &[warning(&INDIRECT_NAMESPACE_REF_REQUIRE).with_message("'goog.dom.TagName' should have its own goog.require.\nPlease add a separate goog.require and use that alias instead.").with_location(3, 10, 11)]);
}

// port: CheckMissingRequiresTest#testIndirectNamespaceRef_jsdoc_errorLocation
#[test]
fn test_indirect_namespace_ref_jsdoc_error_location() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('goog.dom');", "goog.provide('goog.dom.TagName');\n/** @const */\ngoog.dom.TagName = {};\ngoog.dom.TagName.C = class {};\n", "goog.module('test');\nconst dom = goog.require('goog.dom');\n/** @type {dom.TagName.C} */\nconst x = null;\n"]), &[warning(&INDIRECT_NAMESPACE_REF_REQUIRE_TYPE).with_message("'goog.dom.TagName' should have its own goog.requireType.\nPlease add a separate goog.requireType and use that alias instead.").with_location(3, 11, 11)]);
}

// port: CheckMissingRequiresTest#testIndirectNamespaceRef_jsdoc_errorLocation_withFunction
#[test]
fn test_indirect_namespace_ref_jsdoc_error_location_with_function() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('goog.dom');", "goog.provide('goog.dom.TagName'); /** @enum {string} */ goog.dom.TagName = {B: 'b'};", "goog.module('test');\nconst dom = goog.require('goog.dom');\n/** @type {function(dom.TagName)} */\nconst x = null;\n"]), &[warning(&INDIRECT_NAMESPACE_REF_REQUIRE_TYPE).with_message("'goog.dom.TagName' should have its own goog.requireType.\nPlease add a separate goog.requireType and use that alias instead.").with_location(3, 20, 11)]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequire_nestedModuleIndirectRef
#[test]
fn test_warning_missing_require_nested_module_indirect_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_indirect_namespace_ref_require_warning("foo.bar.Baz", &["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();", "goog.module('another');\nconst bar = goog.require('foo.bar');\nfunction ref(a) {};\nref(bar.Baz);\n"]);
}

// port: CheckMissingRequiresTest#testWarning_missingRequireType_nestedModuleIndirectRef
#[test]
fn test_warning_missing_require_type_nested_module_indirect_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_indirect_namespace_ref_require_type_warning("foo.bar.Baz", &["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();exports = class {};", "goog.module('another');\nconst bar = goog.requireType('foo.bar');\nlet /** bar.Baz */ x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_wrongAlias_nestedModuleIndirectRef
#[test]
fn test_warning_wrong_alias_nested_module_indirect_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_indirect_namespace_ref_require_type_warning("foo.bar.Baz", &["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();exports = class {};", "goog.module('another');\nconst bar = goog.requireType('foo.bar');\nconst Baz = goog.requireType('foo.bar.Baz');\nlet /** bar.Baz */ x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_wrongAlias_nestedModuleIndirectRef2
#[test]
fn test_warning_wrong_alias_nested_module_indirect_ref2() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_indirect_namespace_ref_require_warning("foo.bar.Baz", &["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();exports = class {};", "goog.module('another');\nconst bar = goog.require('foo.bar');\nconst Baz = goog.requireType('foo.bar.Baz');\nlet x = bar.Baz;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequire_nestedDirectRef
#[test]
fn test_no_warning_missing_require_nested_direct_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.module('foo.bar'); goog.module.declareLegacyNamespace();",
        "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();exports = class {};",
        "goog.module('another');\nconst Baz = goog.require('foo.bar.Baz');\nlet x = new Baz.C();\n",
    ]);
}

// port: CheckMissingRequiresTest#testNoWarningForRequireType_nestedDirectRef
#[test]
fn test_no_warning_for_require_type_nested_direct_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.bar'); goog.module.declareLegacyNamespace();", "goog.module('foo.bar.Baz'); goog.module.declareLegacyNamespace();exports = class {};", "goog.module('another');\nconst Baz = goog.requireType('foo.bar.Baz');\nlet /** !Baz.C */ x = null;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_overlapping_module_id_and_legacy_namespace
#[test]
fn test_no_warning_overlapping_module_id_and_legacy_namespace() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('jspb'); exports = {Message: class {}};", "goog.module('jspb.Message');\ngoog.module.declareLegacyNamespace();\nconst {Message} = goog.require('jspb');\nexports = Message;\n", "goog.module('another');\nconst {Message} = goog.requireType('jspb');\nlet /** !Message */ x = null;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_overlapping_legacy_namespaces
#[test]
fn test_no_warning_overlapping_legacy_namespaces() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('wiz');", "goog.provide('wiz.controller');", "goog.module('wiz.controller.idomcompatiblecontroller');\ngoog.module.declareLegacyNamespace();\nclass IdomCompatibleController {}\nexports = {IdomCompatibleController};\n", "goog.module('another');\nconst SomeRandomThing = goog.require('wiz.controller.idomcompatiblecontroller');\nlet /** !SomeRandomThing.IdomCompatibleController */ x = null;\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_missingRequireType_sameModuleWithLegacyNamespace_nestedProvide
#[test]
fn test_no_warning_missing_require_type_same_module_with_legacy_namespace_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.provide('foo.bar');", "goog.module('foo.bar.Baz');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports.Baz = function() {}\n/** @type {!foo.bar.Baz} */ let x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_jsDocParam
#[test]
fn test_warning_js_doc_param() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @param {!foo.Bar} x */\nfunction f(x) {}\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocThis
#[test]
fn test_warning_js_doc_this() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @this {foo.Bar} */\nfunction f() {}\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocReturn
#[test]
fn test_warning_js_doc_return() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @return {!foo.Bar} */\nfunction f() {}\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocEnum
#[test]
fn test_warning_js_doc_enum() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @enum {foo.Bar} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocTypedef
#[test]
fn test_warning_js_doc_typedef() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @typedef {foo.Bar} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocExtendsClass
#[test]
fn test_warning_js_doc_extends_class() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\n/**\n * @constructor\n * @extends {foo.Bar}\n */\nfunction Bar() {}\n"]);
}

// port: CheckMissingRequiresTest#testWarning_jsDocExtendsInterface
#[test]
fn test_warning_js_doc_extends_interface() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\n/** @interface */\nexports = function() {}\n", "goog.module('test');\n/**\n * @interface\n * @extends {foo.Bar}\n */\nfunction Bar() {}\n"]);
}

// port: CheckMissingRequiresTest#testWarning_jsDocImplements
#[test]
fn test_warning_js_doc_implements() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("foo.Bar", &["goog.module('foo.Bar');\n/** @interface */\nexports = function() {}\n", "goog.module('test');\n/**\n * @constructor\n * @implements {foo.Bar}\n */\nfunction Bar() {}\n"]);
}

// port: CheckMissingRequiresTest#testWarning_jsDocUnion
#[test]
fn test_warning_js_doc_union() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {string|foo.Bar} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocTemplate
#[test]
fn test_warning_js_doc_template() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning("foo.Bar", &["goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n", "goog.module('test');\n/** @template T */\nclass Quux {}\n/** @type {!Quux<!foo.Bar>} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testWarning_jsDocRecord
#[test]
fn test_warning_js_doc_record() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {{foo: !foo.Bar}} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocFunctionParam
#[test]
fn test_warning_js_doc_function_param() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {function(!foo.Bar)} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocFunctionReturn
#[test]
fn test_warning_js_doc_function_return() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {function():!foo.Bar} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocFunctionNew
#[test]
fn test_warning_js_doc_function_new() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {function(new:foo.Bar)} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocFunctionThis
#[test]
fn test_warning_js_doc_function_this() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {function(this:foo.Bar)} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_jsDocTypeof
#[test]
fn test_warning_js_doc_typeof() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning(
        "foo.Bar",
        &[
            "goog.module('foo.Bar');\n/** @constructor */\nexports = function() {}\n",
            "goog.module('test');\n/** @type {typeof foo.Bar} */\nlet x;\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testNoWarning_jsDocClassTemplateParam
#[test]
fn test_no_warning_js_doc_class_template_param() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('MyType');\ngoog.module.declareLegacyNamespace();\n", "goog.module('test');\n/** @template MyType */\nclass Foo {\n  /** @param {!MyType} x */\n  constructor(x) {}\n}\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_jsDocFunctionTemplateParam
#[test]
fn test_no_warning_js_doc_function_template_param() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('MyType');\ngoog.module.declareLegacyNamespace();\n", "goog.module('test');\n/**\n * @param {!MyType} x\n * @return {!MyType}\n * @template MyType\n */\nfunction foo(x) {\n  return x;\n}\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_jsDocTtlParam
#[test]
fn test_no_warning_js_doc_ttl_param() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('MyType');\ngoog.module.declareLegacyNamespace();\n", "goog.module('YourType');\ngoog.module.declareLegacyNamespace();\n", "goog.module('test');\n/**\n * @param {!MyType} x\n * @return {!YourType}\n * @template MyType\n * @template YourType := MyType =:\n */\nfunction foo(x) {\n  return x;\n}\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_shadow_moduleScope
#[test]
fn test_no_warning_shadow_module_scope() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.module('Bar');\ngoog.module.declareLegacyNamespace();\n",
        "goog.module('test');\nclass Bar {}\n/** @type {!Bar} */\nlet x = new Bar();\n",
    ]);
}

// port: CheckMissingRequiresTest#testNoWarning_shadow_localScope
#[test]
fn test_no_warning_shadow_local_scope() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('Bar');\ngoog.module.declareLegacyNamespace();\n", "goog.module('test');\nfunction foo() {\n  class Bar {}\n  /** @type {!Bar} */\n  let x = new Bar();\n}\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_shadow_parentScope
#[test]
fn test_no_warning_shadow_parent_scope() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('Bar');\ngoog.module.declareLegacyNamespace();\n", "goog.module('test');\nclass Bar {}\nfunction foo() {\n  /** @type {!Bar} */\n  let x = new Bar();\n}\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_providedGoogModule
#[test]
fn test_no_warning_provided_goog_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.provide('goog.module');\ngoog.module = goog.module || {};\n",
        "goog.module('test');\ngoog.module.declareLegacyNamespace();\n",
    ]);
}

// port: CheckMissingRequiresTest#testWarning_providedGoogModule_missingRequireForNestedProvide
#[test]
fn test_warning_provided_goog_module_missing_require_for_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_warning("goog.module.ModuleManager", &["goog.provide('goog.module');\ngoog.module = goog.module || {};\n", "goog.provide('goog.module.ModuleManager');\n/** @constructor */\ngoog.module.ModuleManager = function() {};\n", "goog.module('test');\nlet x = new goog.module.ModuleManager();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_providedGoogModule_missingRequireTypeForNestedProvide
#[test]
fn test_warning_provided_goog_module_missing_require_type_for_nested_provide() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_type_warning("goog.module.ModuleManager", &["goog.provide('goog.module');\ngoog.module = goog.module || {};\n", "goog.provide('goog.module.ModuleManager');\n/** @constructor */\ngoog.module.ModuleManager = function() {};\n", "goog.module('test');\n/** @type {!goog.module.ModuleManager} */\nlet x;\n"]);
}

// port: CheckMissingRequiresTest#testNoCrashOnGetprops
#[test]
fn test_no_crash_on_getprops() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('test');\nclass Class {\n  constructor() {\n    foo().bar;\n    foo['bar'].baz;\n    this.foo;\n    super.foo;\n  }\n}\n"]);
}

// port: CheckMissingRequiresTest#testReferenceNonLegacyGoogModule_inScript_required_warns
#[test]
fn test_reference_non_legacy_goog_module_in_script_required_warns() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(
        Srcs::many(&[
            "goog.module('test.a'); exports.x = 1;",
            "goog.provide('test.b'); goog.require('test.a'); console.log(test.a.x);",
        ]),
        &[error(&NON_LEGACY_GOOG_MODULE_REFERENCE)],
    );
}

// port: CheckMissingRequiresTest#testReferenceNonLegacyGoogModule_inScript_typePosition_required_noWarning
#[test]
fn test_reference_non_legacy_goog_module_in_script_type_position_required_no_warning() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('test.a');\n/** @interface */\nexports.C = class {};\n", "goog.provide('test.b');\ngoog.require('test.a');\n/** @implements {test.a.C} */\ntest.b.C = class {\n  /** @param {!test.a.C} c */\n  foo(c) {}\n};\n"]);
}

// port: CheckMissingRequiresTest#testReferenceNonLegacyGoogModule_inScript_typePosition_and_code_required_warning
#[test]
fn test_reference_non_legacy_goog_module_in_script_type_position_and_code_required_warning() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.module('test.a');\n/** @interface */\nexports.C = class {};\n", "goog.provide('test.b');\ngoog.require('test.a');\n/** @implements {test.a.C} */\ntest.b.C = class {\n  /** @param {!test.a.C} c */\n  foo(c) { test.a.C; } // reports error\n};\n"]), &[error(&NON_LEGACY_GOOG_MODULE_REFERENCE)]);
}

// port: CheckMissingRequiresTest#tesNonLegacyGoogModule_inScript_required_noWarning
#[test]
fn tes_non_legacy_goog_module_in_script_required_no_warning() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&[
        "goog.module('test.a'); goog.module.declareLegacyNamespace(); exports.x = 1;",
        "goog.provide('test.b'); goog.require('test.a'); console.log(test.a.x);",
    ]);
}

// port: CheckMissingRequiresTest#testWarning_googScope_googModuleGet_noRequire
#[test]
fn test_warning_goog_scope_goog_module_get_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_for_goog_module_get_warning("foo.bar.Baz", &["goog.module('foo.bar.Baz');\n/** @constructor */\nexports = function() {}\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst Baz = goog.module.get('foo.bar.Baz');\nexports.fn = function() { new Baz(); }\n});\n"]);
}

// port: CheckMissingRequiresTest#testWarning_script_googModuleGet_inProvideInitialization_noRequire
#[test]
fn test_warning_script_goog_module_get_in_provide_initialization_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_for_goog_module_get_warning(
        "foo.bar.Baz",
        &[
            "goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n",
            "goog.provide('test');\n\ntest.boo = new (goog.module.get('foo.bar.Baz').Boo)();\n",
        ],
    );
}

// port: CheckMissingRequiresTest#testWarning_script_googModuleGet_inScriptIfBlock_noRequire
#[test]
fn test_warning_script_goog_module_get_in_script_if_block_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_for_goog_module_get_warning("foo.bar.Baz", &["goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n", "goog.provide('test');\n\nif (true) { console.log(new (goog.module.get('foo.bar.Baz').Boo)()); }\n"]);
}

// port: CheckMissingRequiresTest#testWarning_script_googModuleGet_inScript_inTopLevelIIFE_noRequire
#[test]
fn test_warning_script_goog_module_get_in_script_in_top_level_iife_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_for_goog_module_get_warning("foo.bar.Baz", &["goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n", "goog.provide('test');\n\n(() => console.log(new (goog.module.get('foo.bar.Baz').Boo)()))();\n"]);
}

// port: CheckMissingRequiresTest#testWarning_googScope_googModuleGet_withPropAccess_noRequire
#[test]
fn test_warning_goog_scope_goog_module_get_with_prop_access_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_for_goog_module_get_warning("foo.bar.Baz", &["goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst Woo = goog.module.get('foo.bar.Baz').Boo.Goo.Woo;\nexports.fn = function() { new Woo(); }\n});\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_googScope_googModuleGet_hasRequire
#[test]
fn test_no_warning_goog_scope_goog_module_get_has_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.bar.Baz');\n/** @constructor */\nexports = function() {}\n", "goog.provide('test');\ngoog.require('foo.bar.Baz');\n\ngoog.scope(function() {\nconst Baz = goog.module.get('foo.bar.Baz');\nfunction fn() { new Baz(); }\n});\n"]);
}

// port: CheckMissingRequiresTest#testWarning_googScope_googModuleGet_IIFE_noRequire
#[test]
fn test_warning_goog_scope_goog_module_get_iife_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_for_goog_module_get_warning("foo.bar.Baz", &["goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n", "goog.provide('test');\n\ngoog.scope(function() {\n  (() => console.log(new (goog.module.get('foo.bar.Baz').Boo)()))();\n});\n"]);
}

// port: CheckMissingRequiresTest#testWarning_script_googModuleGet_inFunctionInScript_noRequire_noWarning
#[test]
fn test_warning_script_goog_module_get_in_function_in_script_no_require_no_warning() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n", "goog.provide('test');\n\n// We allow this goog.module.get despite the lack of goog.require: it's possible that\n// the foo.bar.Baz module will have been loaded by the time test.fn is called. It's up\n// to the caller to ensure that it's loaded.\ntest.fn = function() {\n  console.log(new (goog.module.get('foo.bar.Baz').Boo)());\n}\n"]);
}

// port: CheckMissingRequiresTest#testWarning_script_googModuleGet_inFunctionInScript_inNonTopLevelIIFE_noRequire_noWarning
#[test]
fn test_warning_script_goog_module_get_in_function_in_script_in_non_top_level_iife_no_require_no_warning()
 {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.bar.Baz');\n/** @constructor */\nexports.Boo = function() {}\n", "goog.provide('test');\n\n// We allow this goog.module.get despite the lack of goog.require: it's possible that\n// the foo.bar.Baz module will have been loaded by the time test.fn is called. It's up\n// to the caller to ensure that it's loaded.\ntest.fn = function() {\n  (() => console.log(new (goog.module.get('foo.bar.Baz').Boo)()))();\n}\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_nestedInGoogScope_googModuleGet_noRequire
#[test]
fn test_no_warning_nested_in_goog_scope_goog_module_get_no_require() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.bar.Baz');\n/** @constructor */\nexports = function() {}\n", "goog.provide('test');\n\ngoog.scope(function() {\ntest.fn = function() {\n// We allow this goog.module.get despite the lack of goog.require: it's possible that\n// the foo.bar.Baz module will have been loaded by the time test.fn is called. It's up\n// to the caller to ensure that it's loaded.\n  const Baz = goog.module.get('foo.bar.Baz');\n  new Baz();\n  new goog.module.get('foo.bar.Baz');\n}\n});\n"]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAliasOfModule
#[test]
fn test_warning_goog_scope_alias_of_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_require_in_provides_file_warning("foo.Bar", &["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst Bar = foo.Bar;\n})(); // end goog.scope\n"]);
}

// port: CheckMissingRequiresTest#testNoWarning_googScopeAliasOfModule
#[test]
fn test_no_warning_goog_scope_alias_of_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.check_no_warning(&["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\nexports = class {};\nexports.Baz = class { static fn() {} };\n", "goog.provide('test');\ngoog.require('foo.Bar');\n\ngoog.scope(function() {\nconst Bar = foo.Bar;\nconst Baz = foo.Bar.Baz;\ntest.fn = function() {\n  new Bar(); Bar.fn();\n  new Baz(); Baz.fn();\n  new Bar.Baz(); Bar.Baz.fn();\n}\n})(); // end goog.scope\n"]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_unprovidedParentOfModule
#[test]
fn test_warning_goog_scope_alias_unprovided_parent_of_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { return fooLocal.Bar; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_GOOG_SCOPE)]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_unprovidedParentOfModule_typeRef
#[test]
fn test_warning_goog_scope_alias_unprovided_parent_of_module_type_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { /** @type {!fooLocal.Bar} */ var x; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE)]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_providedParentOfModule
#[test]
fn test_warning_goog_scope_alias_provided_parent_of_module() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('foo');", "goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { return fooLocal.Bar; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_PROVIDES_FILE).with_message_containing("'foo'"), warning(&MISSING_REQUIRE_IN_GOOG_SCOPE).with_message_containing("'foo.Bar'")]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_providedParentOfModule_typeRef
#[test]
fn test_warning_goog_scope_alias_provided_parent_of_module_type_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('foo');", "goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { /** @type {!fooLocal.Bar} */ var x; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_PROVIDES_FILE).with_message_containing("foo"), warning(&MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE).with_message_containing("foo.Bar")]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_providedParentOfModule_requireParent
#[test]
fn test_warning_goog_scope_alias_provided_parent_of_module_require_parent() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('foo');", "goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\ngoog.require('foo');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { return fooLocal.Bar; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_GOOG_SCOPE).with_message_containing("'foo.Bar'")]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_providedParentOfModule_googModuleGetParent
#[test]
fn test_warning_goog_scope_alias_provided_parent_of_module_goog_module_get_parent() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('foo');", "goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\ngoog.require('foo');\n\ngoog.scope(function() {\nconst fooLocal = goog.module.get('foo');\nfunction fn() { return fooLocal.Bar; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_GOOG_SCOPE).with_message_containing("'foo.Bar'")]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_providedParentOfModule_googModuleGetParent_propRef
#[test]
fn test_warning_goog_scope_alias_provided_parent_of_module_goog_module_get_parent_prop_ref() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('foo');", "goog.module('foo.bar');\ngoog.module.declareLegacyNamespace();\n", "goog.module('foo.bar.Raz.Baz');\ngoog.module.declareLegacyNamespace();\nexports = class {};\n", "goog.provide('test');\ngoog.require('foo.bar');\n\ngoog.scope(function() {\nconst fooBarRaz = goog.module.get('foo.bar').Raz;\nfunction fn() { return fooBarRaz.Baz; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_GOOG_SCOPE).with_message_containing("'foo.bar.Raz.Baz'")]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAlias_providedParentOfModule_typeRef_requireParent
#[test]
fn test_warning_goog_scope_alias_provided_parent_of_module_type_ref_require_parent() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('foo');\n", "goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\ngoog.require('foo');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { /** @type {!fooLocal.Bar} */ var x; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE).with_message_containing("foo.Bar")]);
}

// port: CheckMissingRequiresTest#testWarning_googScopeAliasOfParentOfModule_childStillRequired
#[test]
fn test_warning_goog_scope_alias_of_parent_of_module_child_still_required() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.module('foo.Bar');\ngoog.module.declareLegacyNamespace();\n/** @constructor */\nexports = function() {};\n", "goog.provide('test');\n// Note that there is a require for foo.Bar, which is correct,  but the warning is\n// still triggered because of the alias `const fooLocal = foo;`.\ngoog.require('foo.Bar');\n\ngoog.scope(function() {\nconst fooLocal = foo;\nfunction fn() { return fooLocal.Bar; }\n})(); // end goog.scope\n"]), &[warning(&MISSING_REQUIRE_IN_GOOG_SCOPE)]);
}

// port: CheckMissingRequiresTest#testIndirectNamespaceRef_jsdoc_errorLocation_longNamespace
#[test]
fn test_indirect_namespace_ref_jsdoc_error_location_long_namespace() {
    let mut t = CheckMissingRequiresTest::set_up();
    t.test(Srcs::many(&["goog.provide('goog.dom');", "goog.provide('goog.dom.tags');", "goog.provide('goog.dom.tags.misc'); /** @const */ goog.dom.tags.misc = {FOO: 'foo'};", "goog.module('test');\nconst dom = goog.require('goog.dom');\n/** @type {dom.tags.misc.FOO} */\nconst x = null;\n"]), &[warning(&INDIRECT_NAMESPACE_REF_REQUIRE_TYPE).with_message("'goog.dom.tags.misc' should have its own goog.requireType.\nPlease add a separate goog.requireType and use that alias instead.").with_location(3, 11, 13)]);
}
