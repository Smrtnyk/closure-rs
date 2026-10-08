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
//   test/com/google/javascript/jscomp/deps/DepsFileRegexParserTest.java.

use super::*;
use crate::black_hole_error_manager::BlackHoleErrorManager;
use crate::deps::simple_dependency_info::SimpleDependencyInfo;
use std::sync::Mutex;
const SRC_PATH: &str = "/path/1.js";
// port: DepsFileRegexParserTest#testGoodParse
#[test]
fn test_good_parse() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let contents = "/*goog.addDependency('no1', [], []);*//*\ngoog.addDependency('no2', [ ], [ ]);\n*/goog.addDependency('yes1', [], []);\n/* blah */goog.addDependency(\"yes2\", [], [])/* blah*/\ngoog.addDependency('yes3', ['a','b'], ['c']); // goog.addDependency('no3', [], []);\n// goog.addDependency('no4', [], []);\ngoog.addDependency(\"yes4\", [], [ \"a\",'b' , 'c' ]); //no new line at EOF\n";

    let result = parser.parse_file(SRC_PATH, contents);
    let expected = vec![
        SimpleDependencyInfo::builder("yes1", SRC_PATH).build(),
        SimpleDependencyInfo::builder("yes2", SRC_PATH).build(),
        SimpleDependencyInfo::builder("yes3", SRC_PATH)
            .set_provides(vec!["a", "b"])
            .set_requires([Require::parsed_from_deps("c")])
            .build(),
        SimpleDependencyInfo::builder("yes4", SRC_PATH)
            .set_requires([
                Require::parsed_from_deps("a"),
                Require::parsed_from_deps("b"),
                Require::parsed_from_deps("c"),
            ])
            .build(),
    ];

    assert_eq!(result, expected);
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 0);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 0);
}
// port: DepsFileRegexParserTest#testTooFewArgs
#[test]
fn test_too_few_args() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.parse_file(SRC_PATH, "goog.addDependency('a', []);");
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 1);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 0);
}
// port: DepsFileRegexParserTest#testTooManyArgs1
#[test]
fn test_too_many_args1() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.parse_file(SRC_PATH, "goog.addDependency('a', [], [], []);");
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 1);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 0);
}
// port: DepsFileRegexParserTest#testTooManyArgs2
#[test]
fn test_too_many_args2() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.parse_file(SRC_PATH, "goog.addDependency('a', [], [], false, []);");
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 1);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 0);
}
// port: DepsFileRegexParserTest#testTooManyArgs3
#[test]
fn test_too_many_args3() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.parse_file(SRC_PATH, "goog.addDependency('a', [], [], {}, []);");
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 1);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 0);
}
// port: DepsFileRegexParserTest#testBadLoadFlagsSyntax
#[test]
fn test_bad_load_flags_syntax() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.parse_file(
        SRC_PATH,
        "goog.addDependency('a', [], [], {module: 'goog'});",
    );
    assert_eq!(error_manager.lock().unwrap().get_error_count(), 1);
    assert_eq!(error_manager.lock().unwrap().get_warning_count(), 0);
}
// port: DepsFileRegexParserTest#testGoogModule
#[test]
fn test_goog_module() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let result = parser.parse_file(
        SRC_PATH,
        "goog.addDependency('yes1', [], [], true);\ngoog.addDependency('yes2', [], [], false);\n",
    );
    let expected = vec![
        SimpleDependencyInfo::builder("yes1", SRC_PATH)
            .set_goog_module(true)
            .build(),
        SimpleDependencyInfo::builder("yes2", SRC_PATH).build(),
    ];
    assert_eq!(result, expected);
}
// port: DepsFileRegexParserTest#testEs6Module
#[test]
fn test_es6_module() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let result =
        parser.parse_file(
            SRC_PATH,
            "goog.addDependency('path/from/closure.js', [], ['nexttoclosure.js'], {'module':'es6'});\ngoog.addDependency('nexttoclosure.js', [], [], {'module':'es6'});\n");
    let expected = vec![
        SimpleDependencyInfo::builder("path/from/closure.js", SRC_PATH)
            .set_load_flags(IndexMap::from([("module".into(), "es6".into())]))
            .set_provides(vec!["path/from/closure.js"])
            .set_requires([Require::parsed_from_deps("nexttoclosure.js")])
            .build(),
        SimpleDependencyInfo::builder("nexttoclosure.js", SRC_PATH)
            .set_load_flags(IndexMap::from([("module".into(), "es6".into())]))
            .set_provides(vec!["nexttoclosure.js"])
            .build(),
    ];
    assert_eq!(result, expected);
}
// port: DepsFileRegexParserTest#testLoadFlags
#[test]
fn test_load_flags() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let result =
        parser.parse_file(
            SRC_PATH,
            "goog.addDependency('yes1', [], [], {'module': 'goog'});\ngoog.addDependency('yes2', [], [], {\"lang\": \"es6\"});\ngoog.addDependency('yes3', [], [], {});\n");
    let expected = vec![
        SimpleDependencyInfo::builder("yes1", SRC_PATH)
            .set_load_flags(IndexMap::from([("module".into(), "goog".into())]))
            .build(),
        SimpleDependencyInfo::builder("yes2", SRC_PATH)
            .set_load_flags(IndexMap::from([("lang".into(), "es6".into())]))
            .build(),
        SimpleDependencyInfo::builder("yes3", SRC_PATH).build(),
    ];
    assert_eq!(result, expected);
}
// port: DepsFileRegexParserTest#testShortcutMode
#[test]
fn test_shortcut_mode() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    let result = parser.parse_file(
        SRC_PATH,
        "goog.addDependency('yes1', [], []); \nfoo();\ngoog.addDependency('no1', [], []);\n",
    );
    let expected = vec![SimpleDependencyInfo::builder("yes1", SRC_PATH).build()];
    assert_eq!(result, expected);
}
// port: DepsFileRegexParserTest#testNoShortcutMode
#[test]
fn test_no_shortcut_mode() {
    let (mut parser, error_manager) = set_up();
    let _ = &error_manager;

    parser.set_shortcut_mode(false);
    let result = parser.parse_file(
        SRC_PATH,
        "goog.addDependency('yes1', [], []); \nfoo();\ngoog.addDependency('yes2', [], []);\n",
    );
    let expected = vec![
        SimpleDependencyInfo::builder("yes1", SRC_PATH).build(),
        SimpleDependencyInfo::builder("yes2", SRC_PATH).build(),
    ];
    assert_eq!(result, expected);
}
// port: DepsFileRegexParserTest#setUp
fn set_up() -> (DepsFileRegexParser, SharedErrorManager) {
    let manager: SharedErrorManager = Arc::new(Mutex::new(BlackHoleErrorManager::new()));
    let mut parser = DepsFileRegexParser::new(manager.clone());
    parser.set_shortcut_mode(true);
    (parser, manager)
}
