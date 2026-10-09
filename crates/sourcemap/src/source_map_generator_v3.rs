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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
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
use std::{
    cell::Cell,
    fmt::{self, Write},
};
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
    source_file_map: IndexMap<JsString, i32>,
    source_file_content_map: IndexMap<JsString, Option<JsString>>,
    original_name_map: IndexMap<JsString, i32>,
    last_source_file: Option<JsString>,
    last_source_file_index: i32,
    last_mapping: Option<usize>,
    offset_position: FilePosition,
    prefix_position: FilePosition,
    extensions: IndexMap<JsString, ExtensionValue>,
    source_root_path: Option<JsString>,
}
impl SourceMapGeneratorV3 {
    // port: SourceMapGeneratorV3#SourceMapGeneratorV3
    pub fn new() -> Self {
        Self {
            last_source_file_index: -1,
            ..Self::default()
        }
    }
    // port: SourceMapGeneratorV3#reset
    pub fn reset(&mut self) {
        self.mappings.clear();
        self.last_mapping = None;
        self.source_file_map.clear();
        self.original_name_map.clear();
        self.last_source_file = None;
        self.last_source_file_index = -1;
        self.offset_position = FilePosition::new(0, 0);
        self.prefix_position = FilePosition::new(0, 0);
    }
    // port: SourceMapGeneratorV3#validate
    pub fn validate(&mut self, _validate: bool) {}
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
        if source_name.is_none() || source_start_position.get_line() < 0 {
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
        let mut mapping = Mapping::new();
        mapping.source_file = source_name;
        mapping.original_position = source_start_position;
        mapping.original_name = symbol_name;
        mapping.start_position = adjusted_start;
        mapping.end_position = adjusted_end;
        if let Some(last_mapping) = self.last_mapping {
            let last_mapping = &self.mappings[last_mapping];
            let last_line = last_mapping.start_position.get_line();
            let last_column = last_mapping.start_position.get_column();
            let next_line = mapping.start_position.get_line();
            let next_column = mapping.start_position.get_column();
            assert!(
                next_line > last_line || (next_line == last_line && next_column >= last_column),
                "java.lang.IllegalStateException: Incorrect source mappings order, previous : ({last_line},{last_column})\nnew : ({next_line},{next_column})"
            );
        }
        self.last_mapping = Some(self.mappings.len());
        self.mappings.push(mapping);
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
    // Preserve Java's assigned-then-used extension value.
    #[allow(clippy::needless_late_init)]
    // port: SourceMapGeneratorV3#appendTo
    pub fn append_to(
        &mut self,
        out: &mut dyn Write,
        name: Option<JsString>,
    ) -> Result<JsString, fmt::Error> {
        let mut appendable = JavaAppendable::new(out);
        let out = &mut appendable;
        let max_line = self.prep_mappings()?.wrapping_add(1);
        out.write_str("{\n")?;
        Self::append_first_field(out, "version", "3")?;
        if let Some(name) = name {
            Self::append_field(out, "file", Self::escape_string(name))?;
        }
        Self::append_field(out, "lineCount", max_line.to_string())?;
        if let Some(root) = &self.source_root_path
            && !root.is_empty()
        {
            Self::append_field(out, "sourceRoot", Self::escape_string(root))?;
        }
        Self::append_field_start(out, "mappings")?;
        let traversal = MappingTraversal::new(&self.mappings, self.prefix_position);
        LineMapper::new(
            out,
            max_line,
            &mut self.source_file_map,
            &mut self.original_name_map,
            &mut self.last_source_file,
            &mut self.last_source_file_index,
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
        for key in self.extensions.keys() {
            let obj_value = self.extensions.get(key).unwrap();
            let value;
            if let ExtensionValue::String(string) = obj_value {
                value = Self::escape_string(string);
            } else {
                // Object.toString dispatch to the supported extension value types.
                value = match obj_value {
                    ExtensionValue::Integer(value) => value.to_string().into(),
                    ExtensionValue::JsonElement(value) => value.to_js_string(),
                    ExtensionValue::String(_) => unreachable!(),
                };
            }
            Self::append_field(out, key, value)?;
        }
        out.write_str("\n}\n")?;
        Ok(appendable.finish())
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
    // Preserve the Java indexed content lookup.
    #[allow(clippy::needless_range_loop)]
    // port: SourceMapGeneratorV3#addSourcesContentMap
    fn add_sources_content_map(&self, out: &mut JavaAppendable<'_>) -> fmt::Result {
        let mut found = false;
        let size = self.source_file_map.len();
        let mut contents = vec![JsString::default(); size];
        for entry in &self.source_file_map {
            let index = *entry.1;
            assert!((index as usize) < size, "java.lang.IllegalStateException");
            let content = self.source_file_content_map.get(entry.0);
            if let Some(Some(content)) = content {
                contents[index as usize] = content.clone();
                found = true;
            }
        }
        if !found {
            return Ok(());
        }
        Self::append_field_start(out, "sourcesContent")?;
        out.write_char('[')?;
        for i in 0..size {
            if i != 0 {
                out.write_char(',')?;
            }
            let source_content = &contents[i];
            out.append(&Self::escape_string(source_content))?;
        }
        out.write_char(']')?;
        Self::append_field_end(out);
        Ok(())
    }
    // port: SourceMapGeneratorV3#addSymbolNameMap
    fn add_symbol_name_map(&self, out: &mut JavaAppendable<'_>) -> fmt::Result {
        Self::add_name_map(out, &self.original_name_map)
    }
    // Preserve the Java statement order.
    #[allow(clippy::explicit_counter_loop)]
    // port: SourceMapGeneratorV3#addNameMap
    fn add_name_map(out: &mut JavaAppendable<'_>, map: &IndexMap<JsString, i32>) -> fmt::Result {
        let mut i = 0;
        for entry in map {
            let key = entry.0;
            if i != 0 {
                out.write_char(',')?;
            }
            out.append(&Self::escape_string(key))?;
            i += 1;
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3#escapeString
    fn escape_string(value: impl Into<JsString>) -> JsString {
        Util::escape_string(value)
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
    fn prep_mappings(&self) -> Result<i32, fmt::Error> {
        MappingTraversal::new(&self.mappings, self.prefix_position)
            .traverse(&mut UsedMappingCheck)?;
        let mut id = 0;
        let mut max_line = 0;
        for m in &self.mappings {
            if m.used.get() {
                m.id.set(id);
                id += 1;
                let end_position_line = m.end_position.get_line();
                max_line = max_line.max(end_position_line);
            }
        }
        Ok(max_line.wrapping_add(self.prefix_position.get_line()))
    }
    // port: SourceMapGeneratorV3#appendIndexMapTo
    pub fn append_index_map_to(
        &self,
        out: &mut dyn Write,
        name: impl Into<JsString>,
        sections: &[SourceMapSection],
    ) -> Result<JsString, fmt::Error> {
        let name = name.into();
        let mut appendable = JavaAppendable::new(out);
        let out = &mut appendable;
        out.write_str("{\n")?;
        Self::append_first_field(out, "version", "3")?;
        Self::append_field(out, "file", Self::escape_string(name))?;
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
                Self::append_field(out, "url", Self::escape_string(section.get_section_value()))?;
            } else if section.get_section_type() == SectionType::MAP {
                Self::append_field(out, "map", section.get_section_value())?;
            }
            // SectionType is exhaustive in Rust; Java's IOException fallback is unreachable.
            out.write_str("\n}")?;
        }
        out.write_str("\n]")?;
        Self::append_field_end(out);
        out.write_str("\n}\n")?;
        Ok(appendable.finish())
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

struct Mapping {
    id: Cell<i32>,
    source_file: Option<JsString>,
    original_position: FilePosition,
    start_position: FilePosition,
    end_position: FilePosition,
    original_name: Option<JsString>,
    used: Cell<bool>,
}
impl Mapping {
    // port: SourceMapGeneratorV3.Mapping#Mapping
    fn new() -> Self {
        Self {
            id: Cell::new(UNMAPPED),
            source_file: None,
            original_position: FilePosition::default(),
            start_position: FilePosition::default(),
            end_position: FilePosition::default(),
            original_name: None,
            used: Cell::new(false),
        }
    }
}

trait MappingVisitor {
    // port: SourceMapGeneratorV3.MappingVisitor#visit
    fn visit(
        &mut self,
        m: Option<&Mapping>,
        line: i32,
        col: i32,
        end_line: i32,
        end_col: i32,
    ) -> fmt::Result;
}
struct UsedMappingCheck;
impl MappingVisitor for UsedMappingCheck {
    // port: SourceMapGeneratorV3.UsedMappingCheck#visit
    fn visit(
        &mut self,
        m: Option<&Mapping>,
        _line: i32,
        _col: i32,
        _end_line: i32,
        _end_col: i32,
    ) -> fmt::Result {
        if let Some(m) = m {
            m.used.set(true);
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
    fn traverse(mut self, v: &mut dyn MappingVisitor) -> fmt::Result {
        let mut stack: Vec<&Mapping> = Vec::new();
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
    fn maybe_visit(&mut self, v: &mut dyn MappingVisitor, m: &Mapping) -> fmt::Result {
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
        v: &mut dyn MappingVisitor,
        parent: Option<&Mapping>,
        m: &Mapping,
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
        v: &mut dyn MappingVisitor,
        m: Option<&Mapping>,
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
struct LineMapper<'a, 'out> {
    out: &'a mut JavaAppendable<'out>,
    max_line: i32,
    previous_line: i32,
    previous_column: i32,
    previous_source_file_id: i32,
    previous_source_line: i32,
    previous_source_column: i32,
    previous_name_id: i32,
    source_file_map: &'a mut IndexMap<JsString, i32>,
    original_name_map: &'a mut IndexMap<JsString, i32>,
    last_source_file: &'a mut Option<JsString>,
    last_source_file_index: &'a mut i32,
}
impl MappingVisitor for LineMapper<'_, '_> {
    // port: SourceMapGeneratorV3.LineMapper#visit
    fn visit(
        &mut self,
        m: Option<&Mapping>,
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
                if self.previous_line == line {
                    self.out.write_char(',')?;
                }
                self.write_entry(m, col)?;
                self.previous_line = line;
                self.previous_column = col;
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
impl LineMapper<'_, '_> {
    // Borrowed outer generator fields are passed explicitly to preserve Rust's aliasing rules.
    // port: SourceMapGeneratorV3.LineMapper#LineMapper
    fn new<'a, 'out>(
        out: &'a mut JavaAppendable<'out>,
        max_line: i32,
        source_file_map: &'a mut IndexMap<JsString, i32>,
        original_name_map: &'a mut IndexMap<JsString, i32>,
        last_source_file: &'a mut Option<JsString>,
        last_source_file_index: &'a mut i32,
    ) -> LineMapper<'a, 'out> {
        LineMapper {
            out,
            max_line,
            previous_line: -1,
            previous_column: 0,
            previous_source_file_id: 0,
            previous_source_line: 0,
            previous_source_column: 0,
            previous_name_id: 0,
            source_file_map,
            original_name_map,
            last_source_file,
            last_source_file_index,
        }
    }
    // port: SourceMapGeneratorV3#getSourceId
    fn get_source_id(&mut self, source_name: &JsString) -> i32 {
        if self.last_source_file.as_ref() != Some(source_name) {
            *self.last_source_file = Some(source_name.clone());
            let index = self.source_file_map.get(source_name);
            if let Some(index) = index {
                *self.last_source_file_index = *index;
            } else {
                *self.last_source_file_index = self.source_file_map.len() as i32;
                self.source_file_map
                    .insert(source_name.clone(), *self.last_source_file_index);
            }
        }
        *self.last_source_file_index
    }
    // port: SourceMapGeneratorV3#getNameId
    fn get_name_id(&mut self, symbol_name: &JsString) -> i32 {
        let original_name_index;
        let index = self.original_name_map.get(symbol_name);
        if let Some(index) = index {
            original_name_index = *index;
        } else {
            original_name_index = self.original_name_map.len() as i32;
            self.original_name_map
                .insert(symbol_name.clone(), original_name_index);
        }
        original_name_index
    }
    // port: SourceMapGeneratorV3.LineMapper#writeEntry
    fn write_entry(&mut self, m: Option<&Mapping>, column: i32) -> fmt::Result {
        Base64VLQ::encode(self.out, column.wrapping_sub(self.previous_column))?;
        self.previous_column = column;
        if let Some(m) = m {
            let source_id = self.get_source_id(m.source_file.as_ref().unwrap());
            Base64VLQ::encode(
                self.out,
                source_id.wrapping_sub(self.previous_source_file_id),
            )?;
            self.previous_source_file_id = source_id;
            let srcline = m.original_position.get_line();
            let src_column = m.original_position.get_column();
            Base64VLQ::encode(self.out, srcline.wrapping_sub(self.previous_source_line))?;
            self.previous_source_line = srcline;
            Base64VLQ::encode(
                self.out,
                src_column.wrapping_sub(self.previous_source_column),
            )?;
            self.previous_source_column = src_column;
            if let Some(name) = &m.original_name {
                let name_id = self.get_name_id(name);
                Base64VLQ::encode(self.out, name_id.wrapping_sub(self.previous_name_id))?;
                self.previous_name_id = name_id;
            }
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3.LineMapper#appendLineMappings
    fn append_line_mappings(&mut self, traversal: MappingTraversal<'_>) -> fmt::Result {
        self.open_line(true)?;
        traversal.traverse(self)?;
        self.close_line(true)
    }
    // port: SourceMapGeneratorV3.LineMapper#openLine
    fn open_line(&mut self, first_entry: bool) -> fmt::Result {
        if first_entry {
            self.out.write_char('"')?;
        }
        Ok(())
    }
    // port: SourceMapGeneratorV3.LineMapper#closeLine
    fn close_line(&mut self, final_entry: bool) -> fmt::Result {
        self.out.write_char(';')?;
        if final_entry {
            self.out.write_char('"')?;
        }
        Ok(())
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
    units: Vec<u16>,
}
impl<'a> JavaAppendable<'a> {
    fn new(out: &'a mut dyn Write) -> Self {
        Self {
            out,
            units: Vec::new(),
        }
    }
    fn append(&mut self, s: &JsString) -> fmt::Result {
        self.units.extend_from_slice(s.as_units());
        write!(self.out, "{s}")
    }
    fn finish(self) -> JsString {
        JsString::from_units(self.units)
    }
}
impl Write for JavaAppendable<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Rust-only: the code units are appended directly, not through a JS string (D-025).
        self.units.extend(s.encode_utf16());
        self.out.write_str(s)
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        let mut buf = [0u16; 2];
        self.units.extend_from_slice(c.encode_utf16(&mut buf));
        self.out.write_char(c)
    }
}
