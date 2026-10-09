/*
 * Copyright 2024 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ClosureUnawareCodeWarningsGuard.java.

//! Port of `ClosureUnawareCodeWarningsGuard.java`.
use crate::{
    check_level::CheckLevel, diagnostic_group::DiagnosticGroup, js_error::JSError,
    rhino_error_reporter as RhinoErrorReporter, warnings_guard::Priority,
};
use closure_rhino::node::{Ast, NodeId};
use std::{fmt, sync::LazyLock};

// port: ClosureUnawareCodeWarningsGuard#DEFAULT_CLOSURE_UNAWARE_CODE_SUPPRESSIONS
static DEFAULT_CLOSURE_UNAWARE_CODE_SUPPRESSIONS: LazyLock<DiagnosticGroup> = LazyLock::new(|| {
    DiagnosticGroup::new_named(
        "closureUnawareCodeJSDocIncompatible",
        &[
            &RhinoErrorReporter::TYPE_PARSE_ERROR,
            &RhinoErrorReporter::UNRECOGNIZED_TYPE_ERROR,
            &RhinoErrorReporter::JSDOC_MISSING_BRACES_WARNING,
            &RhinoErrorReporter::JSDOC_MISSING_TYPE_WARNING,
            &RhinoErrorReporter::UNNECESSARY_ESCAPE,
            &RhinoErrorReporter::BAD_JSDOC_ANNOTATION,
            &RhinoErrorReporter::UNSUPPORTED_BOUNDED_GENERIC_TYPES,
            &RhinoErrorReporter::BOUNDED_GENERIC_TYPE_ERROR,
        ],
    )
});

/// A warnings guard that suppresses warnings that are spurious for code that is unaware of
/// Closure compiler's requirements.
///
/// Java's `level(JSError)` reads `error.node().isClosureUnawareCode()`; nodes live in the
/// compiler's arena, so this contextual guard takes the arena at evaluation time
/// (`CompilerWarningsGuard` supplies it), like `SuppressDocWarningsGuard`.
#[derive(Debug, Default)]
pub struct ClosureUnawareCodeWarningsGuard;

impl ClosureUnawareCodeWarningsGuard {
    pub fn new() -> Self {
        Self
    }

    // port: ClosureUnawareCodeWarningsGuard#level
    pub fn level(&self, ast: &Ast, error: &JSError) -> Option<CheckLevel> {
        if !DEFAULT_CLOSURE_UNAWARE_CODE_SUPPRESSIONS.matches(error) {
            return None;
        }

        let node: Option<NodeId> = error.node();
        let node = node?;

        if node.is_closure_unaware_code(ast) {
            return Some(CheckLevel::OFF);
        }
        None
    }

    // port: ClosureUnawareCodeWarningsGuard#getPriority
    pub fn get_priority(&self) -> i32 {
        Priority::MAX.get_value()
    }
}

impl fmt::Display for ClosureUnawareCodeWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.ClosureUnawareCodeWarningsGuard@{:x}",
            self as *const Self as usize
        )
    }
}
