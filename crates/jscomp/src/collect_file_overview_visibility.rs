/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CollectFileOverviewVisibility.java.

//! Port of CollectFileOverviewVisibility.java: the compiler pass that collects visibility
//! annotations in `@fileoverview` blocks. Used by CheckAccessControls.
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use closure_rhino::check_state;
use closure_rhino::jsdoc_info::Visibility;
use closure_rhino::node::NodeId;
use closure_rhino::static_source_file::StaticSourceFile;
use indexmap::IndexMap;
use std::sync::Arc;

/// Java's `ImmutableMap<StaticSourceFile, Visibility>`. Neither `SourceFile` nor
/// `SimpleSourceFile` overrides `equals`/`hashCode`, so keys compare by object identity, which is
/// the address of the shared `Arc`. The map keeps the key `Arc`s, so an address is never reused
/// while the map lives.
#[derive(Clone, Default)]
pub struct FileVisibilityMap {
    entries: IndexMap<usize, (Arc<dyn StaticSourceFile>, Visibility)>,
}

impl FileVisibilityMap {
    fn key(file: &Arc<dyn StaticSourceFile>) -> usize {
        Arc::as_ptr(file) as *const () as usize
    }

    /// `ImmutableMap#get`: a null key finds nothing.
    pub fn get(&self, file: Option<&Arc<dyn StaticSourceFile>>) -> Option<Visibility> {
        file.and_then(|f| self.entries.get(&Self::key(f)).map(|(_, v)| *v))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Java's `ImmutableMap.Builder<StaticSourceFile, Visibility>`.
#[derive(Default)]
struct FileVisibilityMapBuilder {
    entries: Vec<(Option<Arc<dyn StaticSourceFile>>, Visibility)>,
}

impl FileVisibilityMapBuilder {
    // port: ImmutableMap.Builder#put (null keys are rejected)
    fn put(&mut self, key: Option<Arc<dyn StaticSourceFile>>, value: Visibility) {
        if key.is_none() {
            panic!("NullPointerException: null key in entry: null={value:?}");
        }
        self.entries.push((key, value));
    }

    // port: ImmutableMap.Builder#buildOrThrow
    fn build_or_throw(&self) -> FileVisibilityMap {
        let mut entries: IndexMap<usize, (Arc<dyn StaticSourceFile>, Visibility)> = IndexMap::new();
        for (key, value) in &self.entries {
            let key = key.as_ref().unwrap();
            let k = FileVisibilityMap::key(key);
            if let Some((existing, existing_value)) = entries.get(&k) {
                panic!(
                    "Multiple entries with same key: {}={:?} and {}={:?}",
                    existing, existing_value, key, value
                );
            }
            entries.insert(k, (Arc::clone(key), *value));
        }
        FileVisibilityMap { entries }
    }
}

// port: CollectFileOverviewVisibility
pub struct CollectFileOverviewVisibility {
    builder: FileVisibilityMapBuilder,
}

impl CollectFileOverviewVisibility {
    // port: CollectFileOverviewVisibility#CollectFileOverviewVisibility
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            builder: FileVisibilityMapBuilder::default(),
        }
    }

    // port: CollectFileOverviewVisibility#visit
    fn visit(&mut self, compiler: &AbstractCompiler, script_node: NodeId) {
        let Some(js_doc_info) = script_node.get_jsdoc_info(compiler) else {
            return;
        };
        // Java's `if (v == null) return;` cannot trigger: getVisibility never returns null.
        let v = js_doc_info.get_visibility();
        self.builder
            .put(script_node.get_static_source_file(compiler), v);
    }

    // port: CollectFileOverviewVisibility#getFileOverviewVisibilityMap
    pub fn get_file_overview_visibility_map(&self) -> FileVisibilityMap {
        self.builder.build_or_throw()
    }
}

impl CompilerPass for CollectFileOverviewVisibility {
    // port: CollectFileOverviewVisibility#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            check_state!(s.is_script(compiler));
            self.visit(compiler, s);
            script = s.get_next(compiler);
        }
    }
}
