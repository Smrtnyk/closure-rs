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
//   test/com/google/javascript/jscomp/WarningsGuardTest.java.

use closure_jscomp::{
    by_path_warnings_guard::ByPathWarningsGuard,
    check_level::CheckLevel::{self, ERROR, OFF, WARNING},
    compose_warnings_guard::ComposeWarningsGuard,
    diagnostic_group::DiagnosticGroup,
    diagnostic_group_warnings_guard::DiagnosticGroupWarningsGuard,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    show_by_path_warnings_guard::{ShowByPathWarningsGuard, ShowType},
    strict_warnings_guard::StrictWarningsGuard,
    warnings_guard::WarningsGuard,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{jscomp_base::Tri, node::Ast, token::Token};
use std::{
    any::Any,
    fmt,
    sync::{Arc, LazyLock},
};
static BAR_WARNING: DiagnosticType = DiagnosticType::warning("BAR", "Bar description");
// Exact immutable registry values captured from the JVM. This fixture is local
// to the tests; the DiagnosticGroups registry is tested separately.
static GROUPS: LazyLock<IndexMap<String, Arc<DiagnosticGroup>>> = LazyLock::new(|| {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/diagnostic_groups.json")).unwrap();
    data.as_object()
        .unwrap()
        .iter()
        .map(|(key, g)| {
            let types: Vec<&'static DiagnosticType> = g["types"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    let level = match t["level"].as_str().unwrap() {
                        "ERROR" => ERROR,
                        "WARNING" => WARNING,
                        _ => OFF,
                    };
                    let type_: &'static DiagnosticType = Box::leak(Box::new(DiagnosticType::make(
                        String::leak(t["key"].as_str().unwrap().into()),
                        level,
                        String::leak(t["format"].as_str().unwrap().into()),
                    )));
                    type_
                })
                .collect();
            (
                key.clone(),
                Arc::new(DiagnosticGroup::new_named(
                    g["name"].as_str().unwrap(),
                    &types,
                )),
            )
        })
        .collect()
});
fn group(name: &str) -> Arc<DiagnosticGroup> {
    GROUPS[name].clone()
}
fn visibility(level: CheckLevel) -> Arc<dyn WarningsGuard> {
    Arc::new(DiagnosticGroupWarningsGuard::new(
        group("ACCESS_CONTROLS"),
        level,
    ))
}
// port: WarningsGuardTest#makeError(String)
fn make_error(source: Option<&str>) -> JSError {
    make_error_with_type(source, &BAR_WARNING)
}
// port: WarningsGuardTest#makeError(String,DiagnosticType)
fn make_error_with_type(source: Option<&str>, type_: &'static DiagnosticType) -> JSError {
    let mut ast = Ast::new();
    let n = ast.new_node(Token::EMPTY);
    if let Some(source) = source {
        n.set_source_file_for_testing(&mut ast, source);
    }
    JSError::make(&ast, n, type_, &[])
}
// port: WarningsGuardTest#makeError(String,CheckLevel)
fn make_error_with_level(source: Option<&str>, level: CheckLevel) -> JSError {
    let type_: &'static DiagnosticType = Box::leak(Box::new(DiagnosticType::make(
        "FOO",
        level,
        "Foo description",
    )));
    make_error_with_type(source, type_)
}
// port: WarningsGuardTest#makeError(String,int)
fn make_error_with_line(source: &str, lineno: i32) -> JSError {
    JSError::make_with_source_location(source, lineno, -1, &BAR_WARNING, &[])
}
// port: WarningsGuardTest#testShowByPathGuard_Restrict
#[test]
fn test_show_by_path_guard_restrict() {
    let g = ShowByPathWarningsGuard::new_with_show_type("/foo/", ShowType::INCLUDE);
    for (path, level, expected) in [
        ("asasasd/foo/hello.js", WARNING, None),
        ("asasasd/foo/hello.js", ERROR, None),
        ("asasasd/hello.js", WARNING, Some(OFF)),
        ("asasasd/hello.js", OFF, Some(OFF)),
        ("asasasd/hello.js", ERROR, None),
    ] {
        assert_eq!(g.level(&make_error_with_level(Some(path), level)), expected);
    }
    assert_eq!(g.level(&make_error(None)), None);
    assert_eq!(g.level(&make_error_with_level(None, WARNING)), None);
    assert_eq!(g.must_run_checks(&group("DEPRECATED")), Tri::UNKNOWN);
}
// port: WarningsGuardTest#testShowByPathGuard_Suppress
#[test]
fn test_show_by_path_guard_suppress() {
    let g = ShowByPathWarningsGuard::new_with_paths_and_show_type(
        &["/foo/", "/bar/"],
        ShowType::EXCLUDE,
    );
    for (path, level, expected) in [
        ("asasasd/foo/hello.js", WARNING, Some(OFF)),
        ("asasasd/foo/bar/hello.js", WARNING, Some(OFF)),
        ("asasasd/bar/hello.js", WARNING, Some(OFF)),
        ("asasasd/foo/bar/hello.js", WARNING, Some(OFF)),
        ("asasasd/foo/hello.js", ERROR, None),
        ("asasasd/hello.js", WARNING, None),
        ("asasasd/hello.js", OFF, None),
        ("asasasd/hello.js", ERROR, None),
    ] {
        assert_eq!(g.level(&make_error_with_level(Some(path), level)), expected);
    }
    assert_eq!(g.level(&make_error(None)), None);
    assert_eq!(g.level(&make_error_with_level(None, WARNING)), None);
    assert_eq!(g.must_run_checks(&group("DEPRECATED")), Tri::UNKNOWN);
}
// port: WarningsGuardTest#testStrictGuard
#[test]
fn test_strict_guard() {
    let g = StrictWarningsGuard;
    assert_eq!(
        g.level(&make_error_with_level(Some("foo/hello.js"), WARNING)),
        Some(ERROR)
    );
    assert_eq!(
        g.level(&make_error_with_level(Some("foo/hello.js"), OFF)),
        None
    );
    assert_eq!(
        g.level(&make_error_with_level(Some("bar.js"), ERROR)),
        Some(ERROR)
    );
    assert_eq!(g.must_run_checks(&group("DEPRECATED")), Tri::UNKNOWN);
}
// port: WarningsGuardTest#testByPathGuard
#[test]
fn test_by_path_guard() {
    let g = ByPathWarningsGuard::for_path(vec!["/foo/".into()], ERROR);
    for (path, level, expected) in [
        ("asasasd/foo/hello.js", WARNING, Some(ERROR)),
        ("asasasd/foo/hello.js", ERROR, None),
        ("asasasd/hello.js", WARNING, None),
        ("asasasd/hello.js", OFF, None),
        ("asasasd/hello.js", ERROR, None),
    ] {
        assert_eq!(g.level(&make_error_with_level(Some(path), level)), expected);
    }
    assert_eq!(g.level(&make_error(None)), None);
    assert_eq!(g.level(&make_error_with_level(None, WARNING)), None);
    assert_eq!(g.must_run_checks(&group("DEPRECATED")), Tri::UNKNOWN);
}
#[derive(Debug)]
struct TestGuard(bool);
impl fmt::Display for TestGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WarningsGuardTest.TestGuard@{:x}",
            self as *const Self as usize
        )
    }
}
impl WarningsGuard for TestGuard {
    // port: WarningsGuardTest.WarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if self.0 {
            if error.source_name() == Some("123456") {
                Some(ERROR)
            } else {
                None
            }
        } else if error.get_line_number() == 12 {
            Some(WARNING)
        } else {
            None
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
// port: WarningsGuardTest#testComposeGuard
#[test]
fn test_compose_guard() {
    let g = ComposeWarningsGuard::new(vec![Arc::new(TestGuard(true)), Arc::new(TestGuard(false))]);
    assert_eq!(g.level(&make_error(Some("aaa"))), None);
    assert_eq!(g.level(&make_error(Some("12345"))), None);
    assert_eq!(g.level(&make_error(Some("123456"))), Some(ERROR));
    assert_eq!(g.level(&make_error_with_line("12345", 12)), Some(WARNING));
    assert_eq!(g.level(&make_error_with_line("12345", 13)), None);
    assert_eq!(g.must_run_checks(&group("DEPRECATED")), Tri::UNKNOWN);
}
// port: WarningsGuardTest#testComposeGuard2
#[test]
fn test_compose_guard2() {
    let path: Arc<dyn WarningsGuard> = Arc::new(ShowByPathWarningsGuard::new("/foo/"));
    let strict: Arc<dyn WarningsGuard> = Arc::new(StrictWarningsGuard);
    for guards in [vec![strict.clone(), path.clone()], vec![path, strict]] {
        let g = ComposeWarningsGuard::new(guards);
        assert_eq!(
            g.level(&make_error_with_level(Some("asasasd/hello.js"), WARNING)),
            Some(OFF)
        );
        assert_eq!(
            g.level(&make_error_with_level(
                Some("asasasd/foo/hello.js"),
                WARNING
            )),
            Some(ERROR)
        );
    }
}
// port: WarningsGuardTest#testComposeGuard3
#[test]
fn test_compose_guard3() {
    let type_guard: Arc<dyn WarningsGuard> = Arc::new(DiagnosticGroupWarningsGuard::new(
        group("DEPRECATED"),
        WARNING,
    ));
    let strict: Arc<dyn WarningsGuard> = Arc::new(StrictWarningsGuard);
    let deprecated = group("DEPRECATED");
    let name = *deprecated
        .get_types()
        .iter()
        .find(|t| t.key == "JSC_DEPRECATED_VAR")
        .unwrap();
    for guards in [
        vec![strict.clone(), type_guard.clone()],
        vec![type_guard, strict],
    ] {
        let g = ComposeWarningsGuard::new(guards);
        assert_eq!(
            g.level(&JSError::make_with_source_location(
                "example.js",
                1,
                0,
                name,
                &[]
            )),
            Some(WARNING)
        );
    }
}
// port: WarningsGuardTest#testComposeGuardOrdering
#[test]
fn test_compose_guard_ordering() {
    let g = ComposeWarningsGuard::new(vec![
        Arc::new(ShowByPathWarningsGuard::new("/foo/")),
        visibility(OFF),
        Arc::new(StrictWarningsGuard),
        visibility(OFF),
        Arc::new(ShowByPathWarningsGuard::new("/bar/")),
        visibility(OFF),
    ]);
    let guards = g.get_guards();
    assert_eq!(guards.len(), 6);
    let mut prev = i32::MIN;
    for guard in guards {
        assert!(guard.get_priority() >= prev);
        prev = guard.get_priority();
    }
}
// port: WarningsGuardTest#testComposeGuardOrdering2
#[test]
fn test_compose_guard_ordering2() {
    let off = visibility(OFF);
    let warning = visibility(WARNING);
    let a = ComposeWarningsGuard::new(vec![]);
    a.add_guard(warning.clone());
    a.add_guard(off.clone());
    a.add_guard(warning.clone());
    let b = ComposeWarningsGuard::new(vec![]);
    b.add_guard(off.clone());
    b.add_guard(warning);
    b.add_guard(off);
    assert_eq!(a.must_run_checks(&group("ACCESS_CONTROLS")), Tri::TRUE);
    assert_eq!(b.must_run_checks(&group("ACCESS_CONTROLS")), Tri::FALSE);
}
// port: WarningsGuardTest#testComposeGuardOrdering3
#[test]
fn test_compose_guard_ordering3() {
    let a = ComposeWarningsGuard::new(vec![]);
    for _ in 0..4 {
        a.add_guard(visibility(WARNING));
    }
    a.add_guard(visibility(OFF));
    assert_eq!(a.must_run_checks(&group("ACCESS_CONTROLS")), Tri::FALSE);
}
// port: WarningsGuardTest#testComposeGuardOrdering4
#[test]
fn test_compose_guard_ordering4() {
    let a = ComposeWarningsGuard::new(vec![]);
    let warning = visibility(WARNING);
    for _ in 0..6 {
        a.add_guard(warning.clone());
    }
    a.add_guard(visibility(OFF));
    assert_eq!(a.must_run_checks(&group("ACCESS_CONTROLS")), Tri::FALSE);
}
// port: WarningsGuardTest#testDiagnosticGuard1
#[test]
fn test_diagnostic_guard1() {
    let check_types = group("CHECK_TYPES");
    let g = DiagnosticGroupWarningsGuard::new(check_types.clone(), ERROR);
    let deterministic = *check_types
        .get_types()
        .iter()
        .find(|t| t.key == "JSC_DETERMINISTIC_TEST")
        .unwrap();
    assert_eq!(
        g.level(&make_error_with_type(Some("foo"), deterministic)),
        Some(ERROR)
    );
    assert_eq!(g.must_run_checks(&check_types), Tri::TRUE);
    assert_eq!(
        g.must_run_checks(&group("MESSAGE_DESCRIPTIONS")),
        Tri::UNKNOWN
    );
}
// port: WarningsGuardTest#testDiagnosticGuard3
#[test]
fn test_diagnostic_guard3() {
    let g = DiagnosticGroupWarningsGuard::new(group("CHECK_TYPES"), OFF);
    assert_eq!(g.must_run_checks(&group("CHECK_TYPES")), Tri::FALSE);
    assert_eq!(
        g.must_run_checks(&group("MESSAGE_DESCRIPTIONS")),
        Tri::UNKNOWN
    );
}
// port: WarningsGuardTest#testDiagnosticGuard4
#[test]
fn test_diagnostic_guard4() {
    let g = DiagnosticGroupWarningsGuard::new(group("DEPRECATED"), OFF);
    assert_eq!(g.must_run_checks(&group("DEPRECATED")), Tri::FALSE);
    for name in ["VISIBILITY", "ACCESS_CONTROLS", "MESSAGE_DESCRIPTIONS"] {
        assert_eq!(g.must_run_checks(&group(name)), Tri::UNKNOWN);
    }
}
// port: WarningsGuardTest#testComposeGuardCycle
#[test]
fn test_compose_guard_cycle() {
    let g = Arc::new(ComposeWarningsGuard::new(vec![
        visibility(OFF),
        visibility(WARNING),
    ]));
    g.add_guard(g.clone());
    assert_eq!(
        g.to_string(),
        "DiagnosticGroup<visibility>(WARNING), DiagnosticGroup<visibility>(OFF)"
    );
}
