/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/deps/JsFileRegexParserTest.java.

use super::*;
use crate::black_hole_error_manager::BlackHoleErrorManager;
use crate::deps::simple_dependency_info::SimpleDependencyInfo;
use crate::deps::{
    browser_module_resolver::BrowserModuleResolver,
    browser_with_transformed_prefixes_module_resolver as prefixes,
};
use std::sync::{Arc, Mutex};
const SRC_PATH: &str = "a";
const CLOSURE_PATH: &str = "b";
// port: JsFileRegexParserTest#testParseFile
#[test]
fn test_parse_file() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/*goog.provide('no1');*//*\ngoog.provide('no2');\n*/goog.provide('yes1');\n/* blah */goog.provide(\"yes2\")/* blah*/\ngoog.require('yes3'); // goog.provide('no3');\n// goog.provide('no4');\ngoog.require(\"bar.data.SuperstarAddStarThreadActionRequestDelegate\"); //no new line at EOF\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1", "yes2"])
        .set_requires([
            Require::goog_require_symbol("yes3"),
            Require::goog_require_symbol("bar.data.SuperstarAddStarThreadActionRequestDelegate"),
        ])
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseFile2
#[test]
fn test_parse_file2() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.module('yes1');\nvar yes2 = goog.require('yes2');\nvar C = goog.require(\"a.b.C\");\nlet {D, E} = goog.require('a.b.d');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_requires(vec![
            Require::goog_require_symbol("yes2"),
            Require::goog_require_symbol("a.b.C"),
            Require::goog_require_symbol("a.b.d"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "goog".into(),
        )]))
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseFile3
#[test]
fn test_parse_file3() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.module('yes1');\nvar yes2=goog.require('yes2');\nvar C=goog.require(\"a.b.C\");\nconst {\n  D,\n  E\n}=goog.require(\"a.b.d\");\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_requires(vec![
            Require::goog_require_symbol("yes2"),
            Require::goog_require_symbol("a.b.C"),
            Require::goog_require_symbol("a.b.d"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "goog".into(),
        )]))
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseFileWithMultiLineRequires
#[test]
fn test_parse_file_with_multi_line_requires() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.set_shortcut_mode(false);

    let contents = "goog.module('yes1');\nvar fakerequire = 5;\nvar yes2=goog.require(\n'yes2');\nvar C=\ngoog.require(\"a.b.C\");\nconst {\n  D,\n  E\n}=goog.require(\n\"a.b.d\");\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_requires(vec![
            Require::goog_require_symbol("yes2"),
            Require::goog_require_symbol("a.b.C"),
            Require::goog_require_symbol("a.b.d"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "goog".into(),
        )]))
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testUnicodeCharacters
#[test]
fn test_unicode_characters() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "const FɵɵΔBar = goog.require('imaginary.test.namespace');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_requires(vec![Require::goog_require_symbol(
            "imaginary.test.namespace",
        )])
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseGoogModuleWithRequireType
#[test]
fn test_parse_goog_module_with_require_type() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.module('yes1');\nvar yes2=goog.requireType('yes2');\nvar C=goog.requireType(\"a.b.C\");\nconst {\n  D,\n  E\n}=goog.requireType(\"a.b.d\");\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_type_requires(vec!["yes2", "a.b.C", "a.b.d"])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "goog".into(),
        )]))
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseScriptWithRequireType
#[test]
fn test_parse_script_with_require_type() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.provide('yes1');\ngoog.requireType('a.b.C');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_type_requires(vec!["a.b.C"])
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseWrappedGoogModule
#[test]
fn test_parse_wrapped_goog_module() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.loadModule(function(){\"use strict\";goog.module('yes1');\nvar yes2=goog.require('yes2');\nvar C=goog.require(\"a.b.C\");\nconst {\n  D,\n  E\n}=goog.require(\"a.b.d\");});\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_requires(vec![
            Require::goog_require_symbol("yes2"),
            Require::goog_require_symbol("a.b.C"),
            Require::goog_require_symbol("a.b.d"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([]))
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseEs6Module
#[test]
fn test_parse_es6_module() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "import def, {yes2} from './yes2';\nimport C from './a/b/C';\nimport * as d from './a/b/d';\nimport \"./dquote\";\nexport * from './exported';\n";

    let expected = SimpleDependencyInfo::builder("a.js", "b.js")
        .set_provides(vec!["module$b"])
        .set_requires([
            Require::es6_import("module$yes2", "./yes2"),
            Require::es6_import("module$a$b$C", "./a/b/C"),
            Require::es6_import("module$a$b$d", "./a/b/d"),
            Require::es6_import("module$dquote", "./dquote"),
            Require::es6_import("module$exported", "./exported"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "es6".into(),
        )]))
        .build();

    let result = parser.parse_file("b.js", "a.js", contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseEs6Module2
#[test]
fn test_parse_es6_module2() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "import './x';\nimport '../y';\nimport '../a/z';\nimport '../c/w';\n";

    let expected = SimpleDependencyInfo::builder("../../a/b.js", "/foo/bar/a/b.js")
        .set_provides(vec!["module$foo$bar$a$b"])
        .set_requires(vec![
            Require::es6_import("module$foo$bar$a$x", "./x"),
            Require::es6_import("module$foo$bar$y", "../y"),
            Require::es6_import("module$foo$bar$a$z", "../a/z"),
            Require::es6_import("module$foo$bar$c$w", "../c/w"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "es6".into(),
        )]))
        .build();

    let result = parser.parse_file("/foo/bar/a/b.js", "../../a/b.js", contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseEs6Module3
#[test]
fn test_parse_es6_module3() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "import 'goog:foo.bar.baz';\ngoog.require('baz.qux');\n";

    let expected = SimpleDependencyInfo::builder("b.js", "a.js")
        .set_provides(vec!["module$a"])
        .set_requires(vec![
            Require::goog_require_symbol("foo.bar.baz"),
            Require::goog_require_symbol("baz.qux"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "es6".into(),
        )]))
        .build();

    let result = parser.parse_file("a.js", "b.js", contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseEs6Module4
#[test]
fn test_parse_es6_module4() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["/foo"])
        .set_inputs(Vec::<SimpleDependencyInfo>::new())
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let contents = "import './a';\nimport './qux/b';\nimport '../closure/c';\nimport '../closure/d/e';\nimport '../../corge/f';\n";
    let expected = SimpleDependencyInfo::builder("../bar/baz.js", "/foo/js/bar/baz.js")
        .set_provides(vec!["module$js$bar$baz"])
        .set_requires(vec![
            Require::es6_import("module$js$bar$a", "./a"),
            Require::es6_import("module$js$bar$qux$b", "./qux/b"),
            Require::es6_import("module$js$closure$c", "../closure/c"),
            Require::es6_import("module$js$closure$d$e", "../closure/d/e"),
            Require::es6_import("module$corge$f", "../../corge/f"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "es6".into(),
        )]))
        .build();

    let result = parser.set_module_loader(loader).parse_file(
        "/foo/js/bar/baz.js",
        "../bar/baz.js",
        contents,
    );

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseEs6ModuleWithGoogProvide
#[test]
fn test_parse_es6_module_with_goog_provide() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["/foo"])
        .set_inputs(Vec::<SimpleDependencyInfo>::new())
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let contents = "goog.provide('my.namespace');\nexport {};";

    let expected = SimpleDependencyInfo::builder("../bar/baz.js", "/foo/js/bar/baz.js")
        .set_provides(vec!["my.namespace"])
        .build();

    let result = parser.set_module_loader(loader).parse_file(
        "/foo/js/bar/baz.js",
        "../bar/baz.js",
        contents,
    );

    assert_eq!(result, expected);
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 0);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 1);
    assert_eq!(
        error_manager.lock().unwrap().get_warnings()[0].get_type(),
        &MODULE_CONFLICT
    );
}
// port: JsFileRegexParserTest#testEs6ModuleWithDeclareModuleId
#[test]
fn test_es6_module_with_declare_module_id() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["/foo"])
        .set_inputs(Vec::<SimpleDependencyInfo>::new())
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let contents = "goog.declareModuleId('my.namespace');\nexport {};";

    let expected = SimpleDependencyInfo::builder("../bar/baz.js", "/foo/js/bar/baz.js")
        .set_provides(vec!["my.namespace", "module$js$bar$baz"])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "es6".into(),
        )]))
        .build();

    let result = parser.set_module_loader(loader).parse_file(
        "/foo/js/bar/baz.js",
        "../bar/baz.js",
        contents,
    );

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testEs6ModuleWithBrowserTransformedPrefixResolver
#[test]
fn test_es6_module_with_browser_transformed_prefix_resolver() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(Vec::<SimpleDependencyInfo>::new())
        .set_factory(Arc::new(prefixes::Factory::new(
            IndexMap::<_, _>::from_iter([("@root/".into(), "/path/to/project/".into())]),
        )))
        .build();

    let contents = "import '@root/my/file.js';";

    let expected = SimpleDependencyInfo::builder("../bar/baz.js", "/foo/js/bar/baz.js")
        .set_provides(vec!["module$foo$js$bar$baz"])
        .set_requires([Require::es6_import(
            "module$path$to$project$my$file",
            "@root/my/file.js",
        )])
        .set_load_flags(IndexMap::<_, _>::from_iter([(
            "module".into(),
            "es6".into(),
        )]))
        .build();

    let result = parser.set_module_loader(loader).parse_file(
        "/foo/js/bar/baz.js",
        "../bar/baz.js",
        contents,
    );

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testNoShortcutForCommonModuleModifiers
#[test]
fn test_no_shortcut_for_common_module_modifiers() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.module('yes1');\ngoog.module.declareLegacyNamespace();\ngoog.setTestOnly();\nvar yes2=goog.require('yes2');\nvar C=goog.require(\"a.b.C\");\nconst {\n  D,\n  E\n}=goog.require(\"a.b.d\");\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_requires([
            Require::goog_require_symbol("yes2"),
            Require::goog_require_symbol("a.b.C"),
            Require::goog_require_symbol("a.b.d"),
        ])
        .set_goog_module(true)
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testMultiplePerLine
#[test]
fn test_multiple_per_line() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.provide('yes1');goog.provide('yes2');/*goog.provide('no1');*/goog.provide('yes3');//goog.provide('no2');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1", "yes2", "yes3"])
        .set_goog_module(false)
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testShortcutMode1
#[test]
fn test_shortcut_mode1() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "// hi !\n/* this is a comment */\ngoog.provide('yes1');\n/* and another comment */\ngoog.provide('yes2'); // include this\nfoo = function() {};\ngoog.provide('no1');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1", "yes2"])
        .set_goog_module(false)
        .build();
    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testShortcutMode2
#[test]
fn test_shortcut_mode2() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents =
        "/** goog.provide('no1'); \n * goog.provide('no2');\n */\ngoog.provide('yes1');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_goog_module(false)
        .build();
    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testShortcutMode3
#[test]
fn test_shortcut_mode3() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/**\n * goog.provide('no1');\n */\ngoog.provide('yes1');\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1"])
        .set_goog_module(false)
        .build();
    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog1
#[test]
fn test_include_goog1() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/**\n * @provideGoog\n */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["goog"])
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog1_quotes
#[test]
fn test_include_goog1_quotes() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "var x = \"/**\\\\n * @provideGoog\\\\n */\\\\n\";";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(Vec::<String>::new())
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog1_quotesSingleLine
#[test]
fn test_include_goog1_quotes_single_line() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "var x = \"/** @provideGoog */\";";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(Vec::<String>::new())
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog1_quotesBeforeComment
#[test]
fn test_include_goog1_quotes_before_comment() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "var x = \"foo\"; /** @provideGoog */";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["goog"])
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog1_multipleQuotes
#[test]
fn test_include_goog1_multiple_quotes() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "var x = \"foo\"; var y = \"/** @provideGoog */\";";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(Vec::<String>::new())
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog2
#[test]
fn test_include_goog2() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.require('bar');";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_requires(vec![
            Require::goog_require_symbol("goog"),
            Require::goog_require_symbol("bar"),
        ])
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog3
#[test]
fn test_include_goog3() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents =
        "goog.provide('x');\n/**\n * the first constant in base.js\n */\nvar COMPILED = false;\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["x"])
        .set_requires(vec![Require::goog_require_symbol("goog")])
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testIncludeGoog4
#[test]
fn test_include_goog4() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.addDependency('foo', [], []);\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_requires(vec![Require::goog_require_symbol("goog")])
        .set_goog_module(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testExternsAnnotation_basic_multiline
#[test]
fn test_externs_annotation_basic_multiline() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/**\n * @externs\n */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_externs_annotation(true)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testExternsAnnotation_basic_oneLine
#[test]
fn test_externs_annotation_basic_one_line() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/** @externs */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_externs_annotation(true)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testExternsAnnotation_blockComment
#[test]
fn test_externs_annotation_block_comment() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/* @externs */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_externs_annotation(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testExternsAnnotation_blockComment_multiline
#[test]
fn test_externs_annotation_block_comment_multiline() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/*\n @externs */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_externs_annotation(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testNoCompileAnnotation_basic_multiline
#[test]
fn test_no_compile_annotation_basic_multiline() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/**\n * @nocompile\n */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_no_compile_annotation(true)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testNoCompileAnnotation_basic_oneLine
#[test]
fn test_no_compile_annotation_basic_one_line() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/** @nocompile */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_no_compile_annotation(true)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testNoCompileAnnotation_blockComment
#[test]
fn test_no_compile_annotation_block_comment() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/* @nocompile */\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_has_no_compile_annotation(false)
        .build();
    let result = parser
        .set_include_goog_base(true)
        .parse_file(SRC_PATH, CLOSURE_PATH, contents);
    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testParseProvidesAndWrappedGoogModule
#[test]
fn test_parse_provides_and_wrapped_goog_module() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "goog.loadModule(function(){\"use strict\";goog.module('yes1');\ngoog.provide('my.provide');\nvar yes2=goog.require('yes2');\nvar C=goog.require(\"a.b.C\");\nconst {\n  D,\n  E\n}=goog.require(\"a.b.d\");});\n";

    let expected = SimpleDependencyInfo::builder(CLOSURE_PATH, SRC_PATH)
        .set_provides(vec!["yes1", "my.provide"])
        .set_requires(vec![
            Require::goog_require_symbol("yes2"),
            Require::goog_require_symbol("a.b.C"),
            Require::goog_require_symbol("a.b.d"),
        ])
        .set_load_flags(IndexMap::<_, _>::from_iter([]))
        .build();

    let result = parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert_deps(expected, result, &error_manager);
}
// port: JsFileRegexParserTest#testEs6AndWrappedGoogModuleIsError
#[test]
fn test_es6_and_wrapped_goog_module_is_error() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents =
        "goog.loadModule(function(){\"use strict\";goog.module('yes1');});\nexport {};\n";

    parser.parse_file(SRC_PATH, CLOSURE_PATH, contents);

    assert!(error_manager.lock().unwrap().get_errors().is_empty());
    assert_eq!(
        error_manager
            .lock()
            .unwrap()
            .get_warnings()
            .iter()
            .map(|e| e.get_type())
            .collect::<Vec<_>>(),
        vec![&MODULE_CONFLICT]
    );
}
// port: JsFileRegexParserTest#setUp
fn set_up() -> (JsFileRegexParser, SharedErrorManager) {
    let manager: SharedErrorManager = Arc::new(Mutex::new(BlackHoleErrorManager::new()));
    let mut parser = JsFileRegexParser::new(manager.clone());
    parser.set_shortcut_mode(true);
    (parser, manager)
}
// port: JsFileRegexParserTest#assertDeps
fn assert_deps(
    expected: SimpleDependencyInfo,
    actual: SimpleDependencyInfo,
    manager: &SharedErrorManager,
) {
    assert_eq!(actual, expected);
    assert!(manager.lock().unwrap().get_errors().is_empty());
    assert!(manager.lock().unwrap().get_warnings().is_empty());
}
