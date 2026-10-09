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
//   src/com/google/javascript/jscomp/JSChunkGraph.java.

#![allow(clippy::needless_late_init)]
// Preserve Java statement order and borrowing branches.
// Java LinkedIdentityHashMap/LinkedHashSet keys compare object identity.
#![allow(clippy::mutable_key_type)]
use crate::{
    compiler_input::CompilerInput,
    diagnostic_type::DiagnosticType,
    js_chunk::{JSChunk, WEAK_CHUNK_NAME},
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::static_source_file::StaticSourceFile;
use std::{cmp::Ordering, fmt};

pub static WEAK_FILE_REACHABLE_FROM_ENTRY_POINT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_WEAK_FILE_REACHABLE_FROM_ENTRY_POINT_ERROR",
    "File strongly reachable from an entry point must not be weak: {0}",
);
pub static EXPLICIT_WEAK_ENTRY_POINT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_EXPLICIT_WEAK_ENTRY_POINT_ERROR",
    "Explicit entry point input must not be weak: {0}",
);
pub static IMPLICIT_WEAK_ENTRY_POINT_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_IMPLICIT_WEAK_ENTRY_POINT_ERROR",
    "Implicit entry point input should not be weak: {0}",
);

/// Adapter for the Java BitSet operations used by the graph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitSet {
    words: Vec<u64>,
}
impl BitSet {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, index: usize) {
        self.words.resize(self.words.len().max(index / 64 + 1), 0);
        self.words[index / 64] |= 1u64 << (index % 64);
    }
    pub fn get(&self, index: usize) -> bool {
        self.words
            .get(index / 64)
            .is_some_and(|w| w & (1u64 << (index % 64)) != 0)
    }
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }
    pub fn or(&mut self, other: &Self) {
        self.words
            .resize(self.words.len().max(other.words.len()), 0);
        for (i, w) in other.words.iter().enumerate() {
            self.words[i] |= w;
        }
    }
    pub fn and(&mut self, other: &Self) {
        for (i, w) in self.words.iter_mut().enumerate() {
            *w &= other.words.get(i).copied().unwrap_or(0);
        }
    }
    pub fn and_not(&mut self, other: &Self) {
        for (i, w) in self.words.iter_mut().enumerate() {
            *w &= !other.words.get(i).copied().unwrap_or(0);
        }
    }
    pub fn next_set_bit(&self, start: usize) -> Option<usize> {
        (start..self.words.len() * 64).find(|i| self.get(*i))
    }
    pub fn previous_set_bit(&self, start: i32) -> Option<usize> {
        if start < 0 {
            None
        } else {
            (0..=start as usize).rev().find(|i| self.get(*i))
        }
    }
}
impl fmt::Display for BitSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indices: Vec<_> = (0..self.words.len() * 64)
            .filter(|i| self.get(*i))
            .map(|i| i.to_string())
            .collect();
        write!(f, "{{{}}}", indices.join(", "))
    }
}

pub struct JSChunkGraph {
    chunks: Vec<JSChunk>,
    self_plus_transitive_deps: Vec<BitSet>,
    subtree_size: Vec<i32>,
    chunks_by_depth: Vec<Vec<JSChunk>>,
    dependency_map: std::sync::Mutex<IndexMap<JSChunk, IndexSet<JSChunk>>>,
}
/// Typed borrowing view for java.lang.reflect.Field replay; the instance fields remain private.
pub struct JSChunkGraphReplayFields<'a> {
    pub chunks: &'a [JSChunk],
    pub self_plus_transitive_deps: &'a [BitSet],
    pub subtree_size: &'a [i32],
    pub chunks_by_depth: &'a [Vec<JSChunk>],
}

impl JSChunkGraph {
    // port: java.lang.reflect.Field#get (native replay access; dependencyMap is only dumped as a
    // reference)
    pub fn replay_fields(&self) -> JSChunkGraphReplayFields<'_> {
        JSChunkGraphReplayFields {
            chunks: &self.chunks,
            self_plus_transitive_deps: &self.self_plus_transitive_deps,
            subtree_size: &self.subtree_size,
            chunks_by_depth: &self.chunks_by_depth,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ChunkDependenceException {
    message: String,
    chunk: JSChunk,
    dependent_chunk: JSChunk,
}
impl ChunkDependenceException {
    // port: JSChunkGraph.ChunkDependenceException#ChunkDependenceException
    pub fn new(message: String, chunk: JSChunk, dependent_chunk: JSChunk) -> Self {
        Self {
            message,
            chunk,
            dependent_chunk,
        }
    }
    // port: JSChunkGraph.ChunkDependenceException#getChunk
    pub fn get_chunk(&self) -> JSChunk {
        self.chunk.clone()
    }
    // port: JSChunkGraph.ChunkDependenceException#getDependentChunk
    pub fn get_dependent_chunk(&self) -> JSChunk {
        self.dependent_chunk.clone()
    }
}
impl fmt::Display for ChunkDependenceException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ChunkDependenceException {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingChunkException(pub String);
impl MissingChunkException {
    // port: JSChunkGraph.MissingChunkException#MissingChunkException
    pub fn new(chunk_name: impl Into<String>) -> Self {
        Self(chunk_name.into())
    }
}
impl fmt::Display for MissingChunkException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for MissingChunkException {}
impl JSChunkGraph {
    // port: JSChunkGraph#toGraphvizGraph
    pub fn to_graphviz_graph(
        &self,
    ) -> crate::graph::linked_directed_graph::LinkedDirectedGraph<JSChunk, String> {
        use crate::graph::graph::Graph;
        let mut graph = crate::graph::linked_directed_graph::LinkedDirectedGraph::create();
        for chunk in &self.chunks {
            graph.create_node(chunk.clone());
            for dep in chunk.get_dependencies() {
                graph.create_node(dep.clone());
                graph.connect(chunk.clone(), "->".into(), dep);
            }
        }
        graph
    }
    // port: JSChunkGraph#JSChunkGraph(List)
    pub fn new(chunks_in_dep_order: Vec<JSChunk>) -> Result<Self, ChunkDependenceException> {
        assert!(!chunks_in_dep_order.is_empty());
        let chunks = Self::make_weak_chunk(chunks_in_dep_order);
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            assert!(chunk.get_index() == -1, "Chunk index already set: {chunk}");
            chunk.set_index(chunk_index as i32);
        }
        let mut graph = Self {
            chunks,
            self_plus_transitive_deps: vec![],
            subtree_size: vec![],
            chunks_by_depth: vec![],
            dependency_map: std::sync::Mutex::new(IndexMap::<_, _>::default()),
        };
        graph.chunks_by_depth = graph.init_chunks_by_depth()?;
        graph.self_plus_transitive_deps = graph.init_transitive_deps_bit_sets();
        graph.subtree_size = graph.init_subtree_size();
        Self::move_marked_weak_sources(
            &graph.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap(),
            graph.get_all_inputs(),
        );
        Ok(graph)
    }
    // port: JSChunkGraph#JSChunkGraph(JSChunk[])
    pub fn new_from_slice(chunks: &[JSChunk]) -> Result<Self, ChunkDependenceException> {
        Self::new(chunks.to_vec())
    }
    // port: JSChunkGraph#initChunksByDepth
    fn init_chunks_by_depth(&self) -> Result<Vec<Vec<JSChunk>>, ChunkDependenceException> {
        let mut tmp_chunks_by_depth: Vec<Vec<JSChunk>> = vec![];
        for chunk in &self.chunks {
            assert!(chunk.get_depth() == -1, "Chunk depth already set: {chunk}");
            let mut depth = 0;
            for dep in chunk.get_dependencies() {
                let dep_depth = dep.get_depth();
                if dep_depth < 0 {
                    return Err(ChunkDependenceException::new(
                        format!(
                            "Chunks not in dependency order: {} preceded {}",
                            chunk.get_name(),
                            dep.get_name()
                        ),
                        chunk.clone(),
                        dep,
                    ));
                }
                depth = depth.max(dep_depth + 1);
            }
            chunk.set_depth(depth);
            if depth as usize == tmp_chunks_by_depth.len() {
                tmp_chunks_by_depth.push(vec![]);
            }
            tmp_chunks_by_depth[depth as usize].push(chunk.clone());
        }
        Ok(tmp_chunks_by_depth)
    }
    // port: JSChunkGraph#makeWeakChunk
    fn make_weak_chunk(mut chunks: Vec<JSChunk>) -> Vec<JSChunk> {
        let mut has_weak_chunk = false;
        for chunk in &chunks {
            if chunk.get_name() == WEAK_CHUNK_NAME {
                has_weak_chunk = true;
                let mut all_other_chunks: IndexSet<_> = chunks.iter().cloned().collect();
                all_other_chunks.shift_remove(chunk);
                let deps = chunk.get_all_dependencies();
                assert!(
                    all_other_chunks.iter().all(|c| deps.contains(c)),
                    "A weak chunk already exists but it does not depend on every other chunk."
                );
                assert!(
                    deps.len() == all_other_chunks.len(),
                    "The weak chunk cannot have extra dependencies."
                );
                break;
            }
        }
        if has_weak_chunk {
            let mut misplaced_weak_files = vec![];
            let mut misplaced_strong_files = vec![];
            for chunk in &chunks {
                let is_weak_chunk = chunk.get_name() == WEAK_CHUNK_NAME;
                for input in chunk.get_inputs() {
                    let source = input.get_source_file();
                    if is_weak_chunk && !source.is_weak() {
                        misplaced_strong_files.push(source.get_name().to_owned());
                    } else if !is_weak_chunk && source.is_weak() {
                        misplaced_weak_files.push(format!(
                            "{} (in chunk {})",
                            source.get_name(),
                            chunk.get_name()
                        ));
                    }
                }
            }
            if !(misplaced_strong_files.is_empty() && misplaced_weak_files.is_empty()) {
                let mut sb = String::from("A weak chunk exists but some sources are misplaced.");
                if !misplaced_strong_files.is_empty() {
                    sb.push_str("\nFound these strong sources in the weak chunk:\n  ");
                    sb.push_str(&misplaced_strong_files.join("\n  "));
                }
                if !misplaced_weak_files.is_empty() {
                    sb.push_str("\nFound these weak sources in other chunks:\n  ");
                    sb.push_str(&misplaced_weak_files.join("\n  "));
                }
                panic!("{sb}");
            }
        } else {
            let weak_chunk = JSChunk::new(WEAK_CHUNK_NAME);
            for chunk in &chunks {
                weak_chunk.add_dependency(chunk);
            }
            chunks.push(weak_chunk);
        }
        chunks
    }
    // port: JSChunkGraph#initTransitiveDepsBitSets
    fn init_transitive_deps_bit_sets(&self) -> Vec<BitSet> {
        let mut array: Vec<BitSet> = Vec::with_capacity(self.chunks.len());
        for (chunk_index, chunk) in self.chunks.iter().enumerate() {
            let mut deps = BitSet::new();
            deps.set(chunk_index);
            for dep in chunk.get_dependencies() {
                deps.or(&array[dep.get_index() as usize]);
            }
            array.push(deps);
        }
        array
    }
    // port: JSChunkGraph#initSubtreeSize
    fn init_subtree_size(&self) -> Vec<i32> {
        let mut subtree_size = vec![0; self.chunks.len()];
        for (dependent_index, dependencies) in self.self_plus_transitive_deps.iter().enumerate() {
            let mut required_index = Some(dependent_index);
            while let Some(i) = required_index {
                subtree_size[i] += 1;
                required_index = dependencies.previous_set_bit(i as i32 - 1);
            }
        }
        subtree_size
    }
    // port: JSChunkGraph#getAllInputs
    pub fn get_all_inputs(&self) -> Vec<CompilerInput> {
        self.chunks.iter().flat_map(JSChunk::get_inputs).collect()
    }
    // port: JSChunkGraph#getInputCount
    pub fn get_input_count(&self) -> usize {
        self.chunks.iter().map(JSChunk::get_input_count).sum()
    }
    // port: JSChunkGraph#getAllChunks
    pub fn get_all_chunks(&self) -> &[JSChunk] {
        &self.chunks
    }
    // port: JSChunkGraph#getIntegralChunkArrayForSerialization
    pub fn get_integral_chunk_array_for_serialization(&self) -> &[JSChunk] {
        &self.chunks
    }
    // port: JSChunkGraph#getChunkByName
    pub fn get_chunk_by_name(&self, name: &str) -> Option<JSChunk> {
        self.chunks.iter().find(|c| c.get_name() == name).cloned()
    }
    // port: JSChunkGraph#getChunksByName
    pub fn get_chunks_by_name(&self) -> IndexMap<String, JSChunk> {
        self.chunks
            .iter()
            .map(|c| (c.get_name(), c.clone()))
            .collect()
    }
    // port: JSChunkGraph#getChunkCount
    pub fn get_chunk_count(&self) -> usize {
        self.chunks.len()
    }
    // port: JSChunkGraph#getRootChunk
    pub fn get_root_chunk(&self) -> JSChunk {
        assert_eq!(self.chunks_by_depth[0].len(), 1);
        self.chunks_by_depth[0][0].clone()
    }
    // port: JSChunkGraph#toJson
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Array(self.chunks.iter().map(|chunk| serde_json::json!({
            "name": chunk.get_name(), "dependencies": chunk.get_dependencies().iter().map(JSChunk::get_name).collect::<Vec<_>>(),
            "transitive-dependencies": self.get_transitive_deps_deepest_first(chunk).iter().map(JSChunk::get_name).collect::<Vec<_>>(),
            "inputs": chunk.get_inputs().iter().map(|i| i.get_source_file().get_name().to_owned()).collect::<Vec<_>>() })).collect())
    }
    // port: JSChunkGraph#dependsOn
    pub fn depends_on(&self, src: &JSChunk, chunk: &JSChunk) -> bool {
        src != chunk
            && self.self_plus_transitive_deps[src.get_index() as usize]
                .get(chunk.get_index() as usize)
    }
    // port: JSChunkGraph#getSmallestCoveringSubtree
    pub fn get_smallest_covering_subtree(
        &self,
        parent_tree: &JSChunk,
        dependent_chunks: &BitSet,
    ) -> JSChunk {
        assert!(!dependent_chunks.is_empty());
        let mut min_dependent_chunk_index = self.chunks.len();
        let mut candidates = BitSet::new();
        for i in 0..self.chunks.len() {
            candidates.set(i);
        }
        let mut dependent_index = dependent_chunks.next_set_bit(0);
        while let Some(i) = dependent_index {
            min_dependent_chunk_index = min_dependent_chunk_index.min(i);
            candidates.and(&self.self_plus_transitive_deps[i]);
            dependent_index = dependent_chunks.next_set_bit(i + 1);
        }
        assert!(
            !candidates.is_empty(),
            "No common dependency found for {dependent_chunks}"
        );
        let parent_tree_index = parent_tree.get_index() as usize;
        let mut best_candidate_index = parent_tree_index;
        let mut candidate_index = candidates.previous_set_bit(min_dependent_chunk_index as i32);
        while let Some(i) = candidate_index {
            let deps = &self.self_plus_transitive_deps[i];
            if deps.get(parent_tree_index) {
                candidates.and_not(deps);
                if self.subtree_size[i] < self.subtree_size[best_candidate_index] {
                    best_candidate_index = i;
                }
            }
            candidate_index = candidates.previous_set_bit(i as i32 - 1);
        }
        self.chunks[best_candidate_index].clone()
    }
    // port: JSChunkGraph#getDeepestCommonDependency
    pub fn get_deepest_common_dependency(&self, m1: &JSChunk, m2: &JSChunk) -> Option<JSChunk> {
        for depth in (0..m1.get_depth().min(m2.get_depth())).rev() {
            for m in self.chunks_by_depth[depth as usize].iter().rev() {
                if self.depends_on(m1, m) && self.depends_on(m2, m) {
                    return Some(m.clone());
                }
            }
        }
        None
    }
    // port: JSChunkGraph#getDeepestCommonDependencyInclusive(JSChunk, JSChunk)
    pub fn get_deepest_common_dependency_inclusive(
        &self,
        m1: &JSChunk,
        m2: &JSChunk,
    ) -> Option<JSChunk> {
        if m2 == m1 || self.depends_on(m2, m1) {
            Some(m1.clone())
        } else if self.depends_on(m1, m2) {
            Some(m2.clone())
        } else {
            self.get_deepest_common_dependency(m1, m2)
        }
    }
    // port: JSChunkGraph#getDeepestCommonDependencyInclusive(Collection)
    pub fn get_deepest_common_dependency_inclusive_collection(
        &self,
        chunks: &[JSChunk],
    ) -> Option<JSChunk> {
        let mut dep = Some(chunks.first().unwrap().clone());
        for chunk in &chunks[1..] {
            dep = self.get_deepest_common_dependency_inclusive(dep.as_ref().unwrap(), chunk);
        }
        dep
    }
    // port: JSChunkGraph#getTransitiveDepsDeepestFirst
    pub fn get_transitive_deps_deepest_first(&self, chunk: &JSChunk) -> Vec<JSChunk> {
        let mut deps: Vec<_> = self.get_transitive_deps(chunk).into_iter().collect();
        deps.sort_by(InverseDepthComparator::compare);
        deps
    }
    // port: JSChunkGraph#getTransitiveDeps
    fn get_transitive_deps(&self, chunk: &JSChunk) -> IndexSet<JSChunk> {
        self.dependency_map
            .lock()
            .unwrap()
            .entry(chunk.clone())
            .or_insert_with(|| chunk.get_all_dependencies())
            .clone()
    }
    // port: JSChunkGraph#moveMarkedWeakSources
    fn move_marked_weak_sources(weak_chunk: &JSChunk, inputs: Vec<CompilerInput>) {
        for input in inputs {
            if input.get_source_file().is_weak() {
                let existing_chunk = input.get_chunk();
                if existing_chunk.as_ref() == Some(weak_chunk) {
                    continue;
                }
                if let Some(chunk) = existing_chunk {
                    chunk.remove(&input);
                }
                weak_chunk.add(input);
            }
        }
    }
    // port: JSChunkGraph#depthCompare
    fn depth_compare(m1: &JSChunk, m2: &JSChunk) -> Ordering {
        if m1 == m2 {
            return Ordering::Equal;
        }
        m1.get_depth().cmp(&m2.get_depth()).then_with(|| {
            m1.get_name()
                .encode_utf16()
                .cmp(m2.get_name().encode_utf16())
        })
    }
}
pub struct InverseDepthComparator;
impl InverseDepthComparator {
    // port: JSChunkGraph.InverseDepthComparator#compare
    pub fn compare(m1: &JSChunk, m2: &JSChunk) -> Ordering {
        JSChunkGraph::depth_compare(m2, m1)
    }
}

pub struct DependencyManagementResult {
    pub ordered_inputs: Vec<CompilerInput>,
    pub sorter: crate::deps::sorted_dependencies::SortedDependencies<
        crate::compiler_input::CompilerInputDependencyInfo,
    >,
    pub entry_point_inputs: IndexSet<CompilerInput>,
}
#[derive(Debug)]
pub enum DependencyManagementError {
    MissingProvide(crate::deps::sorted_dependencies::MissingProvideException),
    MissingChunk(MissingChunkException),
}
impl fmt::Display for DependencyManagementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProvide(error) => fmt::Display::fmt(error, f),
            Self::MissingChunk(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for DependencyManagementError {}
impl JSChunkGraph {
    // port: JSChunkGraph#manageDependencies
    pub fn manage_dependencies(
        &self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        dependency_options: &crate::dependency_options::DependencyOptions,
    ) -> Result<DependencyManagementResult, DependencyManagementError> {
        use crate::deps::sorted_dependencies::SortedDependencies;
        use crate::js_error::JSError;
        let original_inputs = self.get_all_inputs();
        let views: Vec<_> = original_inputs
            .iter()
            .map(|input| input.dependency_snapshot(compiler))
            .collect();
        let sorter = SortedDependencies::new(views.clone());
        let mut entry_point_inputs = self.create_entry_point_inputs(
            compiler,
            dependency_options,
            &original_inputs,
            &sorter,
        )?;
        let mut inputs_by_provide: IndexMap<String, IndexSet<CompilerInput>> =
            IndexMap::<_, _>::default();
        for input in &original_inputs {
            for provide in input.get_known_provides() {
                inputs_by_provide
                    .entry(provide)
                    .or_default()
                    .insert(input.clone());
            }
            inputs_by_provide
                .entry(input.get_path(compiler).to_module_name())
                .or_default()
                .insert(input.clone());
        }
        for input in &original_inputs {
            for require in input.get_dynamic_requires() {
                if let Some(inputs) = inputs_by_provide.get(&require) {
                    entry_point_inputs.extend(inputs.iter().cloned());
                }
            }
        }
        for input in &original_inputs {
            for require in input.get_require_dynamic_imports() {
                if let Some(inputs) = inputs_by_provide.get(&require) {
                    entry_point_inputs.extend(inputs.iter().cloned());
                }
            }
        }
        let absolute_order: Vec<_> = sorter
            .get_strong_dependencies_of(&views, dependency_options.should_sort())
            .into_iter()
            .map(|view| view.input)
            .collect();
        let mut entry_point_inputs_per_chunk: IndexMap<JSChunk, Vec<CompilerInput>> =
            IndexMap::<_, _>::default();
        for input in &entry_point_inputs {
            entry_point_inputs_per_chunk
                .entry(input.get_chunk().unwrap())
                .or_default()
                .push(input.clone());
        }
        for chunk in self.get_all_chunks() {
            chunk.remove_all();
        }
        let mut ordered_inputs = Vec::new();
        let mut reached_inputs = IndexSet::<_>::default();
        for chunk in &self.chunks {
            let entries = entry_point_inputs_per_chunk
                .get(chunk)
                .cloned()
                .unwrap_or_default();
            let transitive_closure: Vec<CompilerInput>;
            if dependency_options.should_sort() && dependency_options.should_prune() {
                let mut closure = Vec::new();
                let mut inputs_not_yet_reached = original_inputs.iter().cloned().collect();
                for entry_point in &entries {
                    closure.extend(Self::get_depth_first_dependencies_of(
                        compiler,
                        entry_point,
                        &mut inputs_not_yet_reached,
                        &inputs_by_provide,
                    ));
                }
                for input in &closure {
                    if reached_inputs.insert(input.clone()) {
                        ordered_inputs.push(input.clone());
                    }
                }
                transitive_closure = closure;
            } else {
                let roots: Vec<_> = entries
                    .iter()
                    .map(|input| {
                        views
                            .iter()
                            .find(|view| &view.input == input)
                            .unwrap()
                            .clone()
                    })
                    .collect();
                transitive_closure = sorter
                    .get_strong_dependencies_of(&roots, dependency_options.should_sort())
                    .into_iter()
                    .map(|view| view.input)
                    .collect();
            }
            for input in transitive_closure {
                if dependency_options.should_prune()
                    && input.get_source_file().is_weak()
                    && !entry_point_inputs.contains(&input)
                {
                    compiler.report(JSError::make_without_location(
                        &WEAK_FILE_REACHABLE_FROM_ENTRY_POINT_ERROR,
                        &[input.get_source_file().get_name()],
                    ));
                }
                if let Some(old_chunk) = input.get_chunk() {
                    input.set_chunk(None);
                    input.set_chunk(
                        self.get_deepest_common_dependency_inclusive(&old_chunk, chunk)
                            .as_ref(),
                    );
                } else {
                    input.set_chunk(Some(chunk));
                }
            }
        }
        if !(dependency_options.should_sort() && dependency_options.should_prune())
            || entry_point_inputs_per_chunk.is_empty()
        {
            ordered_inputs = absolute_order;
        }
        let weak_chunk = self.get_chunk_by_name(WEAK_CHUNK_NAME).unwrap();
        if dependency_options.should_prune() {
            let roots: Vec<_> = ordered_inputs
                .iter()
                .map(|input| {
                    views
                        .iter()
                        .find(|view| &view.input == input)
                        .unwrap()
                        .clone()
                })
                .collect();
            for view in sorter.get_sorted_weak_dependencies_of(&roots) {
                let input = view.input;
                assert!(input.get_chunk().is_none());
                input
                    .get_source_file()
                    .set_kind(closure_rhino::static_source_file::SourceKind::WEAK);
                input.set_chunk(Some(&weak_chunk));
                weak_chunk.add(input);
            }
        } else {
            Self::move_marked_weak_sources(&weak_chunk, original_inputs);
        }
        for input in &ordered_inputs {
            if let Some(chunk) = input.get_chunk()
                && chunk.get_by_name(input.get_name()).is_none()
            {
                chunk.add(input.clone());
            }
        }
        Ok(DependencyManagementResult {
            ordered_inputs,
            sorter,
            entry_point_inputs,
        })
    }
    // port: JSChunkGraph#getDepthFirstDependenciesOf
    fn get_depth_first_dependencies_of(
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        root_input: &CompilerInput,
        unreached_inputs: &mut IndexSet<CompilerInput>,
        inputs_by_provide: &IndexMap<String, IndexSet<CompilerInput>>,
    ) -> Vec<CompilerInput> {
        let mut ordered_inputs = Vec::new();
        if !unreached_inputs.shift_remove(root_input) {
            return ordered_inputs;
        }
        for namespace in crate::deps::dependency_info::Require::as_symbol_list(
            &root_input.get_requires(compiler),
        ) {
            if let Some(inputs) = inputs_by_provide.get(&namespace) {
                for input in inputs {
                    if unreached_inputs.contains(input) {
                        ordered_inputs.extend(Self::get_depth_first_dependencies_of(
                            compiler,
                            input,
                            unreached_inputs,
                            inputs_by_provide,
                        ));
                    }
                }
            }
        }
        ordered_inputs.push(root_input.clone());
        ordered_inputs
    }
    // port: JSChunkGraph#createEntryPointInputs
    fn create_entry_point_inputs(
        &self,
        compiler: &mut crate::abstract_compiler::AbstractCompiler,
        dependency_options: &crate::dependency_options::DependencyOptions,
        inputs: &[CompilerInput],
        sorter: &crate::deps::sorted_dependencies::SortedDependencies<
            crate::compiler_input::CompilerInputDependencyInfo,
        >,
    ) -> Result<IndexSet<CompilerInput>, DependencyManagementError> {
        use crate::{deps::sorted_dependencies::MissingProvideException, js_error::JSError};
        let mut entry_point_inputs = IndexSet::<_>::default();
        let chunks_by_name = self.get_chunks_by_name();
        if dependency_options.should_prune() {
            if let Some(base) = sorter.maybe_get_input_providing("goog") {
                entry_point_inputs.insert(base.input.clone());
            }
            if !dependency_options.should_drop_moochers() {
                for view in sorter.get_inputs_without_provides() {
                    let input = view.input;
                    if input.get_source_file().is_weak() {
                        compiler.report(JSError::make_without_location(
                            &IMPLICIT_WEAK_ENTRY_POINT_ERROR,
                            &[input.get_source_file().get_name()],
                        ));
                    } else {
                        entry_point_inputs.insert(input);
                    }
                }
            }
            for entry_point in dependency_options.entry_points() {
                let input = if entry_point.closure_namespace() == entry_point.module_name() {
                    if let Some(view) =
                        sorter.maybe_get_input_providing(entry_point.closure_namespace())
                    {
                        view.input.clone()
                    } else {
                        sorter
                            .get_input_providing(entry_point.name())
                            .map_err(|_| {
                                DependencyManagementError::MissingProvide(
                                    MissingProvideException::new(entry_point.name()),
                                )
                            })?
                            .input
                    }
                } else {
                    let chunk = chunks_by_name
                        .get(entry_point.module_name())
                        .ok_or_else(|| {
                            DependencyManagementError::MissingChunk(MissingChunkException::new(
                                entry_point.module_name(),
                            ))
                        })?;
                    let input = sorter
                        .get_input_providing(entry_point.closure_namespace())
                        .map_err(|_| {
                            DependencyManagementError::MissingProvide(MissingProvideException::new(
                                entry_point.name(),
                            ))
                        })?
                        .input;
                    input.override_chunk(chunk);
                    input
                };
                if input.get_source_file().is_weak() {
                    compiler.report(JSError::make_without_location(
                        &EXPLICIT_WEAK_ENTRY_POINT_ERROR,
                        &[input.get_source_file().get_name()],
                    ));
                } else {
                    entry_point_inputs.insert(input);
                }
            }
        } else {
            entry_point_inputs.extend(inputs.iter().cloned());
        }
        Ok(entry_point_inputs)
    }
}
