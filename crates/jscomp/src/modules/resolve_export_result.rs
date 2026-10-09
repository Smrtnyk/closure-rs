/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/modules/ResolveExportResult.java.

//! The result of resolving an export, which can be a valid binding, ambiguous, not found, or an
//! error.
use crate::modules::binding::{Binding, CreatedBy};
use closure_rhino::{check_not_null, node::NodeId};
use std::sync::{Arc, LazyLock};

// port: ResolveExportResult.State
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    RESOLVED,
    AMBIGUOUS,
    NOT_FOUND,
    ERROR,
}

#[derive(Debug)]
struct ResolveExportResultData {
    binding: Option<Binding>,
    state: State,
}

/// The result of resolving an export, which can be a valid binding, ambiguous, not found, or an
/// error.
///
/// Java compares these objects by reference (`starResolution != resolution` in
/// `EsModuleProcessor`), so the Rust value is a shared handle and [`ResolveExportResult::ptr_eq`]
/// is Java's `==`.
#[derive(Clone, Debug)]
pub struct ResolveExportResult(Arc<ResolveExportResultData>);

/// The result of resolving the export was ambiguous.
///
/// This happens when there are multiple `export * from` statements that end up causing the same
/// key to be re-exported.
// port: ResolveExportResult#AMBIGUOUS
pub static AMBIGUOUS: LazyLock<ResolveExportResult> =
    LazyLock::new(|| ResolveExportResult::new(None, State::AMBIGUOUS));

/// The export was not found because the module never exported the key.
// port: ResolveExportResult#NOT_FOUND
pub static NOT_FOUND: LazyLock<ResolveExportResult> =
    LazyLock::new(|| ResolveExportResult::new(None, State::NOT_FOUND));

/// There was an error resolving the export (a missing or ambiguous transitive export, a cycle, or
/// a missing requested module). Nothing more needs to be reported.
// port: ResolveExportResult#ERROR
pub static ERROR: LazyLock<ResolveExportResult> =
    LazyLock::new(|| ResolveExportResult::new(None, State::ERROR));

impl ResolveExportResult {
    // port: ResolveExportResult#ResolveExportResult
    fn new(binding: Option<Binding>, state: State) -> Self {
        Self(Arc::new(ResolveExportResultData { binding, state }))
    }

    /// Java's reference equality.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Creates a new result that has the given node for the source of the binding and given type
    /// of binding.
    // port: ResolveExportResult#copy
    pub fn copy(&self, source_node: Option<NodeId>, created_by: CreatedBy) -> Self {
        let source_node = check_not_null!(source_node);
        match &self.0.binding {
            None => self.clone(),
            Some(binding) => Self::new(
                Some(binding.copy(Some(source_node), created_by)),
                self.0.state,
            ),
        }
    }

    /// True if there was an error resolving the export, false otherwise.
    // port: ResolveExportResult#hadError
    pub fn had_error(&self) -> bool {
        self.0.state == State::ERROR
    }

    /// True if the export is ambiguous, false otherwise.
    // port: ResolveExportResult#isAmbiguous
    pub fn is_ambiguous(&self) -> bool {
        self.0.state == State::AMBIGUOUS
    }

    /// True if the export was successfully resolved, false otherwise.
    // port: ResolveExportResult#resolved
    pub fn resolved(&self) -> bool {
        self.0.state == State::RESOLVED
    }

    /// True if the export key exists on the given module, even if it is ambiguous or had an error.
    // port: ResolveExportResult#found
    pub fn found(&self) -> bool {
        self.0.state != State::NOT_FOUND
    }

    // port: ResolveExportResult#getBinding
    pub fn get_binding(&self) -> Option<&Binding> {
        self.0.binding.as_ref()
    }

    // port: ResolveExportResult#of
    pub fn of(binding: Binding) -> Self {
        Self::new(Some(binding), State::RESOLVED)
    }

    // port: ResolveExportResult#AMBIGUOUS
    pub fn ambiguous() -> Self {
        AMBIGUOUS.clone()
    }

    // port: ResolveExportResult#NOT_FOUND
    pub fn not_found() -> Self {
        NOT_FOUND.clone()
    }

    // port: ResolveExportResult#ERROR
    pub fn error() -> Self {
        ERROR.clone()
    }
}
