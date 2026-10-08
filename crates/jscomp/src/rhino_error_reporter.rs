/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RhinoErrorReporter.java.

use crate::{
    check_level::CheckLevel, diagnostic_type::DiagnosticType, error_handler::ErrorHandler,
    js_error::JSError,
};
use closure_rhino::{error_reporter::ErrorReporter, java_lang::regex::Pattern, msg::Msg};
use std::sync::LazyLock;
// port: RhinoErrorReporter#PARSE_ERROR
pub static PARSE_ERROR: DiagnosticType =
    DiagnosticType::error("JSC_PARSE_ERROR", "Parse error. {0}");
// port: RhinoErrorReporter#TYPE_PARSE_ERROR
pub static TYPE_PARSE_ERROR: DiagnosticType =
    DiagnosticType::warning("JSC_TYPE_PARSE_ERROR", "{0}");
// port: RhinoErrorReporter#UNRECOGNIZED_TYPE_ERROR
pub static UNRECOGNIZED_TYPE_ERROR: DiagnosticType =
    DiagnosticType::warning("JSC_UNRECOGNIZED_TYPE_ERROR", "{0}");
// port: RhinoErrorReporter#UNRECOGNIZED_TYPEOF_ERROR
pub static UNRECOGNIZED_TYPEOF_ERROR: DiagnosticType =
    DiagnosticType::warning("JSC_UNRECOGNIZED_TYPEOF_ERROR", "{0}");
// port: RhinoErrorReporter#CYCLIC_INHERITANCE_ERROR
pub static CYCLIC_INHERITANCE_ERROR: DiagnosticType =
    DiagnosticType::warning("JSC_CYCLIC_INHERITANCE_ERROR", "{0}");
// port: RhinoErrorReporter#JSDOC_MISSING_BRACES_WARNING
pub static JSDOC_MISSING_BRACES_WARNING: DiagnosticType =
    DiagnosticType::disabled("JSC_JSDOC_MISSING_BRACES_WARNING", "{0}");
// port: RhinoErrorReporter#JSDOC_MISSING_TYPE_WARNING
pub static JSDOC_MISSING_TYPE_WARNING: DiagnosticType =
    DiagnosticType::disabled("JSC_JSDOC_MISSING_TYPE_WARNING", "{0}");
// port: RhinoErrorReporter#JSDOC_IMPORT_TYPE_WARNING
pub static JSDOC_IMPORT_TYPE_WARNING: DiagnosticType =
    DiagnosticType::disabled("JSC_JSDOC_IMPORT_TYPE_WARNING", "{0}");
// port: RhinoErrorReporter#TOO_MANY_TEMPLATE_PARAMS
pub static TOO_MANY_TEMPLATE_PARAMS: DiagnosticType =
    DiagnosticType::warning("JSC_TOO_MANY_TEMPLATE_PARAMS", "{0}");
// port: RhinoErrorReporter#TRAILING_COMMA
pub static TRAILING_COMMA: DiagnosticType = DiagnosticType::error(
    "JSC_TRAILING_COMMA",
    "Parse error. IE8 (and below) will parse trailing commas in array and object literals incorrectly. If you are targeting newer versions of JS, set the appropriate language_in option.",
);
// port: RhinoErrorReporter#DUPLICATE_PARAM
pub static DUPLICATE_PARAM: DiagnosticType =
    DiagnosticType::error("JSC_DUPLICATE_PARAM", "Parse error. {0}");
// port: RhinoErrorReporter#DUPLICATE_VISIBILITY
pub static DUPLICATE_VISIBILITY: DiagnosticType =
    DiagnosticType::warning("JSC_DUPLICATE_VISIBILITY", "{0}");
// port: RhinoErrorReporter#UNNECESSARY_ESCAPE
pub static UNNECESSARY_ESCAPE: DiagnosticType =
    DiagnosticType::disabled("JSC_UNNECESSARY_ESCAPE", "Parse error. {0}");
// port: RhinoErrorReporter#INVALID_PARAM
pub static INVALID_PARAM: DiagnosticType =
    DiagnosticType::warning("JSC_INVALID_PARAM", "Parse error. {0}");
// port: RhinoErrorReporter#BAD_JSDOC_ANNOTATION
pub static BAD_JSDOC_ANNOTATION: DiagnosticType =
    DiagnosticType::warning("JSC_BAD_JSDOC_ANNOTATION", "Parse error. {0}");
// port: RhinoErrorReporter#INVALID_ES3_PROP_NAME
pub static INVALID_ES3_PROP_NAME: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_ES3_PROP_NAME",
    "Keywords and reserved words are not allowed as unquoted property names in older versions of JavaScript. If you are targeting newer versions of JavaScript, set the appropriate language_in option.",
);
// port: RhinoErrorReporter#PARSE_TREE_TOO_DEEP
pub static PARSE_TREE_TOO_DEEP: DiagnosticType =
    DiagnosticType::error("JSC_PARSE_TREE_TOO_DEEP", "Parse tree too deep.");
// port: RhinoErrorReporter#INVALID_OCTAL_LITERAL
pub static INVALID_OCTAL_LITERAL: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_OCTAL_LITERAL",
    "This style of octal literal is not supported in strict mode.",
);
// port: RhinoErrorReporter#STRING_CONTINUATION
pub static STRING_CONTINUATION: DiagnosticType =
    DiagnosticType::disabled("JSC_STRING_CONTINUATION", "{0}");
// port: RhinoErrorReporter#LANGUAGE_FEATURE
pub static LANGUAGE_FEATURE: DiagnosticType = DiagnosticType::error("JSC_LANGUAGE_FEATURE", "{0}.");
// port: RhinoErrorReporter#UNSUPPORTED_LANGUAGE_FEATURE
pub static UNSUPPORTED_LANGUAGE_FEATURE: DiagnosticType =
    DiagnosticType::error("JSC_UNSUPPORTED_LANGUAGE_FEATURE", "{0}.");
// port: RhinoErrorReporter#UNSUPPORTED_BOUNDED_GENERIC_TYPES
pub static UNSUPPORTED_BOUNDED_GENERIC_TYPES: DiagnosticType = DiagnosticType::error(
    "JSC_UNSUPPORTED_BOUNDED_GENERIC_TYPES",
    "Bounded generic semantics are currently still in development",
);
// port: RhinoErrorReporter#BOUNDED_GENERIC_TYPE_ERROR
pub static BOUNDED_GENERIC_TYPE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_BOUNDED_GENERIC_TYPE_ERROR",
    "Bounded generic type error. {0} assigned to template type {1} is not a subtype of bound {2}",
);
// port: RhinoErrorReporter#CLOSURE_UNAWARE_ANNOTATION_PRESENT
pub static CLOSURE_UNAWARE_ANNOTATION_PRESENT: LazyLock<DiagnosticType> = LazyLock::new(|| {
    DiagnosticType::disabled(
        "JSC_CLOSURE_UNAWARE_ANNOTATION_PRESENT",
        Msg::JSDOC_CLOSURE_UNAWARE_CODE_INVALID.format(),
    )
});
static TYPE_MAP: LazyLock<Vec<(Pattern, &'static DiagnosticType)>> = LazyLock::new(|| {
    vec![
        (
            Pattern::compile("Trailing comma is not legal in an ECMA-262 object initializer"),
            &TRAILING_COMMA,
        ),
        (
            RhinoErrorReporter::replace_place_holders("Duplicate parameter name \"{0}\""),
            &DUPLICATE_PARAM,
        ),
        (
            RhinoErrorReporter::replace_place_holders(Msg::JSDOC_EXTRA_VISIBILITY.format()),
            &DUPLICATE_VISIBILITY,
        ),
        (
            Pattern::compile("Unnecessary escape:.*"),
            &UNNECESSARY_ESCAPE,
        ),
        (Pattern::compile("^invalid param name.*"), &INVALID_PARAM),
        (
            RhinoErrorReporter::replace_place_holders(Msg::BAD_JSDOC_TAG.format()),
            &BAD_JSDOC_ANNOTATION,
        ),
        (
            Pattern::compile("^Keywords and reserved words are not allowed as unquoted property.*"),
            &INVALID_ES3_PROP_NAME,
        ),
        (
            Pattern::compile("^Too many template parameters\n.*"),
            &TOO_MANY_TEMPLATE_PARAMS,
        ),
        (
            Pattern::compile(".*Type annotations should have curly braces.*"),
            &JSDOC_MISSING_BRACES_WARNING,
        ),
        (
            Pattern::compile("Missing type declaration\\."),
            &JSDOC_MISSING_TYPE_WARNING,
        ),
        (
            Pattern::compile(".*Unknown type.*"),
            &UNRECOGNIZED_TYPE_ERROR,
        ),
        (
            Pattern::compile(".*Unknown type.*\n.*"),
            &UNRECOGNIZED_TYPE_ERROR,
        ),
        (
            Pattern::compile("^Missing type for `typeof` value.*"),
            &UNRECOGNIZED_TYPEOF_ERROR,
        ),
        (
            Pattern::compile("^Cycle detected in inheritance chain of type .*"),
            &CYCLIC_INHERITANCE_ERROR,
        ),
        (
            Pattern::compile("^Bad type annotation. Import in typedef.*"),
            &JSDOC_IMPORT_TYPE_WARNING,
        ),
        (
            Pattern::compile("^Bad type annotation.*"),
            &TYPE_PARSE_ERROR,
        ),
        (
            Pattern::compile("constructed type must be an object type"),
            &TYPE_PARSE_ERROR,
        ),
        (
            Pattern::compile("Too deep recursion while parsing"),
            &PARSE_TREE_TOO_DEEP,
        ),
        (
            Pattern::compile("^Octal .*literal.*"),
            &INVALID_OCTAL_LITERAL,
        ),
        (
            Pattern::compile("^String continuations.*"),
            &STRING_CONTINUATION,
        ),
        (
            Pattern::compile("^This language feature is only supported for .*"),
            &LANGUAGE_FEATURE,
        ),
        (
            Pattern::compile(
                "^This language feature is not currently supported by the compiler: .*",
            ),
            &UNSUPPORTED_LANGUAGE_FEATURE,
        ),
        (
            Pattern::compile("Bounded generic semantics are currently still in development"),
            &UNSUPPORTED_BOUNDED_GENERIC_TYPES,
        ),
        (
            Pattern::compile("^Bounded generic type error.*"),
            &BOUNDED_GENERIC_TYPE_ERROR,
        ),
        (
            RhinoErrorReporter::replace_place_holders(
                Msg::JSDOC_CLOSURE_UNAWARE_CODE_INVALID.format(),
            ),
            &CLOSURE_UNAWARE_ANNOTATION_PRESENT,
        ),
    ]
});
pub struct RhinoErrorReporter<'a> {
    internal_reporter: &'a mut dyn ErrorHandler,
}
impl<'a> RhinoErrorReporter<'a> {
    // port: RhinoErrorReporter#replacePlaceHolders
    pub fn replace_place_holders(s: &str) -> Pattern {
        // Pattern.quote escapes embedded \E before the Java placeholder substitution.
        let quoted = format!("\\Q{}\\E", s.replace("\\E", "\\E\\\\E\\Q"));
        let mut out = String::new();
        let chars: Vec<char> = quoted.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 1 && j < chars.len() && chars[j] == '}' {
                    out.push_str("\\E.*\\Q");
                    i = j + 1;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        // The deps branch's JDK matcher has no \Q...\E parser entry point. Expand Java's
        // quoting into escaped literals before compiling, preserving the same expression.
        let chars: Vec<_> = out.chars().collect();
        let mut pattern = String::new();
        let mut quoted = false;
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\\'
                && i + 1 < chars.len()
                && ((!quoted && chars[i + 1] == 'Q') || (quoted && chars[i + 1] == 'E'))
            {
                quoted = !quoted;
                i += 2;
                continue;
            }
            let c = chars[i];
            if quoted && "\\.^$?*+()[]{}|".contains(c) {
                pattern.push('\\');
            }
            pattern.push(c);
            i += 1;
        }
        Pattern::compile(&pattern)
    }
    // port: RhinoErrorReporter#RhinoErrorReporter
    pub fn new(internal_reporter: &'a mut dyn ErrorHandler) -> Self {
        Self { internal_reporter }
    }
    // port: RhinoErrorReporter#forOldRhino
    pub fn for_old_rhino(internal_reporter: &'a mut dyn ErrorHandler) -> OldRhinoErrorReporter<'a> {
        OldRhinoErrorReporter::new(internal_reporter)
    }
    // port: RhinoErrorReporter#warningAtLine
    pub fn warning_at_line(
        &mut self,
        message: &str,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.internal_reporter.report(
            CheckLevel::WARNING,
            Self::make_error(message, source_name, line, line_offset, CheckLevel::WARNING),
        );
    }
    // port: RhinoErrorReporter#errorAtLine
    pub fn error_at_line(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.internal_reporter.report(
            CheckLevel::ERROR,
            Self::make_error(message, source_name, line, line_offset, CheckLevel::ERROR),
        );
    }
    // port: RhinoErrorReporter#mapError
    pub fn map_error(message: &str) -> Option<&'static DiagnosticType> {
        TYPE_MAP
            .iter()
            .find_map(|(pattern, ty)| pattern.matcher(message).matches().then_some(*ty))
    }
    // port: RhinoErrorReporter#makeError
    pub fn make_error(
        message: &str,
        source_name: &str,
        line: i32,
        line_offset: i32,
        default_level: CheckLevel,
    ) -> JSError {
        let type_ = Self::map_error(message);
        let mut builder = JSError::builder(type_.unwrap_or(&PARSE_ERROR), &[message])
            .set_source_location(source_name, line, line_offset);
        if type_.is_none() {
            builder = builder.set_level(default_level);
        }
        builder.build()
    }
}
pub struct OldRhinoErrorReporter<'a> {
    reporter: RhinoErrorReporter<'a>,
}
impl<'a> OldRhinoErrorReporter<'a> {
    // port: RhinoErrorReporter.OldRhinoErrorReporter#OldRhinoErrorReporter
    fn new(internal_reporter: &'a mut dyn ErrorHandler) -> Self {
        Self {
            reporter: RhinoErrorReporter::new(internal_reporter),
        }
    }
}
impl ErrorReporter for OldRhinoErrorReporter<'_> {
    // port: RhinoErrorReporter.OldRhinoErrorReporter#error (WTF-16 adapter)
    fn error_js_string(
        &mut self,
        message: &closure_rhino::js_string::JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        // Diagnostic messages use String in the diagnostics API. Its UTF-8
        // boundary follows Java's replacement of each unpaired surrogate by '?'.
        self.error(
            &closure_rhino::java_lang::charset::utf8_encoded_text(message.as_units()),
            source_name,
            line,
            line_offset,
        );
    }
    // port: RhinoErrorReporter.OldRhinoErrorReporter#warning (WTF-16 adapter)
    fn warning_js_string(
        &mut self,
        message: &closure_rhino::js_string::JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.warning(
            &closure_rhino::java_lang::charset::utf8_encoded_text(message.as_units()),
            source_name,
            line,
            line_offset,
        );
    }
    // port: RhinoErrorReporter.OldRhinoErrorReporter#error
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.reporter
            .error_at_line(message, source_name, line, line_offset);
    }
    // port: RhinoErrorReporter.OldRhinoErrorReporter#warning
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.reporter
            .warning_at_line(message, source_name, line, line_offset);
    }
}
/// Rust-only: the `OldRhinoErrorReporter` the Compiler hands its `JSTypeRegistry`
/// (`Compiler#getTypeRegistry`). Java's reporter calls straight back into `Compiler#report`; here the
/// Compiler owns the registry, so the reporter queues the `JSError` that
/// `RhinoErrorReporter#errorAtLine`/`#warningAtLine` build, and the Compiler reports the queue
/// (`Compiler::report_queued_type_registry_errors`) before it next reports or reads errors. The error
/// managers keep errors sorted (`SortingErrorManager`), so the deferral does not change any output.
pub struct QueueingOldRhinoErrorReporter {
    queue: std::sync::Arc<std::sync::Mutex<Vec<JSError>>>,
}
impl QueueingOldRhinoErrorReporter {
    pub fn new(queue: std::sync::Arc<std::sync::Mutex<Vec<JSError>>>) -> Self {
        Self { queue }
    }
}
impl ErrorReporter for QueueingOldRhinoErrorReporter {
    // port: RhinoErrorReporter.OldRhinoErrorReporter#error (WTF-16 adapter)
    fn error_js_string(
        &mut self,
        message: &closure_rhino::js_string::JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.error(
            &closure_rhino::java_lang::charset::utf8_encoded_text(message.as_units()),
            source_name,
            line,
            line_offset,
        );
    }
    // port: RhinoErrorReporter.OldRhinoErrorReporter#warning (WTF-16 adapter)
    fn warning_js_string(
        &mut self,
        message: &closure_rhino::js_string::JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.warning(
            &closure_rhino::java_lang::charset::utf8_encoded_text(message.as_units()),
            source_name,
            line,
            line_offset,
        );
    }
    // port: RhinoErrorReporter.OldRhinoErrorReporter#error
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        // RhinoErrorReporter#errorAtLine
        self.queue
            .lock()
            .unwrap()
            .push(RhinoErrorReporter::make_error(
                message,
                source_name,
                line,
                line_offset,
                CheckLevel::ERROR,
            ));
    }
    // port: RhinoErrorReporter.OldRhinoErrorReporter#warning
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        // RhinoErrorReporter#warningAtLine
        self.queue
            .lock()
            .unwrap()
            .push(RhinoErrorReporter::make_error(
                message,
                source_name,
                line,
                line_offset,
                CheckLevel::WARNING,
            ));
    }
}
#[derive(Default)]
pub(crate) struct RecordingErrorHandler {
    pub errors: Vec<JSError>,
}
impl ErrorHandler for RecordingErrorHandler {
    fn report(&mut self, _level: CheckLevel, error: JSError) {
        self.errors.push(error);
    }
}
