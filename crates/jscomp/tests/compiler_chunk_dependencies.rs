/*
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/JSChunkGraphTest.java,
//   test/com/google/javascript/jscomp/JSChunkTest.java.

#![allow(clippy::mutable_key_type, clippy::cloned_ref_to_slice_refs)] // Preserve Java shared-object fixtures during compiler merge integration.
use closure_jscomp::{
    compiler::Compiler,
    compiler_input::CompilerInput,
    dependency_options::DependencyOptions,
    deps::dependency_info::Require,
    js_chunk::{JSChunk, WEAK_CHUNK_NAME},
    js_chunk_graph::JSChunkGraph,
    module_identifier::ModuleIdentifier,
    source_file::SourceFile,
};
use closure_rhino::static_source_file::{SourceKind, StaticSourceFile};
use std::sync::Arc;
// port: JSChunkGraphTest#makeDeps
fn make_deps() -> [JSChunk; 6] {
    let a = JSChunk::new("chunkA");
    let b = JSChunk::new("chunkB");
    let c = JSChunk::new("chunkC");
    let d = JSChunk::new("chunkD");
    let e = JSChunk::new("chunkE");
    let f = JSChunk::new("chunkF");
    b.add_dependency(&a);
    c.add_dependency(&a);
    d.add_dependency(&b);
    e.add_dependency(&b);
    e.add_dependency(&c);
    f.add_dependency(&a);
    f.add_dependency(&c);
    f.add_dependency(&e);
    [a, b, c, d, e, f]
}
// port: JSChunkGraphTest#code
fn code(name: &str, provides: &[&str], requires: &[&str], source: &str) -> Arc<SourceFile> {
    let mut text = String::new();
    for p in provides {
        text.push_str(&format!("goog.provide('{p}');\n"));
    }
    for r in requires {
        text.push_str(&format!("goog.require('{r}');\n"));
    }
    text.push_str(source);
    Arc::new(SourceFile::from_code(name, text))
}
// port: JSChunkGraphTest#sourceNames
fn names(inputs: &[CompilerInput]) -> Vec<&str> {
    inputs.iter().map(CompilerInput::get_name).collect()
}
// port: JSChunkGraphTest#assertInputs
fn assert_inputs(chunk: &JSChunk, expected: &[&str]) {
    assert_eq!(names(&chunk.get_inputs()), expected);
}
// port: JSChunkGraphTest#setUpManageDependenciesTest
fn setup_inputs(compiler: &Compiler, chunks: &[JSChunk; 6]) {
    for (index, name, provides, requires) in [
        (0, "a1", vec!["a1"], vec![]),
        (0, "a2", vec!["a2"], vec!["a1"]),
        (0, "a3", vec![], vec!["a1"]),
        (1, "b1", vec!["b1"], vec!["a2"]),
        (1, "b2", vec![], vec!["a1", "a2"]),
        (2, "c1", vec!["c1"], vec!["a1"]),
        (2, "c2", vec!["c2"], vec!["c1"]),
        (4, "e1", vec![], vec!["c1"]),
        (4, "e2", vec![], vec!["c1"]),
    ] {
        chunks[index].add_source_file(code(name, &provides, &requires, ""));
    }
    for chunk in chunks {
        for input in chunk.get_inputs() {
            input.set_compiler(compiler);
        }
    }
}
const BASEJS: &str =
    "/** @fileoverview\n * @provideGoog */\nvar COMPILED = false;\nvar goog = goog || {}\n";
// port: JSChunkTest#testSortInputs
#[test]
fn test_sort_inputs() {
    let [a, b, c, d, e, f] = [
        ("a.js", "goog.require('b');goog.require('c')"),
        ("b.js", "goog.provide('b');goog.require('d')"),
        ("c.js", "goog.provide('c');goog.require('d')"),
        ("d.js", "goog.provide('d')"),
        ("e.js", "goog.provide('e')"),
        ("f.js", "goog.provide('f')"),
    ]
    .map(|(name, code)| CompilerInput::new(SourceFile::from_code(name, code)));
    let pairs = [
        (vec![&d, &b, &c, &a], vec![&a, &b, &c, &d]),
        (vec![&d, &b, &c, &a], vec![&d, &b, &c, &a]),
        (vec![&d, &c, &b, &a], vec![&d, &c, &b, &a]),
        (vec![&d, &b, &c, &a], vec![&d, &a, &b, &c]),
        (vec![&d, &b, &c, &a, &e, &f], vec![&a, &b, &c, &d, &e, &f]),
        (vec![&e, &f, &d, &b, &c, &a], vec![&e, &f, &a, &b, &c, &d]),
        (vec![&d, &b, &c, &a, &e, &f], vec![&a, &b, &c, &e, &d, &f]),
        (vec![&e, &d, &b, &c, &a, &f], vec![&e, &a, &f, &b, &c, &d]),
    ];
    for (expected, shuffled) in pairs {
        let chunk = JSChunk::new("chunk");
        for input in shuffled {
            input.set_chunk(None);
            chunk.add(input.clone());
        }
        let mut compiler = Compiler::new();
        compiler.init_compiler_options_if_testing();
        chunk.sort_inputs_by_deps(&mut compiler);
        assert_eq!(
            chunk.get_inputs(),
            expected.into_iter().cloned().collect::<Vec<_>>()
        );
    }
}
// port: JSChunkGraphTest#testManageDependenciesLooseWithoutEntryPoint
#[test]
fn test_manage_dependencies_loose_without_entry_point() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    setup_inputs(&compiler, &chunks);
    let results = graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_legacy_for_entry_points([]),
        )
        .unwrap();
    assert_inputs(&chunks[0], &["a1", "a3"]);
    assert_inputs(&chunks[1], &["a2", "b2"]);
    assert_inputs(&chunks[2], &[]);
    assert_inputs(&chunks[4], &["c1", "e1", "e2"]);
    assert_eq!(
        names(&results.ordered_inputs),
        ["a1", "a3", "a2", "b2", "c1", "e1", "e2"]
    );
}
// port: JSChunkGraphTest#testManageDependenciesLooseWithEntryPoint
#[test]
fn test_manage_dependencies_loose_with_entry_point() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    setup_inputs(&compiler, &chunks);
    let results = graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_legacy_for_entry_points([ModuleIdentifier::for_closure(
                "c2",
            )]),
        )
        .unwrap();
    assert_inputs(&chunks[0], &["a1", "a3"]);
    assert_inputs(&chunks[1], &["a2", "b2"]);
    assert_inputs(&chunks[2], &["c1", "c2"]);
    assert_inputs(&chunks[4], &["e1", "e2"]);
    assert_eq!(
        names(&results.ordered_inputs),
        ["a1", "a3", "a2", "b2", "c1", "c2", "e1", "e2"]
    );
}
// port: JSChunkGraphTest#testManageDependenciesStrictWithEntryPoint
#[test]
fn test_manage_dependencies_strict_with_entry_point() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    setup_inputs(&compiler, &chunks);
    let results = graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([ModuleIdentifier::for_closure("c2")]),
        )
        .unwrap();
    assert_inputs(&chunks[0], &[]);
    assert_inputs(&chunks[1], &[]);
    assert_inputs(&chunks[2], &["a1", "c1", "c2"]);
    assert_inputs(&chunks[4], &[]);
    assert_eq!(names(&results.ordered_inputs), ["a1", "c1", "c2"]);
}
// port: JSChunkGraphTest#testManageDependenciesStrictForGoogRequireDynamic
#[test]
fn test_manage_dependencies_strict_for_goog_require_dynamic() {
    let mut compiler = Compiler::new();
    let chunk = JSChunk::new("chunk");
    let graph = JSChunkGraph::new(vec![chunk.clone()]).unwrap();
    let input = CompilerInput::new(code("a1", &["a1"], &[], ""));
    input.add_require_dynamic_imports("a2");
    chunk.add(input);
    chunk.add_source_file(code("a2", &["a2"], &[], ""));
    for input in chunk.get_inputs() {
        input.set_compiler(&compiler);
    }
    let results = graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([ModuleIdentifier::for_closure("a1")]),
        )
        .unwrap();
    assert_inputs(&chunk, &["a1", "a2"]);
    let mut actual = names(&results.ordered_inputs);
    actual.sort();
    assert_eq!(actual, ["a1", "a2"]);
}
// port: JSChunkGraphTest#testManageDependenciesStrictWithEntryPointWithDuplicates
#[test]
fn test_manage_dependencies_strict_with_entry_point_with_duplicates() {
    let mut compiler = Compiler::new();
    let chunk = JSChunk::new("a");
    let graph = JSChunkGraph::new(vec![chunk.clone()]).unwrap();
    for file in [
        code("a1", &["a1"], &["a2"], ""),
        code("a2", &["a2"], &[], ""),
        code("a3", &["a2"], &[], ""),
    ] {
        chunk.add_source_file(file);
    }
    for input in chunk.get_inputs() {
        input.set_compiler(&compiler);
    }
    let results = graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([ModuleIdentifier::for_closure("a1")]),
        )
        .unwrap();
    assert_inputs(&chunk, &["a2", "a3", "a1"]);
    assert_eq!(names(&results.ordered_inputs), ["a2", "a3", "a1"]);
}
// port: JSChunkGraphTest#testManageDependenciesSortOnly
#[test]
fn test_manage_dependencies_sort_only() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    setup_inputs(&compiler, &chunks);
    let results = graph
        .manage_dependencies(&mut compiler, &DependencyOptions::sort_only())
        .unwrap();
    assert_inputs(&chunks[0], &["a1", "a2", "a3"]);
    assert_inputs(&chunks[1], &["b1", "b2"]);
    assert_inputs(&chunks[2], &["c1", "c2"]);
    assert_inputs(&chunks[4], &["e1", "e2"]);
    assert_eq!(
        names(&results.ordered_inputs),
        ["a1", "a2", "a3", "b1", "b2", "c1", "c2", "e1", "e2"]
    );
}
// port: JSChunkGraphTest#testManageDependenciesSortOnlyImpl
#[test]
fn test_manage_dependencies_sort_only_impl() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    for file in [
        code("a2", &["a2"], &["a1"], ""),
        code("a1", &["a1"], &[], ""),
        code("base.js", &[], &[], BASEJS),
    ] {
        chunks[0].add_source_file(file);
    }
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let results = graph
        .manage_dependencies(&mut compiler, &DependencyOptions::sort_only())
        .unwrap();
    assert_inputs(&chunks[0], &["base.js", "a1", "a2"]);
    assert_eq!(names(&results.ordered_inputs), ["base.js", "a1", "a2"]);
}
// port: JSChunkGraphTest#testNoFiles
#[test]
fn test_no_files() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    assert!(
        graph
            .manage_dependencies(&mut compiler, &DependencyOptions::sort_only())
            .unwrap()
            .ordered_inputs
            .is_empty()
    );
}
fn shuffle<T>(values: &mut [T], state: &mut u64) {
    for i in (1..values.len()).rev() {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        values.swap(i, (*state >> 32) as usize % (i + 1));
    }
}
// port: JSChunkGraphTest#testGoogBaseOrderedCorrectly
#[test]
fn test_goog_base_ordered_correctly() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    let mut files = Vec::new();
    for n in (2..=9).rev() {
        let name = format!("a{n}");
        files.push(code(&name, &[&name], &[], ""));
    }
    files.push(code(
        "a1",
        &["a1"],
        &["a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9"],
        "",
    ));
    files.push(code("base.js", &[], &[], BASEJS));
    let mut rng = 0x58289;
    for _ in 0..10 {
        shuffle(&mut files, &mut rng);
        chunks[0].remove_all();
        for file in &files {
            chunks[0].add_source_file(file.clone());
        }
        for input in chunks[0].get_inputs() {
            input.set_compiler(&compiler);
        }
        let results = graph
            .manage_dependencies(
                &mut compiler,
                &DependencyOptions::prune_for_entry_points([ModuleIdentifier::for_closure("a1")]),
            )
            .unwrap();
        let expected = [
            "base.js", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9", "a1",
        ];
        assert_inputs(&chunks[0], &expected);
        assert_eq!(names(&results.ordered_inputs), expected);
    }
}
// port: JSChunkGraphTest#testProperEs6ModuleOrdering
#[test]
fn test_proper_es6_module_ordering() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    let mut files = [
        "/entry.js",
        "/a/a.js",
        "/a/b.js",
        "/b/a.js",
        "/b/b.js",
        "/b/c.js",
        "/important.js",
    ]
    .map(|name| code(name, &[], &[], ""));
    let mut rng = 0x75635;
    for _ in 0..10 {
        shuffle(&mut files, &mut rng);
        chunks[0].remove_all();
        for file in &files {
            chunks[0].add_source_file(file.clone());
        }
        for input in chunks[0].get_inputs() {
            input.set_compiler(&compiler);
            let requires = match input.get_name() {
                "/entry.js" => vec!["/b/b.js", "/b/a.js", "/important.js", "/a/b.js", "/a/a.js"],
                "/b/b.js" => vec!["/b/c.js"],
                _ => vec![],
            };
            for require in requires {
                input.add_ordered_require(Require::compiler_chunk(
                    &ModuleIdentifier::for_file(require).to_string(),
                ));
            }
            input.set_has_full_parse_dependency_info(true);
        }
        let results = graph
            .manage_dependencies(
                &mut compiler,
                &DependencyOptions::prune_for_entry_points([ModuleIdentifier::for_file(
                    "/entry.js",
                )]),
            )
            .unwrap();
        let expected = [
            "/b/c.js",
            "/b/b.js",
            "/b/a.js",
            "/important.js",
            "/a/b.js",
            "/a/a.js",
            "/entry.js",
        ];
        assert_inputs(&chunks[0], &expected);
        assert_eq!(names(&results.ordered_inputs), expected);
    }
}
fn assert_files(chunk: &JSChunk, expected: &[Arc<SourceFile>]) {
    let actual = chunk.get_inputs();
    assert_eq!(actual.len(), expected.len());
    for file in expected {
        assert!(
            actual
                .iter()
                .any(|input| Arc::ptr_eq(&input.get_source_file_arc(), file)),
            "{}",
            file.get_name()
        );
    }
}

// port: JSChunkGraphTest#testMoveMarkedWeakSourcesDuringManageDepsSortOnly
#[test]
fn test_move_marked_weak_sources_during_manage_deps_sort_only() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak1 = Arc::new(SourceFile::from_code_with_kind(
        "weak1",
        "",
        SourceKind::WEAK,
    ));
    let weak2 = Arc::new(SourceFile::from_code_with_kind(
        "weak2",
        "",
        SourceKind::WEAK,
    ));
    let strong1 = Arc::new(SourceFile::from_code_with_kind(
        "strong1",
        "",
        SourceKind::STRONG,
    ));
    let strong2 = Arc::new(SourceFile::from_code_with_kind(
        "strong2",
        "",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak1.clone());
    chunks[0].add_source_file(strong1.clone());
    chunks[0].add_source_file(weak2.clone());
    chunks[0].add_source_file(strong2.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(&mut compiler, &DependencyOptions::sort_only())
        .unwrap();
    assert_files(
        &graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(),
        &[weak1.clone(), weak2.clone()],
    );
    assert_files(&chunks[0], &[strong1.clone(), strong2.clone()]);
}

// port: JSChunkGraphTest#testIgnoreMarkedWeakSourcesDuringManageDepsPrune
#[test]
fn test_ignore_marked_weak_sources_during_manage_deps_prune() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak1 = Arc::new(SourceFile::from_code_with_kind(
        "weak1",
        "",
        SourceKind::WEAK,
    ));
    let weak2 = Arc::new(SourceFile::from_code_with_kind(
        "weak2",
        "",
        SourceKind::WEAK,
    ));
    let strong1 = Arc::new(SourceFile::from_code_with_kind(
        "strong1",
        "",
        SourceKind::STRONG,
    ));
    let strong2 = Arc::new(SourceFile::from_code_with_kind(
        "strong2",
        "",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak1.clone());
    chunks[0].add_source_file(strong1.clone());
    chunks[0].add_source_file(weak2.clone());
    chunks[0].add_source_file(strong2.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([
                ModuleIdentifier::for_file("strong1"),
                ModuleIdentifier::for_file("strong2"),
            ]),
        )
        .unwrap();
    assert_files(&graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(), &[]);
    assert_files(&chunks[0], &[strong1.clone(), strong2.clone()]);
}

// port: JSChunkGraphTest#testIgnoreDepsOfMarkedWeakSourcesDuringManageDepsPrune
#[test]
fn test_ignore_deps_of_marked_weak_sources_during_manage_deps_prune() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak1 = Arc::new(SourceFile::from_code_with_kind(
        "weak1",
        "goog.requireType('weak1weak');",
        SourceKind::WEAK,
    ));
    let weak1weak = Arc::new(SourceFile::from_code_with_kind(
        "weak1weak",
        "goog.provide('weak1weak');",
        SourceKind::WEAK,
    ));
    let weak2 = Arc::new(SourceFile::from_code_with_kind(
        "weak2",
        "goog.require('weak2strong');",
        SourceKind::WEAK,
    ));
    let weak2strong = Arc::new(SourceFile::from_code_with_kind(
        "weak2strong",
        "goog.provide('weak2strong');",
        SourceKind::WEAK,
    ));
    let strong1 = Arc::new(SourceFile::from_code_with_kind(
        "strong1",
        "",
        SourceKind::STRONG,
    ));
    let strong2 = Arc::new(SourceFile::from_code_with_kind(
        "strong2",
        "",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak1.clone());
    chunks[0].add_source_file(strong1.clone());
    chunks[0].add_source_file(weak2.clone());
    chunks[0].add_source_file(strong2.clone());
    chunks[0].add_source_file(weak1weak.clone());
    chunks[0].add_source_file(weak2strong.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([
                ModuleIdentifier::for_file("strong1"),
                ModuleIdentifier::for_file("strong2"),
            ]),
        )
        .unwrap();
    assert_files(&graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(), &[]);
    assert_files(&chunks[0], &[strong1.clone(), strong2.clone()]);
}

// port: JSChunkGraphTest#testMoveImplicitWeakSourcesFromMoocherDuringManageDepsLegacyPrune
#[test]
fn test_move_implicit_weak_sources_from_moocher_during_manage_deps_legacy_prune() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak = Arc::new(SourceFile::from_code_with_kind(
        "weak",
        "goog.provide('weak');",
        SourceKind::STRONG,
    ));
    let strong = Arc::new(SourceFile::from_code_with_kind(
        "strong",
        "",
        SourceKind::STRONG,
    ));
    let moocher = Arc::new(SourceFile::from_code_with_kind(
        "moocher",
        "goog.requireType('weak');",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak.clone());
    chunks[0].add_source_file(strong.clone());
    chunks[0].add_source_file(moocher.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_legacy_for_entry_points([ModuleIdentifier::for_file(
                "strong",
            )]),
        )
        .unwrap();
    assert_files(
        &graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(),
        std::slice::from_ref(&weak),
    );
    assert_files(&chunks[0], &[strong.clone(), moocher.clone()]);
}

// port: JSChunkGraphTest#testImplicitWeakSourcesNotMovedDuringManageDepsSortOnly
#[test]
fn test_implicit_weak_sources_not_moved_during_manage_deps_sort_only() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak1 = Arc::new(SourceFile::from_code_with_kind(
        "weak1",
        "goog.provide('weak1');",
        SourceKind::STRONG,
    ));
    let weak2 = Arc::new(SourceFile::from_code_with_kind(
        "weak2",
        "goog.provide('weak2');",
        SourceKind::STRONG,
    ));
    let strong1 = Arc::new(SourceFile::from_code_with_kind(
        "strong1",
        "goog.requireType('weak1');",
        SourceKind::STRONG,
    ));
    let strong2 = Arc::new(SourceFile::from_code_with_kind(
        "strong2",
        "goog.requireType('weak2');",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak1.clone());
    chunks[0].add_source_file(strong1.clone());
    chunks[0].add_source_file(weak2.clone());
    chunks[0].add_source_file(strong2.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(&mut compiler, &DependencyOptions::sort_only())
        .unwrap();
    assert_files(&graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(), &[]);
    assert_files(
        &chunks[0],
        &[
            weak1.clone(),
            strong1.clone(),
            weak2.clone(),
            strong2.clone(),
        ],
    );
}

// port: JSChunkGraphTest#testImplicitWeakSourcesMovedDuringManageDepsPrune
#[test]
fn test_implicit_weak_sources_moved_during_manage_deps_prune() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak1 = Arc::new(SourceFile::from_code_with_kind(
        "weak1",
        "goog.provide('weak1');",
        SourceKind::STRONG,
    ));
    let weak2 = Arc::new(SourceFile::from_code_with_kind(
        "weak2",
        "goog.provide('weak2');",
        SourceKind::STRONG,
    ));
    let strong1 = Arc::new(SourceFile::from_code_with_kind(
        "strong1",
        "goog.requireType('weak1');",
        SourceKind::STRONG,
    ));
    let strong2 = Arc::new(SourceFile::from_code_with_kind(
        "strong2",
        "goog.requireType('weak2');",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak1.clone());
    chunks[0].add_source_file(strong1.clone());
    chunks[0].add_source_file(weak2.clone());
    chunks[0].add_source_file(strong2.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([
                ModuleIdentifier::for_file("strong1"),
                ModuleIdentifier::for_file("strong2"),
            ]),
        )
        .unwrap();
    assert_files(
        &graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(),
        &[weak1.clone(), weak2.clone()],
    );
    assert_files(&chunks[0], &[strong1.clone(), strong2.clone()]);
}

// port: JSChunkGraphTest#testTransitiveWeakSources
#[test]
fn test_transitive_weak_sources() {
    let mut compiler = Compiler::new();
    let chunks = make_deps();
    let weak1 = Arc::new(SourceFile::from_code_with_kind(
        "weak1",
        "goog.provide('weak1'); goog.requireType('weak2'); goog.require('strongFromWeak');",
        SourceKind::STRONG,
    ));
    let strong_from_weak = Arc::new(SourceFile::from_code_with_kind(
        "strongFromWeak",
        "goog.provide('strongFromWeak');",
        SourceKind::STRONG,
    ));
    let weak2 = Arc::new(SourceFile::from_code_with_kind(
        "weak2",
        "goog.provide('weak2'); goog.requireType('weak3');",
        SourceKind::STRONG,
    ));
    let weak3 = Arc::new(SourceFile::from_code_with_kind(
        "weak3",
        "goog.provide('weak3');",
        SourceKind::STRONG,
    ));
    let strong1 = Arc::new(SourceFile::from_code_with_kind(
        "strong1",
        "goog.requireType('weak1');",
        SourceKind::STRONG,
    ));
    chunks[0].add_source_file(weak1.clone());
    chunks[0].add_source_file(strong1.clone());
    chunks[0].add_source_file(weak2.clone());
    chunks[0].add_source_file(weak3.clone());
    chunks[0].add_source_file(strong_from_weak.clone());
    for input in chunks[0].get_inputs() {
        input.set_compiler(&compiler);
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    graph
        .manage_dependencies(
            &mut compiler,
            &DependencyOptions::prune_for_entry_points([ModuleIdentifier::for_file("strong1")]),
        )
        .unwrap();
    assert_files(
        &graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(),
        &[
            weak1.clone(),
            weak2.clone(),
            weak3.clone(),
            strong_from_weak.clone(),
        ],
    );
    assert_files(&chunks[0], std::slice::from_ref(&strong1));
}
