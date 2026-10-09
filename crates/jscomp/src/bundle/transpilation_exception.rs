/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/bundle/TranspilationException.java.

use crate::{
    error_format::ErrorFormat, js_error::JSError, source_excerpt_provider::SourceExcerptProvider,
};
use closure_rhino::node::Ast;
use std::{error::Error, fmt, sync::Arc};

/// An unchecked exception thrown when transpilation fails due to one or
/// more errors in the input script.
///
/// Java's `RuntimeException` is thrown with `std::panic::panic_any`; callers that catch it
/// downcast the panic payload to `TranspilationException`.
#[derive(Clone)]
pub struct TranspilationException {
    errors: Vec<JSError>,
    warnings: Vec<JSError>,
    message: String,
    cause: Option<Arc<dyn Error + Send + Sync>>,
}

impl TranspilationException {
    // port: TranspilationException#TranspilationException(Exception)
    pub fn from_cause(cause: Arc<dyn Error + Send + Sync>) -> Self {
        let root = Self::try_cast_to_transpilation_exception(cause.source());
        Self::from_root(
            root.expect("NullPointerException: root.errors").clone(),
            cause,
        )
    }

    /// `ast` is Rust-only: the formatter reads source positions from the arena of the compiler
    /// that is the `source`.
    // port: TranspilationException#TranspilationException(SourceExcerptProvider, ImmutableList, ImmutableList)
    pub fn new(
        source: Option<Arc<dyn SourceExcerptProvider>>,
        ast: &Ast,
        errors: Vec<JSError>,
        warnings: Vec<JSError>,
    ) -> Self {
        let formatted = Self::format(source, ast, &errors, &warnings);
        Self::new_with_message(errors, warnings, formatted, None)
    }

    // port: TranspilationException#TranspilationException(TranspilationException, Exception)
    fn from_root(root: TranspilationException, cause: Arc<dyn Error + Send + Sync>) -> Self {
        Self::new_with_message(root.errors, root.warnings, root.message, Some(cause))
    }

    // port: TranspilationException#TranspilationException(ImmutableList, ImmutableList, String, Exception)
    fn new_with_message(
        errors: Vec<JSError>,
        warnings: Vec<JSError>,
        formatted: String,
        cause: Option<Arc<dyn Error + Send + Sync>>,
    ) -> Self {
        Self {
            errors,
            warnings,
            message: formatted,
            cause,
        }
    }

    // port: TranspilationException#errors
    pub fn errors(&self) -> &[JSError] {
        &self.errors
    }

    // port: TranspilationException#warnings
    pub fn warnings(&self) -> &[JSError] {
        &self.warnings
    }

    /// `Throwable#getMessage`.
    pub fn get_message(&self) -> &str {
        &self.message
    }

    // port: TranspilationException#format
    fn format(
        source: Option<Arc<dyn SourceExcerptProvider>>,
        ast: &Ast,
        errors: &[JSError],
        warnings: &[JSError],
    ) -> String {
        let mut sb = String::from("Transpilation failed:\n");
        let formatter = if source.is_some() {
            ErrorFormat::SINGLELINE.to_formatter(source, false)
        } else {
            ErrorFormat::SOURCELESS.to_formatter(source, false)
        };
        for error in errors {
            sb.push('\n');
            sb.push_str(&formatter.format_error(ast, error));
        }
        for warning in warnings {
            sb.push('\n');
            sb.push_str(&formatter.format_error(ast, warning));
        }
        sb
    }

    // port: TranspilationException#tryCastToTranspilationException
    fn try_cast_to_transpilation_exception<'a>(
        t: Option<&'a (dyn Error + 'static)>,
    ) -> Option<&'a TranspilationException> {
        t.and_then(|t| t.downcast_ref::<TranspilationException>())
    }
}

impl fmt::Debug for TranspilationException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.bundle.TranspilationException: {}",
            self.message
        )
    }
}

impl fmt::Display for TranspilationException {
    // port: Throwable#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Error for TranspilationException {
    // port: Throwable#getCause
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}
