/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/deps/SortedDependenciesTest.java.

use super::*;
use crate::deps::{dependency_info::Require, simple_dependency_info::SimpleDependencyInfo};
// port: SortedDependenciesTest#testSort
#[test]
fn test_sort() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_requires([
            Require::goog_require_symbol("b"),
            Require::goog_require_symbol("c"),
        ])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_requires([Require::goog_require_symbol("d")])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .set_requires([Require::goog_require_symbol("d")])
        .build();
    let d = SimpleDependencyInfo::builder("d", "d")
        .set_provides(["d"])
        .build();

    assert_sorted_inputs(
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
        vec![a.clone(), b.clone(), c.clone(), d.clone()],
    );
    assert_sorted_inputs(
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
    );
    assert_sorted_inputs(
        vec![d.clone(), c.clone(), b.clone(), a.clone()],
        vec![d.clone(), c.clone(), b.clone(), a.clone()],
    );
    assert_sorted_inputs(
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
        vec![d.clone(), a.clone(), b.clone(), c.clone()],
    );

    assert_sorted_deps(
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
        vec![a.clone()],
    );
    assert_sorted_deps(
        vec![d.clone(), c.clone()],
        vec![d.clone(), c.clone(), b.clone(), a.clone()],
        vec![c.clone()],
    );
    assert_sorted_deps(
        vec![d.clone()],
        vec![d.clone(), c.clone(), b.clone(), a.clone()],
        vec![d.clone()],
    );
}
// port: SortedDependenciesTest#testSort2
#[test]
fn test_sort2() {
    let ab = SimpleDependencyInfo::builder("ab", "ab")
        .set_provides(["a", "b"])
        .set_requires([
            Require::goog_require_symbol("d"),
            Require::goog_require_symbol("f"),
        ])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .set_requires([Require::goog_require_symbol("h")])
        .build();
    let d = SimpleDependencyInfo::builder("d", "d")
        .set_provides(["d"])
        .set_requires([
            Require::goog_require_symbol("e"),
            Require::goog_require_symbol("f"),
        ])
        .build();
    let ef = SimpleDependencyInfo::builder("ef", "ef")
        .set_provides(["e", "f"])
        .set_requires([
            Require::goog_require_symbol("g"),
            Require::goog_require_symbol("c"),
        ])
        .build();
    let g = SimpleDependencyInfo::builder("g", "g")
        .set_provides(["g"])
        .build();
    let hi = SimpleDependencyInfo::builder("hi", "hi")
        .set_provides(["h", "i"])
        .build();

    assert_sorted_inputs(
        vec![
            g.clone(),
            hi.clone(),
            c.clone(),
            ef.clone(),
            d.clone(),
            ab.clone(),
        ],
        vec![
            ab.clone(),
            c.clone(),
            d.clone(),
            ef.clone(),
            g.clone(),
            hi.clone(),
        ],
    );

    assert_sorted_deps(
        vec![g.clone()],
        vec![
            ab.clone(),
            c.clone(),
            d.clone(),
            ef.clone(),
            g.clone(),
            hi.clone(),
        ],
        vec![g.clone()],
    );
    assert_sorted_deps(
        vec![g.clone(), hi.clone(), c.clone(), ef.clone(), d.clone()],
        vec![
            ab.clone(),
            c.clone(),
            d.clone(),
            ef.clone(),
            g.clone(),
            hi.clone(),
        ],
        vec![d.clone(), hi.clone()],
    );
}
// port: SortedDependenciesTest#testSort3
#[test]
fn test_sort3() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_requires([Require::goog_require_symbol("c")])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_requires([Require::goog_require_symbol("a")])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .set_requires([Require::goog_require_symbol("b")])
        .build();

    assert_order(
        vec![a.clone(), b.clone(), c.clone()],
        vec![b.clone(), c.clone(), a.clone()],
    );
}
// port: SortedDependenciesTest#testSort4
#[test]
fn test_sort4() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_requires([Require::goog_require_symbol("a")])
        .build();
    assert_sorted_deps(vec![a.clone()], vec![a.clone()], vec![a.clone()]);
}
// port: SortedDependenciesTest#testSort5
#[test]
fn test_sort5() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .build();

    assert_sorted_inputs(
        vec![a.clone(), b.clone(), c.clone()],
        vec![a.clone(), b.clone(), c.clone()],
    );
    assert_sorted_inputs(
        vec![c.clone(), b.clone(), a.clone()],
        vec![c.clone(), b.clone(), a.clone()],
    );
}
// port: SortedDependenciesTest#testSort6
#[test]
fn test_sort6() {
    let a = SimpleDependencyInfo::builder("gin", "gin")
        .set_provides(["gin"])
        .set_requires([Require::goog_require_symbol("tonic")])
        .build();
    let b = SimpleDependencyInfo::builder("tonic", "tonic")
        .set_provides(["tonic"])
        .set_requires([Require::goog_require_symbol("gin2")])
        .build();
    let c = SimpleDependencyInfo::builder("gin2", "gin2")
        .set_provides(["gin2"])
        .set_requires([Require::goog_require_symbol("gin")])
        .build();
    let d = SimpleDependencyInfo::builder("gin3", "gin3")
        .set_provides(["gin3"])
        .set_requires([Require::goog_require_symbol("gin")])
        .build();

    assert_order(
        vec![a.clone(), b.clone(), c.clone(), d.clone()],
        vec![c.clone(), b.clone(), a.clone(), d.clone()],
    );
}
// port: SortedDependenciesTest#testSort7
#[test]
fn test_sort7() {
    let a = SimpleDependencyInfo::builder("gin", "gin")
        .set_provides(["gin"])
        .set_requires([Require::goog_require_symbol("tonic")])
        .build();
    let b = SimpleDependencyInfo::builder("tonic", "tonic")
        .set_provides(["tonic"])
        .set_requires([Require::goog_require_symbol("gin")])
        .build();
    let c = SimpleDependencyInfo::builder("gin2", "gin2")
        .set_provides(["gin2"])
        .set_requires([Require::goog_require_symbol("gin")])
        .build();
    let d = SimpleDependencyInfo::builder("gin3", "gin3")
        .set_provides(["gin3"])
        .set_requires([Require::goog_require_symbol("gin")])
        .build();

    assert_order(
        vec![a.clone(), b.clone(), c.clone(), d.clone()],
        vec![b.clone(), a.clone(), c.clone(), d.clone()],
    );
}
// port: SortedDependenciesTest#testSort8
#[test]
fn test_sort8() {
    let a = SimpleDependencyInfo::builder("A", "A")
        .set_provides(["A"])
        .set_requires([Require::goog_require_symbol("B")])
        .build();
    let b = SimpleDependencyInfo::builder("B", "B")
        .set_provides(["B"])
        .set_requires([Require::goog_require_symbol("C")])
        .build();
    let c = SimpleDependencyInfo::builder("C", "C")
        .set_provides(["C"])
        .set_requires([Require::goog_require_symbol("D")])
        .build();
    let d = SimpleDependencyInfo::builder("D", "D")
        .set_provides(["D"])
        .set_requires([Require::goog_require_symbol("A")])
        .build();

    assert_order(
        vec![a.clone(), b.clone(), c.clone(), d.clone()],
        vec![d.clone(), c.clone(), b.clone(), a.clone()],
    );
}
// port: SortedDependenciesTest#testSort9
#[test]
fn test_sort9() {
    let a = SimpleDependencyInfo::builder("A", "A")
        .set_provides(["A"])
        .set_requires([Require::goog_require_symbol("B")])
        .build();
    let a2 = SimpleDependencyInfo::builder("A", "A")
        .set_provides(["A"])
        .set_requires([Require::goog_require_symbol("B1")])
        .build();
    let b = SimpleDependencyInfo::builder("B", "B")
        .set_provides(["B"])
        .set_requires([Require::goog_require_symbol("C")])
        .build();
    let c = SimpleDependencyInfo::builder("C", "C")
        .set_provides(["C"])
        .set_requires([Require::goog_require_symbol("E")])
        .build();
    let d = SimpleDependencyInfo::builder("D", "D")
        .set_provides(["D"])
        .set_requires([Require::goog_require_symbol("A")])
        .build();
    let e = SimpleDependencyInfo::builder("B1", "B1")
        .set_provides(["B1"])
        .set_requires([Require::goog_require_symbol("C1")])
        .build();
    let f = SimpleDependencyInfo::builder("C1", "C1")
        .set_provides(["C1"])
        .set_requires([Require::goog_require_symbol("D1")])
        .build();
    let g = SimpleDependencyInfo::builder("D1", "D1")
        .set_provides(["D1"])
        .set_requires([Require::goog_require_symbol("A")])
        .build();

    assert_order(
        vec![
            a.clone(),
            a2.clone(),
            b.clone(),
            c.clone(),
            d.clone(),
            e.clone(),
            f.clone(),
            g.clone(),
        ],
        vec![
            c.clone(),
            b.clone(),
            a.clone(),
            g.clone(),
            f.clone(),
            e.clone(),
            a2.clone(),
            d.clone(),
        ],
    );
}
// port: SortedDependenciesTest#testSort10
#[test]
fn test_sort10() {
    let a = SimpleDependencyInfo::builder("A", "A")
        .set_provides(["A"])
        .set_requires([Require::goog_require_symbol("C")])
        .build();
    let b = SimpleDependencyInfo::builder("B", "B")
        .set_provides(["B"])
        .build();
    let c = SimpleDependencyInfo::builder("C", "C")
        .set_provides(["C"])
        .build();

    let sorted = create_sorted_dependencies(vec![a.clone(), b.clone(), c.clone()]);

    assert_eq!(sorted.get_sorted_list(), &[c, a, b]);
}
// port: SortedDependenciesTest#testWeakSort
#[test]
fn test_weak_sort() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_type_requires(["b"])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_type_requires(["c"])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .set_type_requires(["d"])
        .build();
    let d = SimpleDependencyInfo::builder("d", "d")
        .set_provides(["d"])
        .build();

    assert_sorted_weak_deps(
        vec![b.clone(), c.clone(), d.clone()],
        vec![a.clone(), b.clone(), c.clone(), d.clone()],
        vec![a.clone()],
    );

    assert_sorted_weak_deps(
        vec![c.clone(), b.clone(), d.clone()],
        vec![a.clone(), c.clone(), b.clone(), d.clone()],
        vec![a.clone()],
    );

    assert_sorted_weak_deps(
        vec![d.clone(), b.clone(), c.clone()],
        vec![d.clone(), b.clone(), c.clone(), a.clone()],
        vec![a.clone()],
    );

    assert_sorted_weak_deps(
        vec![c.clone(), d.clone(), b.clone()],
        vec![c.clone(), d.clone(), b.clone(), a.clone()],
        vec![a.clone()],
    );
}
// port: SortedDependenciesTest#testWeakSortIncludesStrongEdgesFromWeakSources
#[test]
fn test_weak_sort_includes_strong_edges_from_weak_sources() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_type_requires(["b"])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_requires([Require::goog_require_symbol("c")])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .build();

    assert_sorted_weak_deps(
        vec![c.clone(), b.clone()],
        vec![a.clone(), b.clone(), c.clone()],
        vec![a.clone()],
    );

    assert_sorted_weak_deps(
        vec![c.clone(), b.clone()],
        vec![a.clone(), c.clone(), b.clone()],
        vec![a.clone()],
    );
}
// port: SortedDependenciesTest#testWeakAndStrongIsStrong
#[test]
fn test_weak_and_strong_is_strong() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_requires([Require::goog_require_symbol("c")])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_type_requires(["c"])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .build();

    assert_sorted_weak_deps(
        vec![],
        vec![a.clone(), b.clone(), c.clone()],
        vec![a.clone(), b.clone()],
    );

    assert_sorted_weak_deps(
        vec![],
        vec![a.clone(), b.clone(), c.clone()],
        vec![b.clone(), a.clone()],
    );

    assert_sorted_weak_deps(
        vec![],
        vec![c.clone(), b.clone(), a.clone()],
        vec![a.clone(), b.clone()],
    );

    assert_sorted_weak_deps(
        vec![],
        vec![c.clone(), a.clone(), b.clone()],
        vec![b.clone(), a.clone()],
    );
}
// port: SortedDependenciesTest#testSortCircularWeakStrongReference
#[test]
fn test_sort_circular_weak_strong_reference() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_requires([Require::goog_require_symbol("b")])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_type_requires(["a"])
        .build();

    assert_sorted_inputs(vec![b.clone(), a.clone()], vec![a.clone(), b.clone()]);
    assert_sorted_inputs(vec![b.clone(), a.clone()], vec![b.clone(), a.clone()]);
}
// port: SortedDependenciesTest#testSortCircularWeakReference
#[test]
fn test_sort_circular_weak_reference() {
    let a = SimpleDependencyInfo::builder("a", "a")
        .set_provides(["a"])
        .set_type_requires(["b"])
        .build();
    let b = SimpleDependencyInfo::builder("b", "b")
        .set_provides(["b"])
        .set_type_requires(["c"])
        .build();
    let c = SimpleDependencyInfo::builder("c", "c")
        .set_provides(["c"])
        .set_type_requires(["b"])
        .build();

    assert_sorted_weak_deps(
        vec![b.clone(), c.clone()],
        vec![b.clone(), c.clone(), a.clone()],
        vec![a.clone()],
    );
    assert_sorted_weak_deps(
        vec![c.clone(), b.clone()],
        vec![c.clone(), b.clone(), a.clone()],
        vec![a.clone()],
    );
    assert_sorted_weak_deps(
        vec![b.clone(), c.clone()],
        vec![a.clone(), b.clone(), c.clone()],
        vec![a.clone()],
    );
}

// port: SortedDependenciesTest#createSortedDependencies
fn create_sorted_dependencies(
    shuffled: Vec<SimpleDependencyInfo>,
) -> SortedDependencies<SimpleDependencyInfo> {
    SortedDependencies::new(shuffled)
}
// port: SortedDependenciesTest#assertSortedInputs
fn assert_sorted_inputs(expected: Vec<SimpleDependencyInfo>, shuffled: Vec<SimpleDependencyInfo>) {
    assert_eq!(
        create_sorted_dependencies(shuffled).get_sorted_list(),
        expected
    );
}
// port: SortedDependenciesTest#assertSortedDeps
fn assert_sorted_deps(
    expected: Vec<SimpleDependencyInfo>,
    shuffled: Vec<SimpleDependencyInfo>,
    roots: Vec<SimpleDependencyInfo>,
) {
    assert_eq!(
        create_sorted_dependencies(shuffled).get_sorted_strong_dependencies_of(&roots),
        expected
    );
}
// port: SortedDependenciesTest#assertSortedWeakDeps
fn assert_sorted_weak_deps(
    expected: Vec<SimpleDependencyInfo>,
    shuffled: Vec<SimpleDependencyInfo>,
    roots: Vec<SimpleDependencyInfo>,
) {
    assert_eq!(
        create_sorted_dependencies(shuffled).get_sorted_weak_dependencies_of(&roots),
        expected
    );
}
// port: SortedDependenciesTest#assertOrder
fn assert_order(shuffle: Vec<SimpleDependencyInfo>, expected: Vec<SimpleDependencyInfo>) {
    assert_eq!(
        create_sorted_dependencies(shuffle).get_sorted_list(),
        expected
    );
}
