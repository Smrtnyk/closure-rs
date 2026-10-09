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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/PruningAnalysis.java.

//! Port of `PruningAnalysis.java`.
use crate::{
    compiler_input::CompilerInputDependencyInfo,
    deps::{dependency_info::DependencyInfo, sorted_dependencies::SortedDependencies},
    graph::{
        dominator_tree::DominatorTree, graph::Graph, linked_directed_graph::LinkedDirectedGraph,
    },
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use std::fmt;

// Limit the size of the "bottleneck" report to the top N files.
const BOTTLENECK_THRESHOLD: usize = 25;

/// Results of the pruning analysis.
///
/// `entry_point_dependency_count`: a map of entry point file name to the number of files that are
/// pulled in by that root, including shared dependencies. `bottleneck_blame`: a map of the top N
/// files that are considered "bottlenecks" in the dependency graph, along with how many files are
/// pulled in by that bottleneck (i.e. how many files are "dominated" by that bottleneck in a
/// dominator tree analysis.)
pub struct Result {
    pub entry_point_dependency_count: ImmutableMap,
    pub bottleneck_blame: ImmutableMap,
}
impl Result {
    // port: PruningAnalysis.Result#entryPointDependencyCount
    pub fn entry_point_dependency_count(&self) -> &ImmutableMap {
        &self.entry_point_dependency_count
    }
    // port: PruningAnalysis.Result#bottleneckBlame
    pub fn bottleneck_blame(&self) -> &ImmutableMap {
        &self.bottleneck_blame
    }
}

/// ImmutableMap<String, Integer>, whose toString is AbstractMap's `{k=v, k=v}`.
pub struct ImmutableMap(pub IndexMap<String, i32>);
impl fmt::Display for ImmutableMap {
    // port: java.util.AbstractMap#toString (ImmutableMap#toString)
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (i, (key, value)) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{key}={value}")?;
        }
        f.write_str("}")
    }
}

/// The Compiler's dependency views stand for the CompilerInputs of
/// `SortedDependencies<CompilerInput>`; their dependency info is the input's.
pub struct PruningAnalysis<'a> {
    sorter: &'a SortedDependencies<CompilerInputDependencyInfo>,
    entry_points: IndexSet<CompilerInputDependencyInfo>,
}

const VIRTUAL_ROOT: &str = "ENTRY POINTS ROOT";

impl<'a> PruningAnalysis<'a> {
    // port: PruningAnalysis#PruningAnalysis
    fn new(
        sorter: &'a SortedDependencies<CompilerInputDependencyInfo>,
        entry_points: impl IntoIterator<Item = CompilerInputDependencyInfo>,
    ) -> Self {
        Self {
            sorter,
            entry_points: entry_points.into_iter().collect(),
        }
    }

    // port: PruningAnalysis#create
    pub fn create(
        sorter: &'a SortedDependencies<CompilerInputDependencyInfo>,
        entry_points: impl IntoIterator<Item = CompilerInputDependencyInfo>,
    ) -> Self {
        Self::new(sorter, entry_points)
    }

    /// Analyzes the dependency graph to identify possible areas of code reduction by pruning
    /// dependencies.
    // port: PruningAnalysis#analyze
    pub fn analyze(&self) -> Result {
        let total = self.calculate_total_blame();
        let bottlenecks = self.calculate_bottlenecks();

        Result {
            entry_point_dependency_count: total,
            bottleneck_blame: bottlenecks,
        }
    }

    // port: PruningAnalysis#calculateTotalBlame
    fn calculate_total_blame(&self) -> ImmutableMap {
        let mut reachability_per_entry_point: IndexMap<String, i32> = IndexMap::<_, _>::default();
        for provider in &self.entry_points {
            let mut transitive_closure: IndexSet<CompilerInputDependencyInfo> =
                IndexSet::<_>::default();
            transitive_closure.extend(self.sorter.get_strong_dependencies_of(
                std::slice::from_ref(provider),
                /* sorted= */ false,
            ));
            transitive_closure.extend(
                self.sorter
                    .get_sorted_weak_dependencies_of(std::slice::from_ref(provider)),
            );
            // ImmutableMap.Builder#buildOrThrow rejects duplicate keys.
            let name = provider.get_name().to_owned();
            assert!(
                !reachability_per_entry_point.contains_key(&name),
                "java.lang.IllegalArgumentException: Multiple entries with same key: {name}"
            );
            reachability_per_entry_point.insert(name, transitive_closure.len() as i32);
        }
        ImmutableMap(reachability_per_entry_point)
    }

    /// Calculates the top N files that are considered "bottlenecks" in the dependency graph,
    /// along with how many files are pulled in by that bottleneck.
    // port: PruningAnalysis#calculateBottlenecks
    fn calculate_bottlenecks(&self) -> ImmutableMap {
        let graph = self.build_graph();

        let dominator_tree = DominatorTree::compute(&graph, VIRTUAL_ROOT.to_owned());

        let mut bottleneck_blame: IndexMap<String, i32> = IndexMap::<_, _>::default();
        for (node, &retained_count) in dominator_tree.get_all_subtree_sizes() {
            if node != VIRTUAL_ROOT && retained_count > 1 {
                bottleneck_blame.insert(node.clone(), retained_count);
            }
        }

        // sorted(comparingInt(Entry::getValue).reversed()) is a stable sort.
        let mut entries: Vec<(String, i32)> = bottleneck_blame.into_iter().collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        ImmutableMap(entries.into_iter().take(BOTTLENECK_THRESHOLD).collect())
    }

    // port: PruningAnalysis#buildGraph
    fn build_graph(&self) -> LinkedDirectedGraph<String, Option<String>> {
        let mut graph = LinkedDirectedGraph::create();
        graph.create_node(VIRTUAL_ROOT.to_owned());

        for input in self.sorter.get_sorted_list() {
            graph.create_node(input.get_name().to_owned());
        }

        for input in &self.entry_points {
            graph.connect(VIRTUAL_ROOT.to_owned(), None, input.get_name().to_owned());
        }

        for input in self.sorter.get_sorted_list() {
            let name = input.get_name();
            let mut dep_symbols: IndexSet<String> = IndexSet::<_>::default();
            for req in input.get_requires() {
                dep_symbols.insert(req.get_symbol().to_owned());
            }
            dep_symbols.extend(input.get_type_requires().iter().cloned());

            for symbol in &dep_symbols {
                match self.sorter.get_input_providing(symbol) {
                    Ok(dep) => {
                        if dep != *input {
                            graph.connect_if_not_found(
                                name.to_owned(),
                                None,
                                dep.get_name().to_owned(),
                            );
                        }
                    }
                    Err(_e) => {
                        // Skip missing symbols.
                    }
                }
            }
        }
        graph
    }
}
