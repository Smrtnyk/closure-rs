/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Compiler.java.

#![allow(clippy::mutable_key_type)]
use crate::{code_printer::LicenseTracker, compiler::Compiler, js_chunk::JSChunk};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::sync::{Arc, Mutex};
type Scripts = Arc<Mutex<IndexMap<String, NodeId>>>;
struct SeenState {
    globally_unique_licenses: IndexSet<JsString>,
    currently_seen_licenses: IndexSet<JsString>,
    last_seen_file: String,
}
struct SeenSetLicenseTracker {
    scripts: Scripts,
    state: Mutex<SeenState>,
    scripts_only: bool,
}
impl SeenSetLicenseTracker {
    // port: Compiler.SeenSetLicenseTracker#SeenSetLicenseTracker
    fn new(compiler: &Compiler, scripts_only: bool) -> Self {
        Self {
            scripts: compiler.get_shared_scripts(),
            state: Mutex::new(SeenState {
                globally_unique_licenses: IndexSet::<_>::default(),
                currently_seen_licenses: IndexSet::<_>::default(),
                last_seen_file: String::new(),
            }),
            scripts_only,
        }
    }
    // port: Compiler.SeenSetLicenseTracker#shouldUseLicenseInfo
    fn should_use_license_info(&self, ast: &Ast, node: NodeId) -> bool {
        if self.scripts_only {
            node.is_root(ast) || node.is_script(ast)
        } else {
            !node.is_root(ast) && !node.is_script(ast)
        }
    }
}
impl LicenseTracker for SeenSetLicenseTracker {
    // port: Compiler.SeenSetLicenseTracker#trackLicensesForNode
    fn track_licenses_for_node(&mut self, ast: &Ast, node: NodeId) {
        if !self.should_use_license_info(ast, node) {
            return;
        }
        let Some(file) = node.get_static_source_file_ref(ast) else {
            return;
        };
        let mut state = self.state.lock().unwrap();
        // The name is copied only when it changes (most nodes are in the last seen file).
        if file.get_name() == state.last_seen_file {
            return;
        }
        let file: String = file.get_name().into();
        state.last_seen_file = file.clone();
        let Some(license) = license_for_file(ast, &self.scripts, &file) else {
            return;
        };
        if state.globally_unique_licenses.contains(&license) {
            return;
        }
        state.currently_seen_licenses.insert(license);
    }
    // port: Compiler.SeenSetLicenseTracker#emitLicenses
    fn emit_licenses(&self) -> IndexSet<JsString> {
        let mut state = self.state.lock().unwrap();
        let result = state.currently_seen_licenses.clone();
        state
            .globally_unique_licenses
            .extend(result.iter().cloned());
        state.currently_seen_licenses.clear();
        result
    }
}
fn license_for_file(ast: &Ast, scripts: &Scripts, file: &str) -> Option<JsString> {
    scripts
        .lock()
        .unwrap()
        .get(file)?
        .get_jsdoc_info(ast)?
        .get_license()
}
pub struct ScriptNodeLicensesOnlyTracker(SeenSetLicenseTracker);
impl ScriptNodeLicensesOnlyTracker {
    // port: Compiler.ScriptNodeLicensesOnlyTracker#ScriptNodeLicensesOnlyTracker
    pub fn new(compiler: &Compiler) -> Self {
        Self(SeenSetLicenseTracker::new(compiler, true))
    }
    // port: Compiler.ScriptNodeLicensesOnlyTracker#shouldUseLicenseInfo
    pub fn should_use_license_info(&self, ast: &Ast, node: NodeId) -> bool {
        node.is_root(ast) || node.is_script(ast)
    }
}
pub struct SingleBinaryLicenseTracker(SeenSetLicenseTracker);
impl SingleBinaryLicenseTracker {
    // port: Compiler.SingleBinaryLicenseTracker#SingleBinaryLicenseTracker
    pub fn new(compiler: &Compiler) -> Self {
        Self(SeenSetLicenseTracker::new(compiler, false))
    }
    // port: Compiler.SingleBinaryLicenseTracker#shouldUseLicenseInfo
    pub fn should_use_license_info(&self, ast: &Ast, node: NodeId) -> bool {
        !node.is_root(ast) && !node.is_script(ast)
    }
}
macro_rules! delegate {
    ($ty:ty) => {
        impl LicenseTracker for $ty {
            fn track_licenses_for_node(&mut self, ast: &Ast, node: NodeId) {
                self.0.track_licenses_for_node(ast, node);
            }
            fn emit_licenses(&self) -> IndexSet<JsString> {
                self.0.emit_licenses()
            }
        }
    };
}
delegate!(ScriptNodeLicensesOnlyTracker);
delegate!(SingleBinaryLicenseTracker);
struct ChunkState {
    licenses_from_chunks: IndexMap<JSChunk, IndexSet<JsString>>,
    have_initialized_current_chunk_licenses: bool,
    current_chunk_licences_in_t_deps: IndexSet<JsString>,
    last_seen_file: String,
    licenses_new_in_current_file: IndexSet<JsString>,
    current_chunk: Option<JSChunk>,
}
pub struct ChunkGraphAwareLicenseTracker {
    log: Mutex<Box<dyn crate::diagnostic::log_file::LogFile>>,
    scripts: Scripts,
    state: Mutex<ChunkState>,
}
impl ChunkGraphAwareLicenseTracker {
    // port: Compiler.ChunkGraphAwareLicenseTracker#ChunkGraphAwareLicenseTracker
    pub fn new(compiler: &Compiler) -> Self {
        Self {
            scripts: compiler.get_shared_scripts(),
            log: Mutex::new(crate::diagnostic::log_file::create_no_op()),
            state: Mutex::new(ChunkState {
                licenses_from_chunks: IndexMap::<_, _>::default(),
                have_initialized_current_chunk_licenses: false,
                current_chunk_licences_in_t_deps: IndexSet::<_>::default(),
                last_seen_file: String::new(),
                licenses_new_in_current_file: IndexSet::<_>::default(),
                current_chunk: None,
            }),
        }
    }
    // port: Compiler.ChunkGraphAwareLicenseTracker#setLogFile
    pub fn set_log_file(&mut self, file: Box<dyn crate::diagnostic::log_file::LogFile>) {
        self.log = Mutex::new(file);
    }
    // port: Compiler.ChunkGraphAwareLicenseTracker#setCurrentChunkContext
    pub fn set_current_chunk_context(&mut self, chunk: JSChunk) {
        self.log
            .lock()
            .unwrap()
            .log(&mut || format!("Initializing licenses from deps for chunk {chunk}"));
        let mut state = self.state.lock().unwrap();
        state.current_chunk = Some(chunk.clone());
        state.current_chunk_licences_in_t_deps.clear();
        state.licenses_new_in_current_file.clear();
        state.have_initialized_current_chunk_licenses = false;
        assert!(
            !state.licenses_from_chunks.contains_key(&chunk),
            "Visiting a chunk more than once is not allowed."
        );
        state
            .licenses_from_chunks
            .insert(chunk, IndexSet::<_>::default());
    }
}
impl LicenseTracker for ChunkGraphAwareLicenseTracker {
    // port: Compiler.ChunkGraphAwareLicenseTracker#trackLicensesForNode
    fn track_licenses_for_node(&mut self, ast: &Ast, node: NodeId) {
        if node.is_root(ast) || node.is_script(ast) {
            return;
        }
        let Some(source_file) = node.get_static_source_file_ref(ast) else {
            return;
        };
        let mut state = self.state.lock().unwrap();
        // The name is copied only when it changes (most nodes are in the last seen file).
        if state.last_seen_file == source_file.get_name() {
            return;
        }
        let source_file: String = source_file.get_name().into();
        state.last_seen_file = source_file.clone();
        if !state.have_initialized_current_chunk_licenses {
            for dep in state.current_chunk.as_ref().unwrap().get_all_dependencies() {
                self.log.lock().unwrap().log(&mut || {
                    format!(
                        "Chunk {} depends on chunk {dep}",
                        state.current_chunk.as_ref().unwrap()
                    )
                });
                let licenses = state
                    .licenses_from_chunks
                    .get(&dep)
                    .expect("Chunk aware license analysis error - analysis out of order.")
                    .clone();
                state.current_chunk_licences_in_t_deps.extend(licenses);
            }
            state.have_initialized_current_chunk_licenses = true;
        }
        let Some(license) = license_for_file(ast, &self.scripts, &source_file) else {
            return;
        };
        if state.current_chunk_licences_in_t_deps.contains(&license) {
            return;
        }
        if state
            .licenses_from_chunks
            .get(state.current_chunk.as_ref().unwrap())
            .unwrap()
            .contains(&license)
        {
            return;
        }
        let newly_added_license = state.licenses_new_in_current_file.insert(license.clone());
        self.log.lock().unwrap().log(&mut ||if newly_added_license {format!("Chunk {} has new license:\n==================\n{license}\n==================\n\n",state.current_chunk.as_ref().unwrap())} else {format!("Chunk {} already depends on license\n==================\n{license}\n==================\n\n",state.current_chunk.as_ref().unwrap())});
    }
    // port: Compiler.ChunkGraphAwareLicenseTracker#emitLicenses
    fn emit_licenses(&self) -> IndexSet<JsString> {
        let mut state = self.state.lock().unwrap();
        let result = state.licenses_new_in_current_file.clone();
        let chunk = state.current_chunk.as_ref().unwrap().clone();
        state
            .licenses_from_chunks
            .get_mut(&chunk)
            .unwrap()
            .extend(result.iter().cloned());
        state.licenses_new_in_current_file.clear();
        result
    }
}
