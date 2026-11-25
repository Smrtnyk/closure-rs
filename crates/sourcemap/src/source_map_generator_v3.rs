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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/debugging/sourcemap/SourceMapGeneratorV3.java.

use crate::{
    base64_vlq::Base64VLQ,
    file_position::FilePosition,
    gson::JsonElement,
    source_map_consumer_v3::SourceMapConsumerV3,
    source_map_generator::SourceMapGenerator,
    source_map_parse_exception::SourceMapParseException as Error,
    source_map_section::{SectionType, SourceMapSection},
    util::Util,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
const UNMAPPED: i32 = -1;
use std::fmt::{self, Write};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionValue {
    String(JsString),
    Integer(i32),
    JsonElement(JsonElement),
}
impl fmt::Display for ExtensionValue {
    // Rust Display dispatch among the supported Java Object value types.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(v) => write!(f, "{v}"),
            Self::Integer(v) => write!(f, "{v}"),
            Self::JsonElement(v) => write!(f, "{v}"),
        }
    }
}
pub trait ExtensionMergeAction {
    // port: SourceMapGeneratorV3.ExtensionMergeAction#merge
    fn merge(
        &mut self,
        extension_key: &JsString,
        current_value: &ExtensionValue,
        new_value: &ExtensionValue,
    ) -> ExtensionValue;
}
impl<F: FnMut(&JsString, &ExtensionValue, &ExtensionValue) -> ExtensionValue> ExtensionMergeAction
    for F
{
    // Rust closure adapter.
    fn merge(&mut self, k: &JsString, c: &ExtensionValue, n: &ExtensionValue) -> ExtensionValue {
        self(k, c, n)
    }
}
#[derive(Default)]
pub struct SourceMapGeneratorV3 {
    mappings: Vec<Mapping>,
    source_file_map: MappingNames,
    source_file_content_map: IndexMap<JsString, Option<JsString>>,
    original_name_map: MappingNames,
    max_used_end_line: i32,
    offset_position: FilePosition,
    prefix_position: FilePosition,
    extensions: IndexMap<JsString, ExtensionValue>,
    source_root_path: Option<JsString>,
    /// Whether to omit a mapped segment whose original position and name are identical to those
    /// of the previous segment written on the same generated line.
    merge_identical_segments: bool,
}
impl SourceMapGeneratorV3 {
    // port: SourceMapGeneratorV3#SourceMapGeneratorV3
    pub fn new() -> Self {
        Self::default()
    }
    // port: SourceMapGeneratorV3#reset
    pub fn reset(&mut self) {
        self.mappings.clear();
        self.source_file_map.clear();
        self.original_name_map.clear();
        self.max_used_end_line = 0;
        self.offset_position = FilePosition::new(0, 0);
        self.prefix_position = FilePosition::new(0, 0);
    }
    // port: SourceMapGeneratorV3#validate
    pub fn validate(&mut self, _validate: bool) {}
    /// Sets whether to omit a mapped segment whose source, original line, original column, and
    /// name are identical to those of the previous segment written on the same generated line.
    ///
    /// Lookups from a generated position to an original position are unchanged: a lookup
    /// resolves to the closest preceding segment on the line, which carries the same original
    /// position and name. The first segment of each generated line is always written, and
    /// unmapped segments are never merged. Reverse lookups may report fewer generated columns
    /// for an original position.
    // port: SourceMapGeneratorV3#setMergeIdenticalSegments
    pub fn set_merge_identical_segments(&mut self, merge_identical_segments: bool) {
        self.merge_identical_segments = merge_identical_segments;
    }
    // port: SourceMapGeneratorV3#setWrapperPrefix
    pub fn set_wrapper_prefix(&mut self, prefix: impl Into<JsString>) {
        let mut prefix_line = 0i32;
        let mut prefix_index = 0i32;
        let prefix = prefix.into();
        for i in 0..prefix.length() {
            if prefix.char_at(i) == b'\n' as u16 {
                prefix_line = prefix_line.wrapping_add(1);
                prefix_index = 0;
            } else {
                prefix_index = prefix_index.wrapping_add(1);
            }
        }
        self.prefix_position = FilePosition::new(prefix_line, prefix_index);
    }
    // port: SourceMapGeneratorV3#setStartingPosition
    pub fn set_starting_position(&mut self, offset_line: i32, offset_index: i32) {
        assert!(offset_line >= 0, "java.lang.IllegalStateException");
        assert!(offset_index >= 0, "java.lang.IllegalStateException");
        self.offset_position = FilePosition::new(offset_line, offset_index);
    }
    // port: SourceMapGeneratorV3#addMapping
    pub fn add_mapping(
        &mut self,
        source_name: Option<JsString>,
        symbol_name: Option<JsString>,
        source_start_position: FilePosition,
        start_position: FilePosition,
        end_position: FilePosition,
    ) {
        let Some(source_name) = source_name else {
            return;
        };
        if source_start_position.get_line() < 0 {
            return;
        }
        let mut adjusted_start = start_position;
        let mut adjusted_end = end_position;
        if self.offset_position.get_line() != 0 || self.offset_position.get_column() != 0 {
            let offset_line = self.offset_position.get_line();
            let mut start_offset_position = self.offset_position.get_column();
            let mut end_offset_position = self.offset_position.get_column();
            if start_position.get_line() > 0 {
                start_offset_position = 0;
            }
            if end_position.get_line() > 0 {
                end_offset_position = 0;
            }
            adjusted_start = FilePosition::new(
                start_position.get_line().wrapping_add(offset_line),
                start_position
                    .get_column()
                    .wrapping_add(start_offset_position),
            );
            adjusted_end = FilePosition::new(
                end_position.get_line().wrapping_add(offset_line),
                end_position.get_column().wrapping_add(end_offset_position),
            );
        }
        if let Some(last_mapping) = self.mappings.last() {
            let last_line = last_mapping.start_position.get_line();
            let last_column = last_mapping.start_position.get_column();
            let next_line = adjusted_start.get_line();
            let next_column = adjusted_start.get_column();
            assert!(
                next_line > last_line || (next_line == last_line && next_column >= last_column),
                "java.lang.IllegalStateException: Incorrect source mappings order, previous : ({last_line},{last_column})\nnew : ({next_line},{next_column})"
            );
        }
        // Consecutive mappings commonly come from the same file.
        let source_file = match self.mappings.last() {
            Some(last) if self.source_file_map.name(last.source_file) == &source_name => {
                last.source_file
            }
            _ => self.source_file_map.intern(source_name),
        };
        self.mappings.push(Mapping {
            source_file,
            original_name: symbol_name.map_or(UNMAPPED, |name| self.original_name_map.intern(name)),
            original_position: source_start_position,
            start_position: adjusted_start,
            end_position: adjusted_end,
        });
    }
    // port: SourceMapGeneratorV3#addSourcesContent
    // Java's map value is a nullable String: a null content (e.g. a null "sourcesContent" entry of
    // an input source map) is stored too, replacing earlier content, and is written as "".
    pub fn add_sources_content(&mut self, source: impl Into<JsString>, content: Option<JsString>) {
        self.source_file_content_map.insert(source.into(), content);
    }
    // port: SourceMapGeneratorV3#mergeMapSection(int,int,String)
    pub fn merge_map_section(
        &mut self,
        line: i32,
        column: i32,
        map_section_contents: impl Into<JsString>,
    ) -> Result<(), Error> {
        self.set_starting_position(line, column);
        let mut section = SourceMapConsumerV3::new();
        section.parse(map_section_contents)?;
        section.visit_mappings(&mut ConsumerEntryVisitor(self));
        Ok(())
    }
    // port: SourceMapGeneratorV3#mergeMapSection(int,int,String,ExtensionMergeAction)
    pub fn merge_map_section_with_merge_action(
        &mut self,
        line: i32,
        column: i32,
        map_section_contents: impl Into<JsString>,
        merge_action: &mut dyn ExtensionMergeAction,
    ) -> Result<(), Error> {
        self.set_starting_position(line, column);
        let mut section = SourceMapConsumerV3::new();
        section.parse(map_section_contents)?;
        section.visit_mappings(&mut ConsumerEntryVisitor(self));
        for (key, value) in section.get_extensions() {
            let extension_key = key;
            if self.extensions.contains_key(extension_key) {
                let value = merge_action.merge(
                    extension_key,
                    self.extensions.get(extension_key).unwrap(),
                    value,
                );
                self.extensions.insert(extension_key.clone(), value);
            } else {
                self.extensions.insert(extension_key.clone(), value.clone());
            }
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3#appendTo
    pub fn append_to(
        &mut self,
        out: &mut dyn Write,
        name: Option<JsString>,
    ) -> Result<JsString, fmt::Error> {
        let mut units = Vec::new();
        self.write_map(&mut JavaAppendable::new(out, Some(&mut units)), name)?;
        Ok(JsString::from_units(units))
    }

    fn write_map(&mut self, out: &mut JavaAppendable<'_>, name: Option<JsString>) -> fmt::Result {
        let max_line = self.prep_mappings()?.wrapping_add(1);
        out.write_str("{\n")?;
        Self::append_first_field(out, "version", "3")?;
        if let Some(name) = name {
            Self::append_escaped_field(out, "file", &name)?;
        }
        Self::append_field(out, "lineCount", max_line.to_string())?;
        if let Some(root) = &self.source_root_path
            && !root.is_empty()
        {
            Self::append_escaped_field(out, "sourceRoot", root)?;
        }
        Self::append_field_start(out, "mappings")?;
        let traversal = MappingTraversal::new(&self.mappings, self.prefix_position);
        LineMapper::new(
            out,
            max_line,
            self.merge_identical_segments,
            &mut self.source_file_map,
            &mut self.original_name_map,
        )
        .append_line_mappings(traversal)?;
        Self::append_field_end(out);
        Self::append_field_start(out, "sources")?;
        out.write_char('[')?;
        self.add_source_name_map(out)?;
        out.write_char(']')?;
        Self::append_field_end(out);
        self.add_sources_content_map(out)?;
        Self::append_field_start(out, "names")?;
        out.write_char('[')?;
        self.add_symbol_name_map(out)?;
        out.write_char(']')?;
        Self::append_field_end(out);
        for (key, value) in &self.extensions {
            match value {
                ExtensionValue::String(value) => Self::append_escaped_field(out, key, value)?,
                ExtensionValue::Integer(value) => Self::append_field(out, key, value.to_string())?,
                ExtensionValue::JsonElement(value) => {
                    Self::append_field(out, key, value.to_js_string())?
                }
            }
        }
        out.write_str("\n}\n")
    }
    // port: SourceMapGeneratorV3#setSourceRoot
    pub fn set_source_root(&mut self, path: impl Into<JsString>) {
        self.source_root_path = Some(path.into());
    }
    // port: SourceMapGeneratorV3#addExtension
    pub fn add_extension(
        &mut self,
        name: impl Into<JsString>,
        object: ExtensionValue,
    ) -> Result<(), Error> {
        let name = name.into();
        if !name.starts_with("x_") {
            return Err(Error::new(
                JsString::from("Extension '")
                    .concat(&name)
                    .concat(&"' must start with 'x_'".into()),
            ));
        }
        self.extensions.insert(name, object);
        Ok(())
    }
    // port: SourceMapGeneratorV3#removeExtension
    pub fn remove_extension(&mut self, name: impl Into<JsString>) {
        let name = name.into();
        if self.extensions.contains_key(&name) {
            self.extensions.shift_remove(&name);
        }
    }
    // port: SourceMapGeneratorV3#hasExtension
    pub fn has_extension(&self, name: impl Into<JsString>) -> bool {
        self.extensions.contains_key(&name.into())
    }
    // port: SourceMapGeneratorV3#getExtension
    pub fn get_extension(&self, name: impl Into<JsString>) -> Option<&ExtensionValue> {
        self.extensions.get(&name.into())
    }
    // port: SourceMapGeneratorV3#addSourceNameMap
    fn add_source_name_map(&self, out: &mut JavaAppendable<'_>) -> fmt::Result {
        Self::add_name_map(out, &self.source_file_map)
    }
    // port: SourceMapGeneratorV3#addSourcesContentMap
    fn add_sources_content_map(&self, out: &mut JavaAppendable<'_>) -> fmt::Result {
        let contents = || {
            self.source_file_map.emitted_names().map(|name| {
                self.source_file_content_map
                    .get(name)
                    .and_then(Option::as_ref)
            })
        };
        if !contents().any(|content| content.is_some()) {
            return Ok(());
        }
        Self::append_field_start(out, "sourcesContent")?;
        out.write_char('[')?;
        for (i, content) in contents().enumerate() {
            if i != 0 {
                out.write_char(',')?;
            }
            if let Some(content) = content {
                Util::write_escaped_string(out, content)?;
            } else {
                out.write_str("\"\"")?;
            }
        }
        out.write_char(']')?;
        Self::append_field_end(out);
        Ok(())
    }
    // port: SourceMapGeneratorV3#addSymbolNameMap
    fn add_symbol_name_map(&self, out: &mut JavaAppendable<'_>) -> fmt::Result {
        Self::add_name_map(out, &self.original_name_map)
    }
    // port: SourceMapGeneratorV3#addNameMap
    fn add_name_map(out: &mut JavaAppendable<'_>, map: &MappingNames) -> fmt::Result {
        for (i, key) in map.emitted_names().enumerate() {
            if i != 0 {
                out.write_char(',')?;
            }
            Util::write_escaped_string(out, key)?;
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3#escapeString
    fn append_escaped_field(
        out: &mut JavaAppendable<'_>,
        name: impl Into<JsString>,
        value: &JsString,
    ) -> fmt::Result {
        Self::append_field_start(out, name)?;
        Util::write_escaped_string(out, value)
    }
    // port: SourceMapGeneratorV3#appendFirstField
    fn append_first_field(
        out: &mut JavaAppendable<'_>,
        name: impl Into<JsString>,
        value: impl Into<JsString>,
    ) -> fmt::Result {
        Self::append_field_start_with_first(out, name, true)?;
        out.append(&value.into())
    }
    // port: SourceMapGeneratorV3#appendField
    fn append_field(
        out: &mut JavaAppendable<'_>,
        name: impl Into<JsString>,
        value: impl Into<JsString>,
    ) -> fmt::Result {
        Self::append_field_start_with_first(out, name, false)?;
        out.append(&value.into())
    }
    // port: SourceMapGeneratorV3#appendFieldStart(Appendable,String)
    fn append_field_start(out: &mut JavaAppendable<'_>, name: impl Into<JsString>) -> fmt::Result {
        Self::append_field_start_with_first(out, name, false)
    }
    // port: SourceMapGeneratorV3#appendFieldStart(Appendable,String,boolean)
    fn append_field_start_with_first(
        out: &mut JavaAppendable<'_>,
        name: impl Into<JsString>,
        first: bool,
    ) -> fmt::Result {
        if !first {
            out.write_str(",\n")?;
        }
        out.write_char('"')?;
        out.append(&name.into())?;
        out.write_char('"')?;
        out.write_char(':')
    }
    // port: SourceMapGeneratorV3#appendFieldEnd
    fn append_field_end(_out: &mut JavaAppendable<'_>) {}
    // port: SourceMapGeneratorV3#prepMappings
    fn prep_mappings(&mut self) -> Result<i32, fmt::Error> {
        // Only the greatest end line of the mappings ever visited is observable. Java preserves
        // their used flags across appends; keep that maximum until reset.
        MappingTraversal::new(&self.mappings, self.prefix_position)
            .traverse(&mut UsedMappingCheck(&mut self.max_used_end_line))?;
        Ok(self
            .max_used_end_line
            .wrapping_add(self.prefix_position.get_line()))
    }
    // port: SourceMapGeneratorV3#appendIndexMapTo
    pub fn append_index_map_to(
        &self,
        out: &mut dyn Write,
        name: impl Into<JsString>,
        sections: &[SourceMapSection],
    ) -> Result<JsString, fmt::Error> {
        let name = name.into();
        let mut units = Vec::new();
        let mut appendable = JavaAppendable::new(out, Some(&mut units));
        let out = &mut appendable;
        out.write_str("{\n")?;
        Self::append_first_field(out, "version", "3")?;
        Self::append_escaped_field(out, "file", &name)?;
        Self::append_field_start(out, "sections")?;
        out.write_str("[\n")?;
        let mut first = true;
        for section in sections {
            if first {
                first = false;
            } else {
                out.write_str(",\n")?;
            }
            out.write_str("{\n")?;
            Self::append_field_start_with_first(out, "offset", true)?;
            Self::append_offset_value(out, section.line, section.column)?;
            if section.get_section_type() == SectionType::URL {
                Self::append_escaped_field(out, "url", section.get_section_value())?;
            } else if section.get_section_type() == SectionType::MAP {
                Self::append_field(out, "map", section.get_section_value())?;
            }
            // SectionType is exhaustive in Rust; Java's IOException fallback is unreachable.
            out.write_str("\n}")?;
        }
        out.write_str("\n]")?;
        Self::append_field_end(out);
        out.write_str("\n}\n")?;
        Ok(JsString::from_units(units))
    }
    // port: SourceMapGeneratorV3#appendOffsetValue
    fn append_offset_value(out: &mut JavaAppendable<'_>, line: i32, column: i32) -> fmt::Result {
        out.write_str("{\n")?;
        Self::append_first_field(out, "line", line.to_string())?;
        Self::append_field(out, "column", column.to_string())?;
        out.write_str("\n}")
    }
}
struct ConsumerEntryVisitor<'a>(&'a mut SourceMapGeneratorV3);
impl crate::source_map_consumer_v3::EntryVisitor for ConsumerEntryVisitor<'_> {
    // port: SourceMapGeneratorV3.ConsumerEntryVisitor#visit
    fn visit(
        &mut self,
        source_name: Option<&JsString>,
        symbol_name: Option<&JsString>,
        source_start_position: FilePosition,
        start_position: FilePosition,
        end_position: FilePosition,
    ) {
        self.0.add_mapping(
            source_name.cloned(),
            symbol_name.cloned(),
            source_start_position,
            start_position,
            end_position,
        );
    }
}

// Rust-only (performance divergence): the per-mapping record holds compact file/name keys.
// An interned key gets its source-map index only when the traversal actually writes it. A
// covered mapping can therefore arrive first and emit its name later, or never emit it at all.
#[derive(Default)]
struct MappingNames {
    interned: IndexMap<JsString, i32>,
    emitted_ids: Vec<i32>,
    emitted: Vec<i32>,
}
impl MappingNames {
    fn clear(&mut self) {
        self.interned.clear();
        self.emitted_ids.clear();
        self.emitted.clear();
    }

    fn intern(&mut self, name: JsString) -> i32 {
        let entry = self.interned.entry(name);
        let index = entry.index();
        if index == self.emitted_ids.len() {
            entry.or_insert(UNMAPPED);
            self.emitted_ids.push(UNMAPPED);
        }
        i32::try_from(index).expect("too many source map names")
    }

    fn name(&self, index: i32) -> &JsString {
        self.interned.get_index(index as usize).unwrap().0
    }

    // port: SourceMapGeneratorV3#getSourceId
    // port: SourceMapGeneratorV3#getNameId
    #[inline]
    fn emitted_id(
        &mut self,
        index: i32,
        output: &mut MappingBuffer<'_, '_>,
    ) -> Result<i32, fmt::Error> {
        let id = &mut self.emitted_ids[index as usize];
        if *id == UNMAPPED {
            // With a fallible Appendable, earlier segment bytes must reach the writer before it
            // can observe a new name on a subsequent append.
            output.flush()?;
            *id = self.emitted.len() as i32;
            self.emitted.push(index);
        }
        Ok(*id)
    }

    fn emitted_names(&self) -> impl Iterator<Item = &JsString> {
        self.emitted.iter().map(|&index| self.name(index))
    }
}

struct Mapping {
    source_file: i32,
    original_name: i32,
    original_position: FilePosition,
    start_position: FilePosition,
    end_position: FilePosition,
}

/// Java's `MappingVisitor`; `'m` is the lifetime of the visited mappings, so that a visitor can
/// keep one (Java keeps the object).
trait MappingVisitor<'m> {
    // port: SourceMapGeneratorV3.MappingVisitor#visit
    fn visit(
        &mut self,
        m: Option<&'m Mapping>,
        line: i32,
        col: i32,
        end_line: i32,
        end_col: i32,
    ) -> fmt::Result;
}
struct UsedMappingCheck<'a>(&'a mut i32);
impl<'m> MappingVisitor<'m> for UsedMappingCheck<'_> {
    // port: SourceMapGeneratorV3.UsedMappingCheck#visit
    fn visit(
        &mut self,
        m: Option<&'m Mapping>,
        _line: i32,
        _col: i32,
        _end_line: i32,
        _end_col: i32,
    ) -> fmt::Result {
        if let Some(m) = m {
            *self.0 = (*self.0).max(m.end_position.get_line());
        }
        Ok(())
    }
}
struct MappingTraversal<'a> {
    mappings: &'a [Mapping],
    prefix_position: FilePosition,
    line: i32,
    col: i32,
}
impl<'a> MappingTraversal<'a> {
    // port: SourceMapGeneratorV3.MappingTraversal#MappingTraversal
    fn new(mappings: &'a [Mapping], prefix_position: FilePosition) -> Self {
        Self {
            mappings,
            prefix_position,
            line: 0,
            col: 0,
        }
    }
    // Preserve the Java statement order.
    #[allow(clippy::manual_while_let_some)]
    // port: SourceMapGeneratorV3.MappingTraversal#traverse
    fn traverse(mut self, v: &mut impl MappingVisitor<'a>) -> fmt::Result {
        let mut stack: Vec<&'a Mapping> = Vec::new();
        for m in self.mappings {
            while stack
                .last()
                .is_some_and(|previous| !Self::is_overlapped(previous, m))
            {
                let previous = stack.pop().unwrap();
                self.maybe_visit(v, previous)?;
            }
            let parent = stack.last().copied();
            self.maybe_visit_parent(v, parent, m)?;
            stack.push(m);
        }
        while !stack.is_empty() {
            let m = stack.pop().unwrap();
            self.maybe_visit(v, m)?;
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3.MappingTraversal#getAdjustedLine
    fn get_adjusted_line(&self, p: FilePosition) -> i32 {
        p.get_line().wrapping_add(self.prefix_position.get_line())
    }
    // port: SourceMapGeneratorV3.MappingTraversal#getAdjustedCol
    fn get_adjusted_col(&self, p: FilePosition) -> i32 {
        let raw_line = p.get_line();
        let raw_col = p.get_column();
        if raw_line != 0 {
            raw_col
        } else {
            raw_col.wrapping_add(self.prefix_position.get_column())
        }
    }
    // port: SourceMapGeneratorV3.MappingTraversal#isOverlapped
    fn is_overlapped(m1: &Mapping, m2: &Mapping) -> bool {
        let l1 = m1.end_position.get_line();
        let l2 = m2.start_position.get_line();
        let c1 = m1.end_position.get_column();
        let c2 = m2.start_position.get_column();
        (l1 == l2 && c1 >= c2) || l1 > l2
    }
    // port: SourceMapGeneratorV3.MappingTraversal#maybeVisit
    fn maybe_visit(&mut self, v: &mut impl MappingVisitor<'a>, m: &'a Mapping) -> fmt::Result {
        let next_line = self.get_adjusted_line(m.end_position);
        let next_col = self.get_adjusted_col(m.end_position);
        if self.line < next_line || (self.line == next_line && self.col < next_col) {
            self.visit(v, Some(m), next_line, next_col)?;
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3.MappingTraversal#maybeVisitParent
    fn maybe_visit_parent(
        &mut self,
        v: &mut impl MappingVisitor<'a>,
        parent: Option<&'a Mapping>,
        m: &'a Mapping,
    ) -> fmt::Result {
        let next_line = self.get_adjusted_line(m.start_position);
        let next_col = self.get_adjusted_col(m.start_position);
        assert!(
            self.line < next_line || self.col <= next_col,
            "java.lang.IllegalStateException"
        );
        if self.line < next_line || (self.line == next_line && self.col < next_col) {
            self.visit(v, parent, next_line, next_col)?;
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3.MappingTraversal#visit
    fn visit(
        &mut self,
        v: &mut impl MappingVisitor<'a>,
        m: Option<&'a Mapping>,
        next_line: i32,
        next_col: i32,
    ) -> fmt::Result {
        assert!(self.line <= next_line, "java.lang.IllegalStateException");
        assert!(
            self.line < next_line || self.col < next_col,
            "java.lang.IllegalStateException"
        );
        if self.line == next_line && self.col == next_col {
            panic!("java.lang.IllegalStateException");
        }
        v.visit(m, self.line, self.col, next_line, next_col)?;
        self.line = next_line;
        self.col = next_col;
        Ok(())
    }
}
// Rust-only (performance divergence): collect ASCII mapping bytes in bounded runs. The
// underlying Appendable receives whole segments in one write.
struct MappingBuffer<'a, 'out> {
    dest: &'a mut JavaAppendable<'out>,
    bytes: Vec<u8>,
}
impl MappingBuffer<'_, '_> {
    fn flush(&mut self) -> fmt::Result {
        if !self.bytes.is_empty() {
            let s = std::str::from_utf8(&self.bytes).expect("mapping buffer is ASCII");
            self.dest.write_str(s)?;
            self.bytes.clear();
        }
        Ok(())
    }

    #[inline]
    fn flush_if_full(&mut self) -> fmt::Result {
        if self.bytes.len() >= 65_536 {
            self.flush()?;
        }
        Ok(())
    }
}
struct LineMapper<'a, 'out, 'm> {
    out: MappingBuffer<'a, 'out>,
    max_line: i32,
    /// The outer generator's `mergeIdenticalSegments`.
    merge_identical_segments: bool,
    previous_line: i32,
    previous_column: i32,
    previous_source_file_id: i32,
    previous_source_line: i32,
    previous_source_column: i32,
    previous_name_id: i32,
    /// The mapping of the last entry written, or null if that entry was unmapped. Only meaningful
    /// when the entry being visited is on previousLine.
    previous_mapping: Option<&'m Mapping>,
    source_file_map: &'a mut MappingNames,
    original_name_map: &'a mut MappingNames,
}
impl<'m> MappingVisitor<'m> for LineMapper<'_, '_, 'm> {
    /// As each segment is visited write out the appropriate line mapping.
    // port: SourceMapGeneratorV3.LineMapper#visit
    fn visit(
        &mut self,
        m: Option<&'m Mapping>,
        line: i32,
        col: i32,
        next_line: i32,
        next_col: i32,
    ) -> fmt::Result {
        if self.previous_line != line {
            self.previous_column = 0;
        }
        if line != next_line || col != next_col {
            if line < self.max_line {
                // When merging, skip a segment that the previous entry on this line already covers
                // with the same original position and name. VLQ deltas stay relative to the last
                // written entry.
                let merge = self.merge_identical_segments
                    && self.previous_line == line
                    && Self::has_same_origin(m, self.previous_mapping);
                if !merge {
                    if self.previous_line == line {
                        // not the first entry for the line
                        self.out.bytes.push(b',');
                    }
                    self.write_entry(m, col)?;
                    self.previous_line = line;
                    self.previous_column = col;
                    self.previous_mapping = m;
                }
            } else {
                assert!(m.is_none(), "java.lang.IllegalStateException");
            }
        }
        let mut i = line;
        while i <= next_line && i < self.max_line {
            if i == next_line {
                break;
            }
            self.close_line(false)?;
            self.open_line(false)?;
            i += 1;
        }
        Ok(())
    }
}
impl<'m> LineMapper<'_, '_, 'm> {
    // Borrowed outer generator fields are passed explicitly to preserve Rust's aliasing rules.
    // port: SourceMapGeneratorV3.LineMapper#LineMapper
    fn new<'a, 'out>(
        out: &'a mut JavaAppendable<'out>,
        max_line: i32,
        merge_identical_segments: bool,
        source_file_map: &'a mut MappingNames,
        original_name_map: &'a mut MappingNames,
    ) -> LineMapper<'a, 'out, 'm> {
        LineMapper {
            out: MappingBuffer {
                dest: out,
                bytes: Vec::with_capacity(65_536),
            },
            max_line,
            merge_identical_segments,
            previous_line: -1,
            previous_column: 0,
            previous_source_file_id: 0,
            previous_source_line: 0,
            previous_source_column: 0,
            previous_name_id: 0,
            previous_mapping: None,
            source_file_map,
            original_name_map,
        }
    }
    /// Returns whether both mappings are non-null and have the same source file, original
    /// position, and original name, i.e. would be written as the same source map segment apart
    /// from the generated column.
    // port: SourceMapGeneratorV3.LineMapper#hasSameOrigin
    fn has_same_origin(m: Option<&Mapping>, previous: Option<&Mapping>) -> bool {
        let (Some(m), Some(previous)) = (m, previous) else {
            return false;
        };
        m.source_file == previous.source_file
            && m.original_position.get_line() == previous.original_position.get_line()
            && m.original_position.get_column() == previous.original_position.get_column()
            && m.original_name == previous.original_name
    }
    // port: SourceMapGeneratorV3.LineMapper#writeEntry
    fn write_entry(&mut self, m: Option<&'m Mapping>, column: i32) -> fmt::Result {
        Base64VLQ::encode_to_bytes(
            &mut self.out.bytes,
            column.wrapping_sub(self.previous_column),
        );
        self.previous_column = column;
        if let Some(m) = m {
            let source_id = self
                .source_file_map
                .emitted_id(m.source_file, &mut self.out)?;
            Base64VLQ::encode_to_bytes(
                &mut self.out.bytes,
                source_id.wrapping_sub(self.previous_source_file_id),
            );
            self.previous_source_file_id = source_id;
            let srcline = m.original_position.get_line();
            let src_column = m.original_position.get_column();
            Base64VLQ::encode_to_bytes(
                &mut self.out.bytes,
                srcline.wrapping_sub(self.previous_source_line),
            );
            self.previous_source_line = srcline;
            Base64VLQ::encode_to_bytes(
                &mut self.out.bytes,
                src_column.wrapping_sub(self.previous_source_column),
            );
            self.previous_source_column = src_column;
            if m.original_name != UNMAPPED {
                let name_id = self
                    .original_name_map
                    .emitted_id(m.original_name, &mut self.out)?;
                Base64VLQ::encode_to_bytes(
                    &mut self.out.bytes,
                    name_id.wrapping_sub(self.previous_name_id),
                );
                self.previous_name_id = name_id;
            }
        }
        self.out.flush_if_full()
    }
    // port: SourceMapGeneratorV3.LineMapper#appendLineMappings
    fn append_line_mappings(&mut self, traversal: MappingTraversal<'m>) -> fmt::Result {
        self.open_line(true)?;
        traversal.traverse(self)?;
        self.close_line(true)
    }
    // port: SourceMapGeneratorV3.LineMapper#openLine
    fn open_line(&mut self, first_entry: bool) -> fmt::Result {
        if first_entry {
            self.out.bytes.push(b'"');
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3.LineMapper#closeLine
    fn close_line(&mut self, final_entry: bool) -> fmt::Result {
        self.out.bytes.push(b';');
        if final_entry {
            self.out.bytes.push(b'"');
            self.out.flush()
        } else {
            self.out.flush_if_full()
        }
    }
}

impl SourceMapGenerator for SourceMapGeneratorV3 {
    // Rust trait dispatch to the single Java method body.
    fn append_to(
        &mut self,
        out: &mut dyn Write,
        name: Option<JsString>,
    ) -> Result<JsString, fmt::Error> {
        self.append_to(out, name)
    }
    fn write_to(&mut self, out: &mut dyn Write, name: Option<JsString>) -> fmt::Result {
        self.write_map(&mut JavaAppendable::new(out, None), name)
    }
    // Rust trait dispatch to the single Java method body.
    fn append_index_map_to(
        &self,
        out: &mut dyn Write,
        name: JsString,
        sections: &[SourceMapSection],
    ) -> Result<JsString, fmt::Error> {
        self.append_index_map_to(out, name, sections)
    }
    // Rust trait dispatch to the single Java method body.
    fn reset(&mut self) {
        self.reset()
    }
    // Rust trait dispatch to the single Java method body.
    fn validate(&mut self, validate: bool) {
        self.validate(validate)
    }
    // Rust trait dispatch to the single Java method body.
    fn set_merge_identical_segments(&mut self, merge_identical_segments: bool) {
        self.set_merge_identical_segments(merge_identical_segments)
    }
    // Rust trait dispatch to the single Java method body.
    fn set_wrapper_prefix(&mut self, prefix: JsString) {
        self.set_wrapper_prefix(prefix)
    }
    // Rust trait dispatch to the single Java method body.
    fn set_starting_position(&mut self, line: i32, column: i32) {
        self.set_starting_position(line, column)
    }
    // Rust trait dispatch to the single Java method body.
    fn add_sources_content(&mut self, source: JsString, content: Option<JsString>) {
        self.add_sources_content(source, content)
    }
    // Rust trait dispatch to the single Java method body.
    fn add_mapping(
        &mut self,
        source: Option<JsString>,
        symbol: Option<JsString>,
        original: FilePosition,
        start: FilePosition,
        end: FilePosition,
    ) {
        self.add_mapping(source, symbol, original, start, end)
    }
}

// Util.escapeString produces ASCII, but raw section maps, extension keys and
// JsonElement.toString can contain lone surrogates. This single internal buffer
// preserves those code units; fmt::Write receives their scalar display. Returning
// the completed JsString also lets lossless callers retain the Java output.
struct JavaAppendable<'a> {
    out: &'a mut dyn Write,
    units: Option<&'a mut Vec<u16>>,
}
impl<'a> JavaAppendable<'a> {
    fn new(out: &'a mut dyn Write, units: Option<&'a mut Vec<u16>>) -> Self {
        Self { out, units }
    }
    fn append(&mut self, s: &JsString) -> fmt::Result {
        if let Some(units) = self.units.as_mut() {
            units.extend_from_slice(s.as_units());
        }
        write!(self.out, "{s}")
    }
}
impl Write for JavaAppendable<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Rust-only: the code units are appended directly, not through a JS string
        // (performance divergence).
        if let Some(units) = self.units.as_mut() {
            units.extend(s.encode_utf16());
        }
        self.out.write_str(s)
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        if let Some(units) = self.units.as_mut() {
            let mut buf = [0u16; 2];
            units.extend_from_slice(c.encode_utf16(&mut buf));
        }
        self.out.write_char(c)
    }
}
