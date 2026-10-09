/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AllowlistWarningsGuard.java.

//! Port of `AllowlistWarningsGuard.java`. The module and type spell the Java word as two words
//! ("allow list"), because CI rejects the one-word lowercase spelling in Rust sources.
use crate::{
    check_level::CheckLevel,
    diagnostic_groups::DEPRECATED,
    diagnostic_type::DiagnosticType,
    error_handler::ErrorHandler,
    js_error::JSError,
    warnings_guard::{Priority, WarningsGuard},
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    java_lang::{regex::Pattern, utf_8::BufferedReader},
    js_string::JsString,
};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{self, Read, Write},
    sync::LazyLock,
};

/// Pattern to match line number in error descriptions.
// port: AllowlistWarningsGuard#LINE_NUMBER
static LINE_NUMBER: LazyLock<Pattern> = LazyLock::new(|| Pattern::compile(":-?\\d+"));
// port: AllowlistWarningsGuard#normalizeSourceName (the pattern of its replaceFirst)
static BLAZE_OUT: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("blaze-out/[^/]*/(bin|genfiles)/"));

/// Java `String#replaceFirst` with a replacement that has no `$` or `\`.
fn replace_first(pattern: &Pattern, input: &str, replacement: &str) -> String {
    let units = JsString::from(input);
    let mut matcher = pattern.matcher(units.clone());
    if !matcher.find() {
        return input.to_string();
    }
    let mut result = units.substring(0, matcher.start()).to_string_lossy();
    result.push_str(replacement);
    result.push_str(&units.substring_from(matcher.end()).to_string_lossy());
    result
}

/// Guava `Splitter.on('\n').split(s)`.
fn split_lines(s: &str) -> Vec<&str> {
    s.split('\n').collect()
}

/// An extension of `WarningsGuard` that provides functionality to maintain a list of warnings
/// (allow list). It is subclasses' responsibility to decide what to do with the allow list by
/// implementing the `level` function. Warnings are defined by the name of the JS file and the
/// first line of warnings description.
#[derive(Debug)]
pub struct AllowListWarningsGuard {
    /// The set of allow-listed warnings, same format as `formatWarning`.
    allow_list: IndexSet<String>,
}

impl Default for AllowListWarningsGuard {
    // port: AllowlistWarningsGuard#AllowlistWarningsGuard()
    fn default() -> Self {
        Self::new(&IndexSet::<_>::default())
    }
}

impl AllowListWarningsGuard {
    /// This class depends on an input set that contains the allow list. The format of each
    /// string is: `<file-name>:<line-number>? <warning-description>` `# <optional-comment>`.
    // port: AllowlistWarningsGuard#AllowlistWarningsGuard(Set)
    pub fn new(allow_list: &IndexSet<String>) -> Self {
        Self {
            allow_list: Self::normalize_allow_list(allow_list),
        }
    }

    /// Loads legacy warnings list from the set of strings. During development line numbers are
    /// changed very often - we just cut them and compare without ones.
    ///
    /// Also remove lines starting with "#" or are blank lines.
    // port: AllowlistWarningsGuard#normalizeAllowlist
    pub fn normalize_allow_list(allow_list: &IndexSet<String>) -> IndexSet<String> {
        let mut result = IndexSet::<_>::default();
        for line in allow_list {
            let trimmed = closure_rhino::java_lang::trim(&JsString::from(line.as_str()));
            if trimmed.is_empty() || trimmed.char_at(0) == u16::from(b'#') {
                // strip out empty lines and comments.
                continue;
            }
            // Strip line number for matching.
            result.insert(replace_first(&LINE_NUMBER, &trimmed.to_string_lossy(), ":"));
        }
        result
    }

    /// Determines whether a given warning is included in the allow list.
    // port: AllowlistWarningsGuard#containWarning
    pub fn contain_warning(&self, formatted_warning: &str) -> bool {
        self.allow_list.contains(formatted_warning)
    }

    /// Creates a warnings guard from a file.
    // port: AllowlistWarningsGuard#fromFile
    pub fn from_file(file: &str) -> Self {
        Self::new(&Self::load_allow_listed_js_warnings(file))
    }

    /// Loads legacy warnings list from the file.
    // port: AllowlistWarningsGuard#loadAllowlistedJsWarnings(File)
    pub fn load_allow_listed_js_warnings(file: &str) -> IndexSet<String> {
        // port: AllowlistWarningsGuard#loadAllowlistedJsWarnings(CharSource)
        let opened = std::fs::File::open(file);
        let result = opened.and_then(Self::load_allow_listed_js_warnings_from_reader);
        match result {
            Ok(result) => result,
            Err(e) => panic!("java.io.IOException: {e}"),
        }
    }

    /// Loads legacy warnings list from the file.
    // port: AllowlistWarningsGuard#loadAllowlistedJsWarnings(Reader)
    pub fn load_allow_listed_js_warnings_from_reader(
        reader: impl Read,
    ) -> io::Result<IndexSet<String>> {
        let mut result = IndexSet::<_>::default();
        // CharStreams.readLines
        let mut reader = BufferedReader::new(reader);
        while let Some(line) = reader.read_line()? {
            result.insert(line.to_string_lossy());
        }
        Ok(result)
    }

    /// If subclasses want to modify the formatting, they should override
    /// `format_warning_with_meta_data`, not this method.
    // port: AllowlistWarningsGuard#formatWarning(JSError)
    pub fn format_warning(&self, error: &JSError) -> String {
        self.format_warning_with_meta_data(error, false)
    }

    /// `with_meta_data`: if true, include metadata that's useful to humans. This metadata won't
    /// be used for matching the warning.
    // port: AllowlistWarningsGuard#formatWarning(JSError,boolean)
    pub fn format_warning_with_meta_data(&self, error: &JSError, with_meta_data: bool) -> String {
        let mut sb = String::new();
        sb.push_str(&Self::normalize_source_name(error.source_name()));
        sb.push(':');
        if with_meta_data {
            sb.push_str(&error.get_line_number().to_string());
        }
        let lines = split_lines(error.description());
        sb.push_str("  ");
        sb.push_str(lines[0]);
        // Add the rest of the message as a comment.
        if with_meta_data {
            for line in lines.iter().skip(1) {
                sb.push_str("\n# ");
                sb.push_str(line);
            }
            sb.push('\n');
        }
        sb
    }

    // port: AllowlistWarningsGuard#normalizeSourceName
    fn normalize_source_name(source_name: Option<&str>) -> String {
        match source_name {
            // e.g.
            // "blaze-out/k8-fastbuild/genfiles/some/path/foo.js" -> "some/path/foo.js"
            Some(source_name) => replace_first(&BLAZE_OUT, source_name, ""),
            // StringBuilder#append(null)
            None => "null".to_string(),
        }
    }

    // port: AllowlistWarningsGuard#getFirstLine
    pub fn get_first_line(warning: &str) -> String {
        let units = JsString::from(warning);
        let line_length = units.index_of_char(u16::from(b'\n'));
        if line_length > 0 {
            return units.substring(0, line_length as usize).to_string_lossy();
        }
        warning.to_string()
    }

    /// Allow list builder.
    // port: AllowlistWarningsGuard.AllowlistBuilder#AllowlistBuilder
    pub fn allow_list_builder(&self) -> AllowListBuilder<'_> {
        AllowListBuilder {
            guard: self,
            warnings: Vec::new(),
            product_name: None,
            generator_target: None,
            header_note: None,
        }
    }
}

impl WarningsGuard for AllowListWarningsGuard {
    // port: AllowlistWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if error.default_level() == CheckLevel::ERROR {
            return None;
        }
        if !self.allow_list.is_empty() && self.contain_warning(&self.format_warning(error)) {
            // If the message matches the guard we use WARNING, so that it
            // - Shows up on stderr, and
            // - Gets caught by the AllowListBuilder downstream in the pipeline
            return Some(CheckLevel::WARNING);
        }
        None
    }
    // port: AllowlistWarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        Priority::SUPPRESS_BY_ALLOWLIST.get_value()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl fmt::Display for AllowListWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.AllowlistWarningsGuard@{:x}",
            self as *const Self as usize
        )
    }
}

/// Allow list builder (Java's inner class `AllowlistBuilder`).
pub struct AllowListBuilder<'a> {
    guard: &'a AllowListWarningsGuard,
    /// Java `LinkedHashSet<JSError>`: insertion order, record equality.
    warnings: Vec<JSError>,
    product_name: Option<String>,
    generator_target: Option<String>,
    header_note: Option<String>,
}

impl AllowListBuilder<'_> {
    /// Fill in your product name to get a fun message!
    // port: AllowlistWarningsGuard.AllowlistBuilder#setProductName
    pub fn set_product_name(&mut self, name: &str) -> &mut Self {
        self.product_name = Some(name.to_string());
        self
    }

    /// Fill in instructions on how to generate this allow list.
    // port: AllowlistWarningsGuard.AllowlistBuilder#setGeneratorTarget
    pub fn set_generator_target(&mut self, name: &str) -> &mut Self {
        self.generator_target = Some(name.to_string());
        self
    }

    /// A note to include at the top of the allow list file.
    // port: AllowlistWarningsGuard.AllowlistBuilder#setNote
    pub fn set_note(&mut self, note: &str) -> &mut Self {
        self.header_note = Some(note.to_string());
        self
    }

    /// Writes the warnings collected in a format that the guard can read back later.
    // port: AllowlistWarningsGuard.AllowlistBuilder#writeAllowlist
    pub fn write_allow_list(&self, out: &str) -> io::Result<()> {
        let mut stream = std::fs::File::create(out)?;
        self.append_allow_list(&mut stream)
    }

    /// Writes the warnings collected in a format that the guard can read back later.
    // port: AllowlistWarningsGuard.AllowlistBuilder#appendAllowlist
    pub fn append_allow_list(&self, out: &mut dyn Write) -> io::Result<()> {
        out.write_all(b"# This is a list of legacy warnings that have yet to be fixed.\n")?;
        if let Some(product_name) = &self.product_name
            && !product_name.is_empty()
            && !self.warnings.is_empty()
        {
            out.write_all(
                format!(
                    "# Please find some time and fix at least one of them and it will be the \
                     happiest day for {product_name}.\n"
                )
                .as_bytes(),
            )?;
        }
        if let Some(generator_target) = &self.generator_target
            && !generator_target.is_empty()
        {
            out.write_all(
                format!("# When you fix any of these warnings, run {generator_target} task.\n")
                    .as_bytes(),
            )?;
        }
        if let Some(header_note) = &self.header_note {
            out.write_all(format!("#{}\n", split_lines(header_note).join("\n# ")).as_bytes())?;
        }

        // TreeMultimap: keys in DiagnosticType#compareTo order, values in String order.
        let mut warnings_by_type: BTreeMap<&'static DiagnosticType, BTreeSet<Utf16Ordered>> =
            BTreeMap::new();
        for warning in &self.warnings {
            warnings_by_type
                .entry(warning.get_type())
                .or_default()
                .insert(Utf16Ordered(self.guard.format_warning_with_meta_data(
                    warning, true, /* withLineNumber */
                )));
        }
        for (type_, warnings) in &warnings_by_type {
            if DEPRECATED.matches_type(type_) {
                // Deprecation warnings are not raisable to error, so we don't need them in allow
                // lists.
                continue;
            }
            out.write_all(
                format!(
                    "\n# Warning {}: {}\n",
                    type_.key,
                    split_lines(type_.format)[0]
                )
                .as_bytes(),
            )?;
            for warning in warnings {
                out.write_all(format!("{}\n", warning.0).as_bytes())?;
            }
        }
        out.flush()
    }
}

impl ErrorHandler for AllowListBuilder<'_> {
    // port: AllowlistWarningsGuard.AllowlistBuilder#report
    fn report(&mut self, _level: CheckLevel, error: JSError) {
        if error.default_level() == CheckLevel::ERROR {
            // ERROR-level diagnostics are ignored by the guard (c.f. above level).
            return;
        }
        if !self.warnings.contains(&error) {
            self.warnings.push(error);
        }
    }
}

/// A `String` ordered by Java's `String#compareTo` (UTF-16 code units).
#[derive(PartialEq, Eq)]
struct Utf16Ordered(String);
impl Ord for Utf16Ordered {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.encode_utf16().cmp(other.0.encode_utf16())
    }
}
impl PartialOrd for Utf16Ordered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
