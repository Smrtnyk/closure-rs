/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/BlackHoleErrorManager.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/CompilerTestCaseUtils.java.

//! The real Compiler API import surface used by the native harness.
pub use closure_jscomp::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    black_hole_error_manager::BlackHoleErrorManager,
    check_level::CheckLevel,
    compiler::Compiler,
    compiler_options::*,
    compiler_pass::CompilerPass,
    compose_warnings_guard::ComposeWarningsGuard,
    custom_pass_execution_time::CustomPassExecutionTime,
    diagnostic_group::DiagnosticGroup,
    diagnostic_group_warnings_guard::DiagnosticGroupWarningsGuard,
    diagnostic_type::DiagnosticType,
    error_manager::ErrorManager,
    js_chunk::JSChunk,
    js_chunk_graph::JSChunkGraph,
    js_error::JSError,
    lightweight_message_formatter::LightweightMessageFormatter,
    message_formatter::MessageFormatter,
    pass_factory::PassFactory,
    region::Region,
    result::Result as CompilationResult,
    source_excerpt_provider::SourceExcerptProvider,
    source_file::SourceFile,
    sourcemap_mapping_placeholder::OriginalMapping,
    warnings_guard::WarningsGuard,
};
pub use closure_rhino::static_source_file::{SourceKind, StaticSourceFile};
pub type ErrorManagerHandle = Box<dyn ErrorManager>;
// port: BlackHoleErrorManager#BlackHoleErrorManager
pub fn black_hole_error_manager() -> ErrorManagerHandle {
    Box::new(BlackHoleErrorManager::new())
}

// The Java fixture installs and resets the Compiler's actual RecentChange handler.
pub trait CompilerHarnessAccess {
    // port: CompilerTestCase#testInternal (getChangeTracker().getRecentChange().reset())
    fn reset_recent_change(&mut self);
    // port: CompilerTestCase#testInternal (getChangeTracker().getRecentChange().hasCodeChanged())
    fn has_code_changed(&self) -> bool;
    // port: CompilerTestCase#testInternal (addChangeHandler)
    fn add_recent_change_handler(&mut self);
    // port: CompilerTestCaseUtils#multistageSerializeAndDeserialize (removeChangeHandler)
    fn remove_recent_change_handler(&mut self);
}
impl CompilerHarnessAccess for Compiler {
    // port: CompilerTestCase#testInternal
    fn reset_recent_change(&mut self) {
        self.get_change_tracker()
            .get_recent_change()
            .lock()
            .unwrap()
            .reset();
    }
    // port: CompilerTestCase#testInternal
    fn has_code_changed(&self) -> bool {
        self.get_change_tracker_ref()
            .get_recent_change()
            .lock()
            .unwrap()
            .has_code_changed()
    }
    // port: CompilerTestCase#testInternal
    fn add_recent_change_handler(&mut self) {
        let recent = self.get_change_tracker().get_recent_change();
        self.get_change_tracker().add_change_handler(recent);
    }
    // port: CompilerTestCaseUtils#multistageSerializeAndDeserialize
    fn remove_recent_change_handler(&mut self) {
        let recent: std::sync::Arc<
            std::sync::Mutex<dyn closure_jscomp::code_change_handler::CodeChangeHandler>,
        > = self.get_change_tracker().get_recent_change();
        self.get_change_tracker().remove_change_handler(&recent);
    }
}

// Missing compiler-owned operations retain their Java signatures behind this surface.
pub use crate::harness_passes::{TypeCheck, new_semantic_reverse_abstract_interpreter};
pub use crate::stand_in::polymer_pass::*;
