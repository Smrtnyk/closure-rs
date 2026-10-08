/*
 * Copyright 2005 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JSChunk.java.

// Identity keys are immutable even though the associated Java objects are mutable.
#![allow(clippy::mutable_key_type)]
use crate::{compiler_input::CompilerInput, source_file::SourceFile};
use indexmap::{IndexMap, IndexSet};
use std::{
    collections::VecDeque,
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

pub const STRONG_CHUNK_NAME: &str = "$strong$";
pub const WEAK_CHUNK_NAME: &str = "$weak$";
#[derive(Clone)]
pub struct JSChunk(pub(crate) Arc<Mutex<JSChunkData>>);
pub(crate) struct JSChunkData {
    name: String,
    inputs: IndexMap<String, CompilerInput>,
    deps: Vec<JSChunk>,
    depth: i32,
    index: i32,
}
impl JSChunk {
    pub const STRONG_CHUNK_NAME: &'static str = STRONG_CHUNK_NAME;
    pub const WEAK_CHUNK_NAME: &'static str = WEAK_CHUNK_NAME;
    // port: JSChunk#JSChunk
    pub fn new(name: impl Into<String>) -> Self {
        Self(Arc::new(Mutex::new(JSChunkData {
            name: name.into(),
            inputs: IndexMap::new(),
            deps: Vec::new(),
            depth: -1,
            index: -1,
        })))
    }
    // port: JSChunk#getName
    pub fn get_name(&self) -> String {
        self.0.lock().unwrap().name.clone()
    }
    // port: JSChunk#getProvides
    pub fn get_provides(&self) -> Vec<String> {
        vec![self.get_name()]
    }
    // port: JSChunk#getHasExternsAnnotation
    pub fn get_has_externs_annotation(&self) -> bool {
        false
    }
    // port: JSChunk#getHasNoCompileAnnotation
    pub fn get_has_no_compile_annotation(&self) -> bool {
        false
    }
    // port: JSChunk#getTypeRequires
    pub fn get_type_requires(&self) -> Vec<String> {
        vec![]
    }
    // port: JSChunk#getPathRelativeToClosureBase
    pub fn get_path_relative_to_closure_base(&self) -> String {
        panic!("UnsupportedOperationException")
    }
    // port: JSChunk#getLoadFlags
    pub fn get_load_flags(&self) -> IndexMap<String, String> {
        panic!("UnsupportedOperationException")
    }
    // port: JSChunk#isGoogModule
    pub fn is_goog_module(&self) -> bool {
        panic!("UnsupportedOperationException")
    }
    // port: JSChunk#isEs6Module
    pub fn is_es6_module(&self) -> bool {
        panic!("UnsupportedOperationException")
    }
    // port: JSChunk#add(SourceFile)
    pub fn add_source_file(&self, file: impl Into<Arc<SourceFile>>) {
        self.add(CompilerInput::new(file));
    }
    // port: JSChunk#add(CompilerInput)
    pub fn add(&self, input: CompilerInput) {
        let input_name = input.get_name().to_owned();
        let previous = self
            .0
            .lock()
            .unwrap()
            .inputs
            .insert(input_name.clone(), input.clone());
        assert!(
            previous.is_none(),
            "{} already exist in chunk {}",
            input_name,
            self.get_name()
        );
        input.set_chunk(Some(self));
    }
    // port: JSChunk#addDependency
    pub fn add_dependency(&self, dep: &Self) {
        assert!(dep != self, "Cannot add dependency on self ({self})");
        self.0.lock().unwrap().deps.push(dep.clone());
    }
    // port: JSChunk#remove
    pub fn remove(&self, input: &CompilerInput) {
        input.set_chunk(None);
        self.0.lock().unwrap().inputs.shift_remove(input.get_name());
    }
    // port: JSChunk#removeAll
    pub fn remove_all(&self) {
        for input in self.get_inputs() {
            input.set_chunk(None);
        }
        self.0.lock().unwrap().inputs.clear();
    }
    // port: JSChunk#getDependencies
    pub fn get_dependencies(&self) -> Vec<Self> {
        self.0.lock().unwrap().deps.clone()
    }
    // port: JSChunk#getSortedDependencyNames
    pub fn get_sorted_dependency_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.get_dependencies().iter().map(Self::get_name).collect();
        names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        names
    }
    // port: JSChunk#getAllDependencies
    pub fn get_all_dependencies(&self) -> IndexSet<Self> {
        let deps = self.get_dependencies();
        let mut all_deps: IndexSet<_> = deps.iter().cloned().collect();
        let mut stack: VecDeque<_> = deps.into();
        while let Some(chunk) = stack.pop_front() {
            for dep in chunk.get_dependencies() {
                if all_deps.insert(dep.clone()) {
                    stack.push_front(dep);
                }
            }
        }
        all_deps
    }
    // port: JSChunk#getThisAndAllDependencies
    pub fn get_this_and_all_dependencies(&self) -> IndexSet<Self> {
        let mut deps = self.get_all_dependencies();
        deps.insert(self.clone());
        deps
    }
    // port: JSChunk#getInputCount
    pub fn get_input_count(&self) -> usize {
        self.0.lock().unwrap().inputs.len()
    }
    // port: JSChunk#getFirst
    pub fn get_first(&self) -> CompilerInput {
        self.0.lock().unwrap().inputs.first().unwrap().1.clone()
    }
    // port: JSChunk#getInputs
    pub fn get_inputs(&self) -> Vec<CompilerInput> {
        self.0.lock().unwrap().inputs.values().cloned().collect()
    }
    // port: JSChunk#getInputsIterable
    pub fn get_inputs_iterable(&self) -> impl Iterator<Item = CompilerInput> {
        self.get_inputs().into_iter()
    }
    // port: JSChunk#getByName
    pub fn get_by_name(&self, name: &str) -> Option<CompilerInput> {
        self.0.lock().unwrap().inputs.get(name).cloned()
    }
    // port: JSChunk#removeByName
    pub fn remove_by_name(&self, name: &str) -> bool {
        self.0.lock().unwrap().inputs.shift_remove(name).is_some()
    }
    // port: JSChunk#isSynthetic
    pub fn is_synthetic(&self) -> bool {
        matches!(
            self.get_name().as_str(),
            STRONG_CHUNK_NAME | WEAK_CHUNK_NAME
        )
    }
    // port: JSChunk#isWeak
    pub fn is_weak(&self) -> bool {
        self.get_name() == WEAK_CHUNK_NAME
    }
    // port: JSChunk#setDepth
    pub fn set_depth(&self, dep: i32) {
        assert!(dep >= 0, "invalid depth: {dep}");
        self.0.lock().unwrap().depth = dep;
    }
    // port: JSChunk#getDepth
    pub fn get_depth(&self) -> i32 {
        self.0.lock().unwrap().depth
    }
    // port: JSChunk#setIndex
    pub fn set_index(&self, index: i32) {
        assert!(index >= 0, "Invalid chunk index: {index}");
        self.0.lock().unwrap().index = index;
    }
    // port: JSChunk#getIndex
    pub fn get_index(&self) -> i32 {
        self.0.lock().unwrap().index
    }
}
impl PartialEq for JSChunk {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for JSChunk {}
impl Hash for JSChunk {
    fn hash<H: Hasher>(&self, h: &mut H) {
        Arc::as_ptr(&self.0).hash(h);
    }
}
impl fmt::Debug for JSChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl fmt::Display for JSChunk {
    // port: JSChunk#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.get_name())
    }
}
impl JSChunk {
    // port: JSChunk#getRequires
    pub fn get_requires(&self) -> Vec<crate::deps::dependency_info::Require> {
        self.get_dependencies()
            .iter()
            .map(|chunk| crate::deps::dependency_info::Require::compiler_chunk(&chunk.get_name()))
            .collect()
    }
    // port: JSChunk#sortInputsByDeps
    pub fn sort_inputs_by_deps(&self, compiler: &mut crate::abstract_compiler::AbstractCompiler) {
        let inputs = self.get_inputs();
        for input in &inputs {
            input.set_compiler(compiler);
        }
        let sorted = crate::deps::sorted_dependencies::SortedDependencies::new(
            inputs
                .iter()
                .map(|input| input.dependency_snapshot(compiler))
                .collect(),
        );
        let mut data = self.0.lock().unwrap();
        data.inputs.clear();
        for input in sorted.get_sorted_list() {
            data.inputs
                .insert(input.input.get_name().into(), input.input.clone());
        }
    }
}
