/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/testing/JSChunkGraphBuilder.java.

//! Port of testing/JSChunkGraphBuilder.java: builds a list of JSChunks with a given dependency
//! shape and one source file per chunk, for tests.
use closure_jscomp::js_chunk::JSChunk;
use closure_jscomp::source_file::SourceFile;
use closure_rhino::check_state;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::jscomp_base::guava_format;

// port: JSChunkGraphBuilder.GraphType
#[allow(clippy::upper_case_acronyms)] // Java enum constant names
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphType {
    /// each chunk depends on the chunk immediately before
    CHAIN,
    /// each chunk depends on the fist chunk
    STAR,
    /// no dependencies between chunks
    DISJOINT,
    /// chunk 2 depends on chunk 1, and all others depend on chunk 2
    BUSH,
    /// binary tree
    TREE,
}

impl GraphType {
    // port: JSChunkGraphBuilder.GraphType#addDependencyEdges
    fn add_dependency_edges(self, chunks: &[JSChunk]) {
        match self {
            GraphType::CHAIN => {
                for i in 1..chunks.len() {
                    chunks[i].add_dependency(&chunks[i - 1]);
                }
            }
            GraphType::STAR => {
                for chunk in chunks.iter().skip(1) {
                    chunk.add_dependency(&chunks[0]);
                }
            }
            GraphType::DISJOINT => {}
            GraphType::BUSH => {
                check_state!(chunks.len() > 2, "BUSHes need at least three graph nodes");
                for (i, chunk) in chunks.iter().enumerate().skip(1) {
                    chunk.add_dependency(&chunks[if i == 1 { 0 } else { 1 }]);
                }
            }
            GraphType::TREE => {
                for (i, chunk) in chunks.iter().enumerate().skip(1) {
                    chunk.add_dependency(&chunks[(i - 1) / 2]);
                }
            }
        }
    }
}

// port: JSChunkGraphBuilder
pub struct JSChunkGraphBuilder {
    graph_type: GraphType,
    chunks: Vec<String>,
    /// correspondances between chunk indices in `chunks` and specified names. if no name is
    /// specified defaults to "m{i}"
    chunk_names: IndexMap<usize, String>,
    filename_format: String,
}

impl JSChunkGraphBuilder {
    // port: JSChunkGraphBuilder#JSChunkGraphBuilder
    fn new(graph_type: GraphType) -> Self {
        Self {
            graph_type,
            chunks: Vec::new(),
            chunk_names: IndexMap::<_, _>::default(),
            filename_format: "i%s.js".to_string(),
        }
    }

    // port: JSChunkGraphBuilder#forChain
    pub fn for_chain() -> Self {
        Self::new(GraphType::CHAIN)
    }

    // port: JSChunkGraphBuilder#forStar
    pub fn for_star() -> Self {
        Self::new(GraphType::STAR)
    }

    // port: JSChunkGraphBuilder#forBush
    pub fn for_bush() -> Self {
        Self::new(GraphType::BUSH)
    }

    // port: JSChunkGraphBuilder#forTree
    pub fn for_tree() -> Self {
        Self::new(GraphType::TREE)
    }

    // port: JSChunkGraphBuilder#forUnordered
    pub fn for_unordered() -> Self {
        Self::new(GraphType::DISJOINT)
    }

    // port: JSChunkGraphBuilder#addChunk
    pub fn add_chunk(mut self, input: impl Into<String>) -> Self {
        self.chunks.push(input.into());
        self
    }

    // port: JSChunkGraphBuilder#addChunkWithName
    pub fn add_chunk_with_name(
        mut self,
        input: impl Into<String>,
        chunk_name: impl Into<String>,
    ) -> Self {
        self.chunk_names
            .insert(self.chunks.len(), chunk_name.into());
        self.chunks.push(input.into());
        self
    }

    // port: JSChunkGraphBuilder#addChunks(List)
    pub fn add_chunks<S: Into<String>>(mut self, inputs: impl IntoIterator<Item = S>) -> Self {
        self.chunks.extend(inputs.into_iter().map(Into::into));
        self
    }

    // port: JSChunkGraphBuilder#setFilenameFormat
    pub fn set_filename_format(mut self, filename_format: impl Into<String>) -> Self {
        self.filename_format = filename_format.into();
        self
    }

    // port: JSChunkGraphBuilder#build
    pub fn build(&self) -> Vec<JSChunk> {
        let mut chunks = Vec::with_capacity(self.chunks.len());
        for (i, code) in self.chunks.iter().enumerate() {
            let chunk_name = self
                .chunk_names
                .get(&i)
                .cloned()
                .unwrap_or_else(|| format!("m{i}"));
            let chunk = JSChunk::new(chunk_name);
            chunk.add_source_file(SourceFile::from_code(
                &guava_format(&self.filename_format, &[i.to_string()]),
                code.as_str(),
            ));
            chunks.push(chunk);
        }
        self.graph_type.add_dependency_edges(&chunks);
        chunks
    }
}
