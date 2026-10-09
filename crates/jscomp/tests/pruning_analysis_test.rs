/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/PruningAnalysisTest.java.

//! Port of PruningAnalysisTest.java.
use closure_jscomp::{
    compiler::Compiler, compiler_input::CompilerInput, compiler_options::CompilerOptions,
    deps::sorted_dependencies::SortedDependencies, pruning_analysis::PruningAnalysis,
    pruning_analysis::Result, source_file::SourceFile,
};

// port: PruningAnalysisTest#setUp
fn set_up() -> Compiler {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    compiler
}

// port: PruningAnalysisTest#createInput
fn create_input(compiler: &Compiler, name: &str, code: &str) -> CompilerInput {
    let input = CompilerInput::new(SourceFile::from_code(name, code));
    input.set_compiler(compiler);
    input
}

// port: PruningAnalysisTest#analyze
fn analyze(
    compiler: &mut Compiler,
    entry_points: &[&CompilerInput],
    all_inputs: &[&CompilerInput],
) -> Result {
    let input_list: Vec<_> = all_inputs
        .iter()
        .map(|input| input.dependency_snapshot(compiler))
        .collect();
    let sorter = SortedDependencies::new(input_list.clone());
    let entry_points: Vec<_> = entry_points
        .iter()
        .map(|entry| {
            input_list
                .iter()
                .find(|view| view.input == **entry)
                .unwrap()
                .clone()
        })
        .collect();
    PruningAnalysis::create(&sorter, entry_points).analyze()
}

fn entry(result: &closure_jscomp::pruning_analysis::ImmutableMap, key: &str) -> Option<i32> {
    result.0.get(key).copied()
}

#[test]
// port: PruningAnalysisTest#testNoBloat
fn test_no_bloat() {
    let mut compiler = set_up();
    let root1 = create_input(
        &compiler,
        "root1.js",
        "goog.provide('root1'); goog.require('dep1');",
    );
    let dep1 = create_input(&compiler, "dep1.js", "goog.provide('dep1');");
    let root2 = create_input(
        &compiler,
        "root2.js",
        "goog.provide('root2'); goog.require('dep2');",
    );
    let dep2 = create_input(&compiler, "dep2.js", "goog.provide('dep2');");

    let result = analyze(
        &mut compiler,
        &[&root1, &root2],
        &[&root1, &dep1, &root2, &dep2],
    );

    // containsExactly("root1.js", 2, "root2.js", 2)
    assert_eq!(result.entry_point_dependency_count().0.len(), 2);
    assert_eq!(
        entry(result.entry_point_dependency_count(), "root1.js"),
        Some(2)
    );
    assert_eq!(
        entry(result.entry_point_dependency_count(), "root2.js"),
        Some(2)
    );
}

#[test]
// port: PruningAnalysisTest#testSharedDependencies
fn test_shared_dependencies() {
    let mut compiler = set_up();
    let root1 = create_input(
        &compiler,
        "root1.js",
        "goog.provide('root1'); goog.require('s1'); goog.require('s2');",
    );
    let root2 = create_input(
        &compiler,
        "root2.js",
        "goog.provide('root2'); goog.require('s1'); goog.require('s3');",
    );
    let root3 = create_input(
        &compiler,
        "root3.js",
        "goog.provide('root3'); goog.require('s1'); goog.require('s2'); goog.require('s3');",
    );
    let s1 = create_input(&compiler, "s1.js", "goog.provide('s1');");
    let s2 = create_input(&compiler, "s2.js", "goog.provide('s2');");
    let s3 = create_input(&compiler, "s3.js", "goog.provide('s3');");

    let result = analyze(
        &mut compiler,
        &[&root1, &root2, &root3],
        &[&root1, &root2, &root3, &s1, &s2, &s3],
    );

    let counts = result.entry_point_dependency_count();
    assert_eq!(entry(counts, "root1.js"), Some(3));
    assert_eq!(entry(counts, "root2.js"), Some(3));
    assert_eq!(entry(counts, "root3.js"), Some(4));
}

#[test]
// port: PruningAnalysisTest#testWeakDependencies_included
fn test_weak_dependencies_included() {
    let mut compiler = set_up();
    let root1 = create_input(
        &compiler,
        "root1.js",
        "goog.provide('root1'); goog.requireType('weak1');",
    );
    let weak1 = create_input(&compiler, "weak1.js", "goog.provide('weak1');");

    let result = analyze(&mut compiler, &[&root1], &[&root1, &weak1]);

    assert_eq!(
        entry(result.entry_point_dependency_count(), "root1.js"),
        Some(2)
    );
}

#[test]
// port: PruningAnalysisTest#testForwardDeclare_notIncluded
fn test_forward_declare_not_included() {
    let mut compiler = set_up();
    // goog.forwardDeclare is NOT captured by JsFileRegexParser as a dependency.
    let root1 = create_input(
        &compiler,
        "root1.js",
        "goog.provide('root1'); goog.forwardDeclare('missing');",
    );
    let fwd_declared = create_input(&compiler, "fwdDeclared.js", "goog.provide('fwdDeclared');");

    let result = analyze(&mut compiler, &[&root1], &[&root1, &fwd_declared]);

    assert_eq!(
        entry(result.entry_point_dependency_count(), "root1.js"),
        Some(1)
    );
}

#[test]
// port: PruningAnalysisTest#testOverlappingEntryPoints
fn test_overlapping_entry_points() {
    let mut compiler = set_up();
    let entry1 = create_input(&compiler, "entry1.js", "goog.require('dep1');");
    let entry2 = create_input(&compiler, "entry2.js", "goog.require('dep2');");
    let dep1 = create_input(
        &compiler,
        "dep1.js",
        "goog.provide('dep1'); goog.require('dep2'); goog.require('excl1');",
    );
    let dep2 = create_input(
        &compiler,
        "dep2.js",
        "goog.provide('dep2'); goog.require('excl2');",
    );
    let excl1 = create_input(&compiler, "excl1.js", "goog.provide('excl1');");
    let excl2 = create_input(&compiler, "excl2.js", "goog.provide('excl2');");

    let result = analyze(
        &mut compiler,
        &[&entry1, &entry2],
        &[&entry1, &entry2, &dep1, &dep2, &excl1, &excl2],
    );

    // Symbols are entry1.js and entry2.js.
    // entry1.js closure: {entry1.js, dep1.js, dep2.js excl1.js, excl2.js}
    // entry2.js closure: {entry2.js, dep2.js, excl2.js}
    let counts = result.entry_point_dependency_count();
    assert_eq!(entry(counts, "entry1.js"), Some(5));
    assert_eq!(entry(counts, "entry2.js"), Some(3));
}

#[test]
// port: PruningAnalysisTest#testMultipleEntryPoints
fn test_multiple_entry_points() {
    let mut compiler = set_up();
    let root1 = create_input(&compiler, "root1.js", "goog.require('dep1');");
    let root2 = create_input(&compiler, "root2.js", "goog.require('dep2');");
    let dep1 = create_input(&compiler, "dep1.js", "goog.provide('dep1');");
    let dep2 = create_input(&compiler, "dep2.js", "goog.provide('dep2');");

    let result = analyze(
        &mut compiler,
        &[&root1, &root2],
        &[&root1, &root2, &dep1, &dep2],
    );

    let counts = result.entry_point_dependency_count();
    assert_eq!(entry(counts, "root1.js"), Some(2));
    assert_eq!(entry(counts, "root2.js"), Some(2));
}

#[test]
// port: PruningAnalysisTest#testWeakCycle
fn test_weak_cycle() {
    let mut compiler = set_up();
    let dep1 = create_input(
        &compiler,
        "dep1.js",
        "goog.provide('dep1'); goog.require('dep2');",
    );
    let dep2 = create_input(
        &compiler,
        "dep2.js",
        "goog.provide('dep2'); goog.requireType('root1');",
    );
    let root = create_input(&compiler, "root.js", "goog.require('dep1');");

    let result = analyze(&mut compiler, &[&root], &[&root, &dep1, &dep2]);

    assert_eq!(
        entry(result.entry_point_dependency_count(), "root.js"),
        Some(3)
    );
}

#[test]
// port: PruningAnalysisTest#testBottlenecks
fn test_bottlenecks() {
    let mut compiler = set_up();
    // entry -> A -> B -> {C, D, E}
    let a = create_input(&compiler, "a.js", "goog.provide('a'); goog.require('b');");
    let b = create_input(
        &compiler,
        "b.js",
        "goog.provide('b'); goog.require('c'); goog.require('d'); goog.require('e');",
    );
    let c = create_input(&compiler, "c.js", "goog.provide('c');");
    let d = create_input(&compiler, "d.js", "goog.provide('d');");
    let e = create_input(&compiler, "e.js", "goog.provide('e');");
    let entry_input = create_input(&compiler, "entry.js", "goog.require('a');");

    let result = analyze(
        &mut compiler,
        &[&entry_input],
        &[&a, &b, &c, &d, &e, &entry_input],
    );

    // Bottleneck scores (dominated subtree size):
    // a.js: dominates {a, b, c, d, e}. Size = 5.
    // b.js: dominates {b, c, d, e}. Size = 4.
    // c.js, d.js, e.js: Size = 1.
    assert_eq!(entry(result.bottleneck_blame(), "a.js"), Some(5));
    assert_eq!(entry(result.bottleneck_blame(), "b.js"), Some(4));
    assert_ne!(entry(result.bottleneck_blame(), "c.js"), Some(1));
}

#[test]
// port: PruningAnalysisTest#testSharedBottleneckInAnalysis
fn test_shared_bottleneck_in_analysis() {
    let mut compiler = set_up();
    // entry1 -> B, entry2 -> B, B -> {C, D}
    let b = create_input(
        &compiler,
        "b.js",
        "goog.provide('b'); goog.require('c'); goog.require('d');",
    );
    let c = create_input(&compiler, "c.js", "goog.provide('c');");
    let d = create_input(&compiler, "d.js", "goog.provide('d');");
    let entry1 = create_input(&compiler, "entry1.js", "goog.require('b');");
    let entry2 = create_input(&compiler, "entry2.js", "goog.require('b');");

    let result = analyze(
        &mut compiler,
        &[&entry1, &entry2],
        &[&b, &c, &d, &entry1, &entry2],
    );

    // b.js dominates {b.js, c.js, d.js}. Size = 3.
    assert_eq!(entry(result.bottleneck_blame(), "b.js"), Some(3));
}
