/*
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit ddfede7:
//   test/com/google/javascript/jscomp/CompilerOptionsTest.java.

use closure_jscomp::abstract_compiler::AbstractCompiler;
use closure_jscomp::compiler_options::{BrowserFeaturesetYear, CompilerOptions, LanguageMode};
use closure_jscomp::compiler_pass::CompilerPass;
use closure_jscomp::custom_pass_execution_time::CustomPassExecutionTime;
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::{Arc, Mutex};
// port: CompilerOptionsTest#testBrowserFeaturesetYearOptionSetsLanguageOut
#[test]
fn test_browser_featureset_year_option_sets_language_out() {
    let mut options = CompilerOptions::new();
    for (year, expected) in [
        (2012, LanguageMode::ECMASCRIPT5_STRICT.to_feature_set()),
        (2019, LanguageMode::ECMASCRIPT_2017.to_feature_set()),
        (2020, FeatureSet::BROWSER_2020),
        (2021, FeatureSet::BROWSER_2021),
        (2022, FeatureSet::BROWSER_2022),
        (2023, FeatureSet::BROWSER_2023),
        (2024, FeatureSet::BROWSER_2024),
        (2025, FeatureSet::BROWSER_2025),
        (2026, FeatureSet::BROWSER_2026),
    ] {
        options.set_browser_featureset_year(year);
        assert_eq!(options.get_output_feature_set(), expected);
    }
}
// port: CompilerOptionsTest#testBrowserFeaturesetYearOptionSetsAssumeES5
#[test]
fn test_browser_featureset_year_option_sets_assume_es5() {
    let mut options = CompilerOptions::new();
    let mut ast = Ast::new();
    options.set_browser_featureset_year(2012);
    assert_eq!(
        options.get_define_replacements(&mut ast)["$jscomp.ASSUME_ES5"].get_token(&ast),
        Token::FALSE
    );
    options.set_browser_featureset_year(2019);
    assert_eq!(
        options.get_define_replacements(&mut ast)["$jscomp.ASSUME_ES5"].get_token(&ast),
        Token::TRUE
    );
}
// port: CompilerOptionsTest#testBrowserFeaturesetYearOptionSetsAssumeES6
#[test]
fn test_browser_featureset_year_option_sets_assume_es6() {
    let mut options = CompilerOptions::new();
    let mut ast = Ast::new();
    for (year, name, token) in [
        (2012, "$jscomp.ASSUME_ES6", Token::FALSE),
        (2018, "$jscomp.ASSUME_ES6", Token::TRUE),
        (2020, "$jscomp.ASSUME_ES2020", Token::FALSE),
        (2021, "$jscomp.ASSUME_ES2020", Token::TRUE),
    ] {
        options.set_browser_featureset_year(year);
        assert_eq!(
            options.get_define_replacements(&mut ast)[name].get_token(&ast),
            token
        );
    }
}
// port: CompilerOptionsTest#testMinimumBrowserFeatureSetYearRequiredFor
#[test]
fn test_minimum_browser_feature_set_year_required_for() {
    for (feature, year) in [
        (Feature::GETTER, BrowserFeaturesetYear::YEAR_2012),
        (Feature::CLASSES, BrowserFeaturesetYear::YEAR_2018),
        (
            Feature::REGEXP_UNICODE_PROPERTY_ESCAPE,
            BrowserFeaturesetYear::YEAR_2021,
        ),
    ] {
        assert_eq!(
            BrowserFeaturesetYear::minimum_required_for(feature),
            Some(year)
        );
    }
}
// port: CompilerOptionsTest#testMinimumBrowserFeatureSetYearRequiredFor_returnsUnspecifiedIfUnsupported
#[test]
fn test_minimum_browser_feature_set_year_required_for_returns_unspecified_if_unsupported() {
    assert_eq!(
        BrowserFeaturesetYear::minimum_required_for(Feature::ES_NEXT_RUNTIME),
        None
    );
}
// port: CompilerOptionsTest#testDefines
#[test]
fn test_defines() {
    let mut options = CompilerOptions::new();
    options.set_define_to_boolean_literal("trueVar", true);
    options.set_define_to_boolean_literal("falseVar", false);
    options.set_define_to_number_literal("threeVar", 3);
    options.set_define_to_string_literal("strVar", "str");
    let mut ast = Ast::new();
    let actual = options.get_define_replacements(&mut ast);
    let true_node = ast.new_node(Token::TRUE);
    let false_node = ast.new_node(Token::FALSE);
    let number_node = ast.new_number(3.0);
    let string_node = ast.new_string("str");
    for (expected, name) in [
        (true_node, "trueVar"),
        (false_node, "falseVar"),
        (number_node, "threeVar"),
        (string_node, "strVar"),
    ] {
        assert!(expected.is_equivalent_to(&ast, actual[name]));
    }
}
// port: CompilerOptionsTest#testLanguageModeFromString
#[test]
fn test_language_mode_from_string() {
    for (input, expected) in [
        ("ECMASCRIPT3", Some(LanguageMode::ECMASCRIPT3)),
        ("  es3  ", Some(LanguageMode::ECMASCRIPT3)),
        ("junk", None),
        ("ECMASCRIPT_2020", Some(LanguageMode::ECMASCRIPT_2020)),
        ("  es_2020  ", Some(LanguageMode::ECMASCRIPT_2020)),
        ("  es2020  ", None),
        ("junk", None),
    ] {
        assert_eq!(LanguageMode::from_string(Some(input)), expected);
    }
}
// port: CompilerOptionsTest#testEmitUseStrictWorksInEs3
#[test]
fn test_emit_use_strict_works_in_es3() {
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(true);
    assert!(options.should_emit_use_strict());
}
// port: CompilerOptionsTest#testRemoveRegexFromPath
#[test]
fn test_remove_regex_from_path() {
    let options = CompilerOptions::new();
    let pattern = options
        .get_conformance_remove_regex_from_path()
        .as_ref()
        .unwrap();
    assert_eq!(
        pattern
            .matcher("/blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/blaze-out/k8-fastbin/genfiles/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("something/google3/blaze-out/k8-fastbin/some/path")
            .replace_all(""),
        "blaze-out/k8-fastbin/some/path"
    );
    assert_eq!(
        pattern
            .matcher("/something/google3/blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern.matcher("google3/some/path").replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/foo/blaze-out/some/path")
            .replace_all(""),
        "foo/blaze-out/some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/blaze-out/foo/blaze-out/some/path")
            .replace_all(""),
        "blaze-out/foo/blaze-out/some/path"
    );
    assert_eq!(
        pattern
            .matcher("bazel-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("bazel-out/k8-fastbin/genfiles/some/path")
            .replace_all(""),
        "some/path"
    );
}
// port: CompilerOptionsTest#testAllInstanceFieldsArePrivate
#[test]
fn test_all_instance_fields_are_private() {
    let source = include_str!("../src/compiler_options.rs");
    let fields = source
        .split("pub struct CompilerOptions {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for field in fields.lines().filter(|line| line.contains(':')) {
        assert!(
            !field.trim_start().starts_with("pub"),
            "All instance fields in CompilerOptions must be private: {field}"
        );
    }
}
// port: CompilerOptionsTest#testTranspilePublicClassFields
#[test]
fn test_transpile_public_class_fields() {
    let mut options = CompilerOptions::new();
    options.set_browser_featureset_year(2023);
    assert!(!options.get_transpile_public_class_fields());
    assert!(
        options
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    );
    options.set_transpile_public_class_fields(true);
    assert!(options.get_transpile_public_class_fields());
    assert!(
        !options
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    );
    options.set_transpile_public_class_fields(false);
    assert!(!options.get_transpile_public_class_fields());
    assert!(
        options
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    );
}
// port: CompilerOptionsTest#testRequiresTypesForOptimization
#[test]
fn test_requires_types_for_optimization() {
    let mut options = CompilerOptions::new();
    assert!(!options.requires_types_for_optimization());

    options.set_closure_pass(true);
    assert!(!options.requires_types_for_optimization());

    options.set_closure_pass(false);
    options.set_remove_closure_asserts(true);
    assert!(!options.requires_types_for_optimization());

    options.set_remove_closure_asserts(false);
    options.set_remove_abstract_methods(true);
    assert!(!options.requires_types_for_optimization());

    options.set_closure_pass(true);
    assert!(options.requires_types_for_optimization());

    options.set_remove_abstract_methods(false);
    options.set_remove_closure_asserts(true);
    assert!(options.requires_types_for_optimization());
}

// port: CompilerOptionsTest#testCollectionFieldsPreserveInsertionOrder
#[test]
fn test_collection_fields_preserve_insertion_order() {
    let mut options = CompilerOptions::new();

    let mut types = IndexSet::<String>::default();
    types.insert("typeB".to_string());
    types.insert("typeA".to_string());
    types.insert("typeC".to_string());
    options.set_strip_types(types);
    assert_eq!(
        options.get_strip_types().iter().collect::<Vec<_>>(),
        ["typeB", "typeA", "typeC"]
    );

    let mut suffixes = IndexSet::<String>::default();
    suffixes.insert("suffixB".to_string());
    suffixes.insert("suffixA".to_string());
    suffixes.insert("suffixC".to_string());
    options.set_strip_name_suffixes(suffixes);
    assert_eq!(
        options.get_strip_name_suffixes().iter().collect::<Vec<_>>(),
        ["suffixB", "suffixA", "suffixC"]
    );

    let mut prefixes = IndexSet::<String>::default();
    prefixes.insert("prefixB".to_string());
    prefixes.insert("prefixA".to_string());
    prefixes.insert("prefixC".to_string());
    options.set_strip_name_prefixes(prefixes);
    assert_eq!(
        options.get_strip_name_prefixes().iter().collect::<Vec<_>>(),
        ["prefixB", "prefixA", "prefixC"]
    );

    let pass1: Arc<Mutex<dyn CompilerPass + Send>> = Arc::new(Mutex::new(NoOpPass));
    let pass2: Arc<Mutex<dyn CompilerPass + Send>> = Arc::new(Mutex::new(NoOpPass));
    options.add_custom_pass(CustomPassExecutionTime::BEFORE_CHECKS, pass1.clone());
    options.add_custom_pass(CustomPassExecutionTime::BEFORE_CHECKS, pass2.clone());
    let passes = options.get_custom_passes_at(CustomPassExecutionTime::BEFORE_CHECKS);
    assert_eq!(passes.len(), 2);
    assert!(Arc::ptr_eq(&passes[0], &pass1));
    assert!(Arc::ptr_eq(&passes[1], &pass2));
}

/// The lambdas `(externs, root) -> {}` of testCollectionFieldsPreserveInsertionOrder.
struct NoOpPass;
impl CompilerPass for NoOpPass {
    fn process(&mut self, _compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {}
}

// port: CompilerOptionsTest#testToStringDeterministicFormat
#[test]
fn test_to_string_deterministic_format() {
    let new_options = || {
        let mut options = CompilerOptions::new();
        options.set_define_to_boolean_literal("defineA", true);
        options.set_define_to_boolean_literal("defineB", false);
        options.set_strip_types(["typeA", "typeB"].map(String::from).into_iter().collect());
        options.set_strip_name_suffixes(
            ["suffixA", "suffixB"]
                .map(String::from)
                .into_iter()
                .collect(),
        );
        options.set_strip_name_prefixes(
            ["prefixA", "prefixB"]
                .map(String::from)
                .into_iter()
                .collect(),
        );
        options
            .set_id_generator_names(["idGenA", "idGenB"].map(String::from).into_iter().collect());
        options
    };
    let options1 = new_options();
    let options2 = new_options();

    let s1 = options1.to_string();
    let s2 = options2.to_string();

    assert!(s1.starts_with("CompilerOptions{"));
    assert!(s1.ends_with('}'));
    assert!(!s1.contains('@')); // No identity hash codes
    assert_eq!(s1, s2);

    let expected = concat!(
        "CompilerOptions{aliasStringsMode=NONE, ambiguateProperties=false, angularPass=false,",
        " assumeGettersArePure=true,",
        " assumePropertiesAreStaticallyAnalyzable=true, assumeStrictThis=false,",
        " browserResolverPrefixReplacements={}, checkDeterminism=false,",
        " checkSuspiciousCode=false, checkSymbols=false, checkTypes=false, checksOnly=false,",
        " chunksToPrintAfterEachPassRegexList=[], closurePass=false,",
        " coalesceVariableNames=false, collapseAnonymousFunctions=false,",
        " collapseObjectLiterals=false, collapseProperties=NONE,",
        " collapseVariableDeclarations=false, colorizeErrorOutput=false,",
        " computeFunctionSideEffects=false, conformanceConfigs=[],",
        " conformanceRemoveRegexFromPath=Optional.of(^((.*/)?google3/)?(/?(blaze|bazel)-out/[^/]+/(bin|genfiles)/)?),",
        " conformanceReportingMode=IGNORE_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,",
        " continueAfterErrors=false, convertToDottedProperties=false,",
        " crossChunkCodeMotion=false, crossChunkCodeMotionNoStubMethods=false,",
        " crossChunkMethodMotion=false, deadAssignmentElimination=false,",
        " defineReplacements={defineA=TRUE, defineB=FALSE},",
        " dependencyOptions=DependencyOptions[mode=NONE, entryPoints=[]], devMode=OFF,",
        " devirtualizeMethods=false, disambiguateProperties=false,",
        " emitUseStrict=Optional.absent(), enableModuleRewriting=true, environment=BROWSER,",
        " errorFormat=FULL, es6ModuleTranspilation=COMPILE,",
        " es6SubclassTranspilation=CONCISE_UNSAFE, exportLocalPropertyDefinitions=true,",
        " exportTestFunctions=false, extractPrototypeMemberDeclarations=OFF,",
        " filesToPrintAfterEachPassRegexList=[], flowSensitiveInlineVariables=false,",
        " foldConstants=false, forceLibraryInjection=[], gatherCssNames=false,",
        " generateExports=true, generatePseudoNames=false, generateTypedExterns=false,",
        " idGenerators={idGenA=INCONSISTENT, idGenB=INCONSISTENT}, incrementalCheckMode=OFF,",
        " inferConsts=true, inferTypes=false, inlineConstantVars=false,",
        " inlineFunctionsLevel=NONE, inlineGetters=false, inlineLocalVariables=false,",
        " inlineProperties=false, inlineVariables=false, inputDelimiter=// Input %num%,",
        " inputSourceMaps={}, instrumentForCoverageOnly=false,",
        " instrumentForCoverageOption=NONE, isolatePolyfills=false, j2clMinifierEnabled=true,",
        " j2clPassMode=AUTO, labelRenaming=false, languageIn=ECMASCRIPT_NEXT,",
        " languageOutIsDefaultStrict=Optional.absent(), lineBreak=false,",
        " lineLengthThreshold=500, markAsCompiled=false, maxFunctionSizeAfterInlining=-1,",
        " moduleRoots=[./], nameGenerator=com.google.javascript.jscomp.DefaultNameGenerator,",
        " numParallelThreads=1, optimizeCalls=false, optimizeESClassConstructors=false,",
        " optimizeLocalAccessForGlobalSymbolNamespace=DISABLED,",
        " outputFeatureSet=Optional.absent(), outputJs=NORMAL, outputJsStringUsage=false,",
        " parentChunkCanSeeSymbolsDeclaredInChildren=false,",
        " parseJsDocDocumentation=TYPES_ONLY, pathEscaper=ESCAPE, polymerPass=false,",
        " preferSingleQuotes=false, preferStableNames=false,",
        " preserveDetailedSourceInfo=false, preserveGoogProvidesAndRequires=false,",
        " preserveNonJSDocComments=false, preserveTypeAnnotations=false, prettyPrint=false,",
        " printConfig=false, printInputDelimiter=false, printSourceAfterEachPass=false,",
        " processCommonJSModules=false, productionInstrumentationArrayName=,",
        " propertiesThatMustDisambiguate=[], propertyRenaming=OFF,",
        " propertyRenamingOnlyCompilationMode=false, protectHiddenSideEffects=false,",
        " qnameUsesToPrintAfterEachPassRegexList=[], quoteKeywordProperties=false,",
        " removeAbstractMethods=false, removeClosureAsserts=false, removeJ2clAsserts=true,",
        " removeUnusedClassProperties=false, removeUnusedLocalVars=false,",
        " removeUnusedPrototypeProperties=false, removeUnusedVars=false,",
        " renamePrefixNamespaceAssumeCrossChunkNames=false, replaceIdGenerators=true,",
        " replaceMessagesWithChromeI18n=false, replaceStringsFunctionDescriptions=[],",
        " replaceStringsPlaceholderToken=, reserveRawExports=false,",
        " rewriteFunctionExpressions=false,",
        " rewriteGlobalDeclarationsForTryCatchWrapping=false,",
        " rewriteModulesBeforeTypechecking=false, rewritePolyfills=false,",
        " runtimeLibraryMode=INJECT, skipNonTranspilationPasses=false,",
        " smartNameRemoval=false, sourceMapDetailLevel=ALL, sourceMapFormat=DEFAULT,",
        " sourceMapLocationMappings=[], strictMessageReplacement=false,",
        " stripNamePrefixes=[prefixA, prefixB], stripNameSuffixes=[suffixA, suffixB],",
        " stripTypes=[typeA, typeB], summaryDetailLevel=1, tracer=OFF, trustedStrings=false,",
        " tweakProcessing=OFF, useTypesForLocalOptimization=false, variableRenaming=OFF,",
        " warningsGuard=, wrapGoogModulesForWhitespaceOnly=true}",
    );
    assert_eq!(s1, expected);
}
