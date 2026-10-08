/*
 * Copyright 2026 The closure-rs Authors.
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

//! Checks of the ported testing utilities (JSChunkGraphBuilder, JSCompCorrespondences,
//! TestErrorManager). Java has no unit tests of its own for them; these pin the Java behaviour
//! that the corpus tests rely on.
use closure_jscomp::compiler::Compiler;
use closure_jscomp::error_manager::ErrorManager;
use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;
use closure_testing::testing::js_comp_correspondences::equality_when_parsed_as_expression;
use closure_testing::testing::test_error_manager::TestErrorManager;

fn dependency_names(chunks: &[closure_jscomp::js_chunk::JSChunk]) -> Vec<Vec<String>> {
    chunks
        .iter()
        .map(|c| c.get_dependencies().iter().map(|d| d.get_name()).collect())
        .collect()
}

// JSChunkGraphBuilder#build: names m{i}, files i{i}.js, CHAIN/STAR/BUSH/TREE/DISJOINT edges.
#[test]
fn chunk_graph_shapes_names_and_files() {
    let chain = JSChunkGraphBuilder::for_chain()
        .add_chunk("a")
        .add_chunk_with_name("b", "named")
        .add_chunks(["c"])
        .build();
    assert_eq!(
        chain.iter().map(|c| c.get_name()).collect::<Vec<_>>(),
        ["m0", "named", "m2"]
    );
    assert_eq!(
        chain
            .iter()
            .map(|c| c.get_inputs()[0].get_name().to_string())
            .collect::<Vec<_>>(),
        ["i0.js", "i1.js", "i2.js"]
    );
    assert_eq!(
        dependency_names(&chain),
        [vec![], vec!["m0".to_string()], vec!["named".to_string()]]
    );
    let star = JSChunkGraphBuilder::for_star()
        .add_chunks(["a", "b", "c"])
        .build();
    assert_eq!(
        dependency_names(&star),
        [vec![], vec!["m0".to_string()], vec!["m0".to_string()]]
    );
    let bush = JSChunkGraphBuilder::for_bush()
        .add_chunks(["a", "b", "c", "d"])
        .build();
    assert_eq!(
        dependency_names(&bush),
        [
            vec![],
            vec!["m0".to_string()],
            vec!["m1".to_string()],
            vec!["m1".to_string()]
        ]
    );
    let tree = JSChunkGraphBuilder::for_tree()
        .add_chunks(["a", "b", "c", "d"])
        .build();
    assert_eq!(
        dependency_names(&tree),
        [
            vec![],
            vec!["m0".to_string()],
            vec!["m0".to_string()],
            vec!["m1".to_string()]
        ]
    );
    let disjoint = JSChunkGraphBuilder::for_unordered()
        .add_chunks(["a", "b"])
        .set_filename_format("f%s.js")
        .build();
    assert_eq!(dependency_names(&disjoint), [Vec::<String>::new(), vec![]]);
    assert_eq!(disjoint[1].get_inputs()[0].get_name(), "f1.js");
}

#[test]
#[should_panic(expected = "BUSHes need at least three graph nodes")]
fn bush_needs_three_chunks() {
    JSChunkGraphBuilder::for_bush()
        .add_chunks(["a", "b"])
        .build();
}

#[test]
fn equality_when_parsed_as_expression_compares_across_compilers() {
    let mut compiler = Compiler::new();
    let root = compiler.parse_test_code("a + b;");
    let expr = root.get_first_first_child(&compiler).unwrap();
    assert!(equality_when_parsed_as_expression(&compiler, expr, "a + b"));
    assert!(!equality_when_parsed_as_expression(
        &compiler, expr, "a - b"
    ));
}

#[test]
fn test_error_manager_counts_expected_diagnostics() {
    let mut manager = TestErrorManager::new();
    manager.expect_errors(&[]);
    assert!(manager.has_encountered_all_errors());
    assert!(manager.has_encountered_all_warnings());
    assert_eq!(manager.get_error_count(), 0);
}
