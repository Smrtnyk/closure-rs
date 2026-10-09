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

#![allow(clippy::mutable_key_type)]
use closure_jscomp::{
    js_chunk::{JSChunk, WEAK_CHUNK_NAME},
    js_chunk_graph::{BitSet, JSChunkGraph},
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::static_source_file::SourceKind;
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
// port: JSChunkGraphTest#assertSmallestCoveringSubtree
fn assert_smallest(
    graph: &JSChunkGraph,
    expected: &JSChunk,
    parent: &JSChunk,
    chunks: &[&JSChunk],
) {
    let mut bits = BitSet::new();
    for c in chunks {
        bits.set(c.get_index() as usize);
    }
    assert_eq!(
        graph.get_smallest_covering_subtree(parent, &bits),
        *expected
    );
}
// port: JSChunkGraphTest#assertDeepestCommonDepOneWay
fn assert_deepest(
    graph: &JSChunkGraph,
    expected: Option<&JSChunk>,
    m1: &JSChunk,
    m2: &JSChunk,
    inclusive: bool,
) {
    for (a, b) in [(m1, m2), (m2, m1)] {
        let actual = if inclusive {
            graph.get_deepest_common_dependency_inclusive(a, b)
        } else {
            graph.get_deepest_common_dependency(a, b)
        };
        assert_eq!(actual.as_ref(), expected);
    }
}
fn panic_message(f: impl FnOnce()) -> String {
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_err();
    if let Some(s) = error.downcast_ref::<String>() {
        s.clone()
    } else {
        error.downcast_ref::<&str>().unwrap().to_string()
    }
}

// port: JSChunkGraphTest#testDeepestCommonDep
#[test]
fn test_deepest_common_dep() {
    let [a, b, c, d, e, f] = make_deps();
    let graph = JSChunkGraph::new(vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f.clone(),
    ])
    .unwrap();
    assert_deepest(&graph, None, &a, &a, false);
    assert_deepest(&graph, None, &a, &b, false);
    assert_deepest(&graph, None, &a, &c, false);
    assert_deepest(&graph, None, &a, &d, false);
    assert_deepest(&graph, None, &a, &e, false);
    assert_deepest(&graph, None, &a, &f, false);
    assert_deepest(&graph, Some(&a), &b, &b, false);
    assert_deepest(&graph, Some(&a), &b, &c, false);
    assert_deepest(&graph, Some(&a), &b, &d, false);
    assert_deepest(&graph, Some(&a), &b, &e, false);
    assert_deepest(&graph, Some(&a), &b, &f, false);
    assert_deepest(&graph, Some(&a), &c, &c, false);
    assert_deepest(&graph, Some(&a), &c, &d, false);
    assert_deepest(&graph, Some(&a), &c, &e, false);
    assert_deepest(&graph, Some(&a), &c, &f, false);
    assert_deepest(&graph, Some(&b), &d, &d, false);
    assert_deepest(&graph, Some(&b), &d, &e, false);
    assert_deepest(&graph, Some(&b), &d, &f, false);
    assert_deepest(&graph, Some(&c), &e, &e, false);
    assert_deepest(&graph, Some(&c), &e, &f, false);
    assert_deepest(&graph, Some(&e), &f, &f, false);
}

// port: JSChunkGraphTest#testDeepestCommonDepInclusive
#[test]
fn test_deepest_common_dep_inclusive() {
    let [a, b, c, d, e, f] = make_deps();
    let graph = JSChunkGraph::new(vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f.clone(),
    ])
    .unwrap();
    assert_deepest(&graph, Some(&a), &a, &a, true);
    assert_deepest(&graph, Some(&a), &a, &b, true);
    assert_deepest(&graph, Some(&a), &a, &c, true);
    assert_deepest(&graph, Some(&a), &a, &d, true);
    assert_deepest(&graph, Some(&a), &a, &e, true);
    assert_deepest(&graph, Some(&a), &a, &f, true);
    assert_deepest(&graph, Some(&b), &b, &b, true);
    assert_deepest(&graph, Some(&a), &b, &c, true);
    assert_deepest(&graph, Some(&b), &b, &d, true);
    assert_deepest(&graph, Some(&b), &b, &e, true);
    assert_deepest(&graph, Some(&b), &b, &f, true);
    assert_deepest(&graph, Some(&c), &c, &c, true);
    assert_deepest(&graph, Some(&a), &c, &d, true);
    assert_deepest(&graph, Some(&c), &c, &e, true);
    assert_deepest(&graph, Some(&c), &c, &f, true);
    assert_deepest(&graph, Some(&d), &d, &d, true);
    assert_deepest(&graph, Some(&b), &d, &e, true);
    assert_deepest(&graph, Some(&b), &d, &f, true);
    assert_deepest(&graph, Some(&e), &e, &e, true);
    assert_deepest(&graph, Some(&e), &e, &f, true);
    assert_deepest(&graph, Some(&f), &f, &f, true);
}

// port: JSChunkGraphTest#testSmallestCoveringSubtree
#[test]
fn test_smallest_covering_subtree() {
    let [a, b, c, d, e, f] = make_deps();
    let graph = JSChunkGraph::new(vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f.clone(),
    ])
    .unwrap();
    assert_smallest(&graph, &a, &a, &[&a, &a]);
    assert_smallest(&graph, &a, &a, &[&a, &b]);
    assert_smallest(&graph, &a, &a, &[&a, &c]);
    assert_smallest(&graph, &a, &a, &[&a, &d]);
    assert_smallest(&graph, &a, &a, &[&a, &e]);
    assert_smallest(&graph, &a, &a, &[&a, &f]);
    assert_smallest(&graph, &b, &a, &[&b, &b]);
    assert_smallest(&graph, &a, &a, &[&b, &c]);
    assert_smallest(&graph, &b, &a, &[&b, &d]);
    assert_smallest(&graph, &b, &a, &[&b, &e]);
    assert_smallest(&graph, &b, &a, &[&b, &f]);
    assert_smallest(&graph, &c, &a, &[&c, &c]);
    assert_smallest(&graph, &a, &a, &[&c, &d]);
    assert_smallest(&graph, &c, &a, &[&c, &e]);
    assert_smallest(&graph, &c, &a, &[&c, &f]);
    assert_smallest(&graph, &d, &a, &[&d, &d]);
    assert_smallest(&graph, &b, &a, &[&d, &e]);
    assert_smallest(&graph, &b, &a, &[&d, &f]);
    assert_smallest(&graph, &e, &a, &[&e, &e]);
    assert_smallest(&graph, &e, &a, &[&e, &f]);
    assert_smallest(&graph, &f, &a, &[&f, &f]);
}

// port: JSChunkGraphTest#testGetTransitiveDepsDeepestFirst
#[test]
fn test_get_transitive_deps_deepest_first() {
    let [a, b, c, d, e, f] = make_deps();
    let graph = JSChunkGraph::new(vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f.clone(),
    ])
    .unwrap();
    assert_eq!(graph.get_transitive_deps_deepest_first(&a), vec![]);
    assert_eq!(graph.get_transitive_deps_deepest_first(&b), vec![a.clone()]);
    assert_eq!(graph.get_transitive_deps_deepest_first(&c), vec![a.clone()]);
    assert_eq!(
        graph.get_transitive_deps_deepest_first(&d),
        vec![b.clone(), a.clone()]
    );
    assert_eq!(
        graph.get_transitive_deps_deepest_first(&e),
        vec![c.clone(), b.clone(), a.clone()]
    );
    assert_eq!(
        graph.get_transitive_deps_deepest_first(&f),
        vec![e.clone(), c.clone(), b.clone(), a.clone()]
    );
}

// port: JSChunkTest#testDependencies
#[test]
fn test_dependencies() {
    let a = JSChunk::new("chunk1");
    let b = JSChunk::new("chunk2");
    let c = JSChunk::new("chunk3");
    let d = JSChunk::new("chunk4");
    let e = JSChunk::new("chunk5");
    b.add_dependency(&a);
    c.add_dependency(&a);
    d.add_dependency(&b);
    d.add_dependency(&c);
    e.add_dependency(&a);
    for (chunk, expected) in [
        (&a, vec![]),
        (&b, vec![a.clone()]),
        (&c, vec![a.clone()]),
        (&d, vec![a.clone(), b.clone(), c.clone()]),
    ] {
        assert_eq!(
            chunk.get_all_dependencies(),
            expected.iter().cloned().collect::<IndexSet<_>>()
        );
        let mut inclusive: IndexSet<_> = expected.into_iter().collect();
        inclusive.insert(chunk.clone());
        assert_eq!(chunk.get_this_and_all_dependencies(), inclusive);
    }
}
// port: JSChunkGraphTest#testMakesWeakChunkIfNotPassed
#[test]
fn test_makes_weak_chunk_if_not_passed() {
    let chunks = make_deps();
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    assert_eq!(graph.get_chunk_count(), 7);
    assert!(graph.get_chunks_by_name().contains_key(WEAK_CHUNK_NAME));
    assert_eq!(
        graph
            .get_chunk_by_name(WEAK_CHUNK_NAME)
            .unwrap()
            .get_all_dependencies(),
        chunks.into_iter().collect::<IndexSet<_>>()
    );
}
// port: JSChunkGraphTest#testAcceptExistingWeakChunk
#[test]
fn test_accept_existing_weak_chunk() {
    let mut chunks = make_deps().to_vec();
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    for c in &chunks {
        weak.add_dependency(c);
    }
    weak.add_source_file(SourceFile::from_code_with_kind(
        "weak",
        "",
        SourceKind::WEAK,
    ));
    chunks.push(weak.clone());
    let graph = JSChunkGraph::new(chunks).unwrap();
    assert_eq!(graph.get_chunk_count(), 7);
    assert_eq!(graph.get_chunk_by_name(WEAK_CHUNK_NAME), Some(weak));
}
// port: JSChunkGraphTest#testExistingWeakChunkMustHaveDependenciesOnAllOtherChunks
#[test]
fn test_existing_weak_chunk_must_have_dependencies_on_all_other_chunks() {
    let mut chunks = make_deps().to_vec();
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    for c in &chunks[..5] {
        weak.add_dependency(c);
    }
    chunks.push(weak);
    assert_eq!(
        panic_message(|| {
            let _ = JSChunkGraph::new(chunks);
        }),
        "A weak chunk already exists but it does not depend on every other chunk."
    );
}
// port: JSChunkGraphTest#testWeakFileCannotExistOutsideWeakChunk
#[test]
fn test_weak_file_cannot_exist_outside_weak_chunk() {
    let mut chunks = make_deps().to_vec();
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    for c in &chunks {
        weak.add_dependency(c);
    }
    chunks[0].add_source_file(SourceFile::from_code_with_kind("a", "", SourceKind::WEAK));
    chunks.push(weak);
    assert!(
        panic_message(|| {
            let _ = JSChunkGraph::new(chunks);
        })
        .contains("Found these weak sources in other chunks:\n  a (in chunk chunkA)")
    );
}
// port: JSChunkGraphTest#testStrongFileCannotExistInWeakChunk
#[test]
fn test_strong_file_cannot_exist_in_weak_chunk() {
    let mut chunks = make_deps().to_vec();
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    for c in &chunks {
        weak.add_dependency(c);
    }
    weak.add_source_file(SourceFile::from_code_with_kind("a", "", SourceKind::STRONG));
    chunks.push(weak);
    assert!(
        panic_message(|| {
            let _ = JSChunkGraph::new(chunks);
        })
        .contains("Found these strong sources in the weak chunk:\n  a")
    );
}
// port: JSChunkGraphTest#testChunkDepth
#[test]
fn test_chunk_depth() {
    let chunks = make_deps();
    let _graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    for (c, depth) in chunks.iter().zip([0, 1, 1, 2, 2, 3]) {
        assert_eq!(c.get_depth(), depth);
    }
}
// port: JSChunkGraphTest#testSmallerTreeBeatsDeeperTree
#[test]
fn test_smaller_tree_beats_deeper_tree() {
    let [a, b, c, d, e, f, g, h] = ["a", "b", "c", "d", "e", "f", "g", "h"].map(JSChunk::new);
    b.add_dependency(&a);
    c.add_dependency(&a);
    e.add_dependency(&b);
    f.add_dependency(&b);
    g.add_dependency(&b);
    d.add_dependency(&c);
    e.add_dependency(&d);
    f.add_dependency(&d);
    g.add_dependency(&d);
    h.add_dependency(&d);
    let graph = JSChunkGraph::new(vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f.clone(),
        g.clone(),
        h,
    ])
    .unwrap();
    assert_smallest(&graph, &b, &a, &[&e, &f, &g]);
    assert_smallest(&graph, &d, &c, &[&e, &f, &g]);
}
// port: JSChunkGraphTest#testToJson
#[test]
fn test_to_json() {
    let graph = JSChunkGraph::new(make_deps().to_vec()).unwrap();
    let json = graph.to_json();
    let chunks = json.as_array().unwrap();
    assert_eq!(chunks.len(), 7);
    for c in chunks {
        for field in ["name", "dependencies", "transitive-dependencies", "inputs"] {
            assert!(!c[field].is_null());
        }
    }
    let m = &chunks[3];
    assert_eq!(m["name"], "chunkD");
    assert_eq!(m["dependencies"].to_string(), "[\"chunkB\"]");
    assert_eq!(m["transitive-dependencies"].as_array().unwrap().len(), 2);
    assert_eq!(m["inputs"].to_string(), "[]");
}
// port: JSChunkGraphTest#testMoveMarkedWeakSources
#[test]
fn test_move_marked_weak_sources() {
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
    for source in [&weak1, &strong1, &weak2, &strong2] {
        chunks[0].add_source_file(source.clone());
    }
    let graph = JSChunkGraph::new(chunks.to_vec()).unwrap();
    for (chunk, expected) in [
        (
            graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(),
            vec![weak1, weak2],
        ),
        (chunks[0].clone(), vec![strong1, strong2]),
    ] {
        let inputs = chunk.get_inputs();
        assert_eq!(inputs.len(), expected.len());
        for (i, s) in inputs.iter().zip(expected) {
            assert!(Arc::ptr_eq(&i.get_source_file_arc(), &s));
        }
    }
}
