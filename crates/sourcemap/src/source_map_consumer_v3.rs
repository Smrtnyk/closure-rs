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
//   src/com/google/debugging/sourcemap/SourceMapConsumerV3.java.

use crate::{
    base64_vlq::{Base64VLQ, CharIterator},
    file_position::FilePosition,
    proto::mapping::{OriginalMapping, Precision},
    source_map_consumer::SourceMapConsumer,
    source_map_generator_v3::{ExtensionValue, SourceMapGeneratorV3},
    source_map_object::SourceMapObject,
    source_map_object_parser::SourceMapObjectParser,
    source_map_parse_exception::SourceMapParseException as Error,
    source_map_section::SectionType,
    source_map_supplier::SourceMapSupplier,
    source_mapping::SourceMapping,
    source_mapping_reversable::SourceMappingReversable,
};
use closure_rhino::js_string::JsString;
use indexmap::IndexMap;
const UNMAPPED: i32 = -1;
type ReverseSourceMapping = IndexMap<Option<JsString>, IndexMap<i32, Vec<OriginalMapping>>>;
#[derive(Default)]
pub struct SourceMapConsumerV3 {
    sources: Option<Vec<Option<JsString>>>,
    sources_content: Option<Vec<Option<JsString>>>,
    names: Option<Vec<Option<JsString>>>,
    file: Option<JsString>,
    line_count: i32,
    mappings: Option<Mappings>,
    reverse_source_mapping: Option<ReverseSourceMapping>,
    source_root: Option<JsString>,
    extensions: IndexMap<JsString, ExtensionValue>,
}
pub struct DefaultSourceMapSupplier;
impl SourceMapSupplier for DefaultSourceMapSupplier {
    // port: SourceMapConsumerV3.DefaultSourceMapSupplier#getSourceMap
    fn get_source_map(&self, _url: &JsString) -> Result<Option<JsString>, std::io::Error> {
        Ok(None)
    }
}
impl SourceMapConsumerV3 {
    // port: SourceMapConsumerV3#SourceMapConsumerV3
    pub fn new() -> Self {
        Self::default()
    }
    // port: SourceMapConsumerV3#parse(String)
    pub fn parse(&mut self, contents: impl Into<JsString>) -> Result<(), Error> {
        let source_map_object = SourceMapObjectParser::parse(contents)?;
        self.parse_object(source_map_object, None)
    }
    // port: SourceMapConsumerV3#parse(SourceMapObject,SourceMapSupplier)
    pub fn parse_object(
        &mut self,
        source_map_object: SourceMapObject,
        section_supplier: Option<&dyn SourceMapSupplier>,
    ) -> Result<(), Error> {
        if source_map_object.get_version() != 3 {
            return Err(Error::new(format!(
                "Unknown version: {}",
                source_map_object.get_version()
            )));
        }
        self.file = source_map_object.get_file();
        if self.file.as_ref().is_some_and(JsString::is_empty) {
            return Err(Error::new("File entry is empty"));
        }
        if source_map_object.get_sections().is_some() {
            self.parse_meta_map(source_map_object, section_supplier)?;
            return Ok(());
        }
        self.line_count = source_map_object.get_line_count();
        self.source_root = source_map_object.get_source_root();
        self.sources = source_map_object.get_sources().clone();
        self.sources_content = source_map_object.get_sources_content().clone();
        self.names = source_map_object.get_names().clone();
        let sources = self.sources.as_ref().unwrap_or_else(|| panic!("java.lang.NullPointerException: Cannot read the array length because \"this.sources\" is null"));
        let use_compact_mappings =
            sources.len() < 65535 && self.names.as_ref().is_none_or(|names| names.len() < 65535);
        let mut estimated_entries = if self.line_count >= 0 {
            self.line_count.wrapping_mul(2)
        } else {
            1000
        };
        if let Some(mappings) = source_map_object.get_mappings() {
            estimated_entries = estimated_entries.max((mappings.length() / 6) as i32);
        }
        self.extensions
            .extend(source_map_object.get_extensions().clone());
        let line_map = source_map_object.get_mappings().unwrap_or_else(|| panic!("java.lang.NullPointerException: Cannot invoke \"String.length()\" because \"content\" is null"));
        self.mappings = Some(
            MappingBuilder::new(
                line_map,
                self.line_count,
                use_compact_mappings,
                estimated_entries,
                sources,
                self.names.as_deref(),
            )
            .build()?,
        );
        Ok(())
    }
    // port: SourceMapConsumerV3#parseMetaMap
    fn parse_meta_map(
        &mut self,
        source_map_object: SourceMapObject,
        section_supplier: Option<&dyn SourceMapSupplier>,
    ) -> Result<(), Error> {
        // Reborrow the parameter so the replacement can use a local lifetime.
        let mut section_supplier = section_supplier;
        let default_source_map_supplier;
        if section_supplier.is_none() {
            default_source_map_supplier = DefaultSourceMapSupplier;
            section_supplier = Some(&default_source_map_supplier);
        }
        if source_map_object.get_line_count() >= 0
            || source_map_object.get_mappings().is_some()
            || source_map_object
                .get_sources()
                .as_ref()
                .is_some_and(|sources| !sources.is_empty())
            || source_map_object.get_names().is_some()
        {
            return Err(Error::new("Invalid map format"));
        }
        let mut generator = SourceMapGeneratorV3::new();
        for section in source_map_object.get_sections().as_ref().unwrap() {
            let mut map_section_contents = Some(section.get_section_value().clone());
            if section.get_section_type() == SectionType::URL {
                map_section_contents = section_supplier
                    .unwrap()
                    .get_source_map(section.get_section_value())
                    .map_err(|ex| Error::new(format!("IO exception: java.io.IOException: {ex}")))?;
            }
            if map_section_contents.is_none() {
                return Err(Error::new(
                    JsString::from("Unable to retrieve: ").concat(section.get_section_value()),
                ));
            }
            generator.merge_map_section(
                section.get_line(),
                section.get_column(),
                map_section_contents.unwrap(),
            )?;
        }
        let mut sb = String::new();
        let contents = generator
            .append_to(&mut sb, source_map_object.get_file())
            .unwrap();
        self.parse(contents)
    }
    // port: SourceMapConsumerV3#getMappingForLine
    pub fn get_mapping_for_line(&self, line_number: i32, column: i32) -> Option<OriginalMapping> {
        let line_number = line_number.wrapping_sub(1);
        let column = column.wrapping_sub(1);
        let m = self.mappings.as_ref().unwrap();
        if line_number < 0 || line_number >= m.get_parsed_line_count() {
            return None;
        }
        let start = m.get_line_start(line_number);
        let end = m.get_line_start(line_number + 1);
        if start == end {
            return self.get_previous_mapping(line_number);
        }
        if m.get_generated_column(start) > column {
            return self.get_previous_mapping(line_number);
        }
        let index = self.search(column, start, end - m.get_entry_size());
        assert!(
            index >= 0,
            "java.lang.IllegalStateException: unexpected:{index}"
        );
        self.get_original_mapping_for_entry(index, Precision::EXACT)
    }
    // port: SourceMapConsumerV3#getOriginalSources
    pub fn get_original_sources(&self) -> &[Option<JsString>] {
        self.sources
            .as_deref()
            .unwrap_or_else(|| panic!("java.lang.NullPointerException"))
    }
    // port: SourceMapConsumerV3#getOriginalSourcesContent
    pub fn get_original_sources_content(&self) -> Option<&[Option<JsString>]> {
        self.sources_content.as_deref()
    }
    // port: SourceMapConsumerV3#getOriginalNames
    pub fn get_original_names(&self) -> &[Option<JsString>] {
        self.names
            .as_deref()
            .unwrap_or_else(|| panic!("java.lang.NullPointerException"))
    }
    // port: SourceMapConsumerV3#getFile
    pub fn get_file(&self) -> Option<JsString> {
        self.file.clone()
    }
    // port: SourceMapConsumerV3#getLineCount
    pub fn get_line_count(&self) -> i32 {
        self.line_count
    }
    // port: SourceMapConsumerV3#getSourceRoot
    pub fn get_source_root(&self) -> Option<JsString> {
        self.source_root.clone()
    }
    // port: SourceMapConsumerV3#getExtensions
    pub fn get_extensions(&self) -> &IndexMap<JsString, ExtensionValue> {
        &self.extensions
    }
    // Preserve the Java statement order.
    #[allow(clippy::unnecessary_unwrap)]
    // port: SourceMapConsumerV3#getReverseMapping
    pub fn get_reverse_mapping(
        &mut self,
        original_file: Option<JsString>,
        line: i32,
        _column: i32,
    ) -> &[OriginalMapping] {
        if self.reverse_source_mapping.is_none() {
            self.create_reverse_mapping();
        }
        let source_line_to_collection_map = self
            .reverse_source_mapping
            .as_ref()
            .unwrap()
            .get(&original_file);
        if source_line_to_collection_map.is_none() {
            &[]
        } else {
            let mappings = source_line_to_collection_map.unwrap().get(&line);
            if mappings.is_none() {
                &[]
            } else {
                mappings.unwrap()
            }
        }
    }
    // port: SourceMapConsumerV3#search
    fn search(&self, target: i32, mut start: i32, mut end: i32) -> i32 {
        let m = self.mappings.as_ref().unwrap();
        let entry_size = m.get_entry_size();
        loop {
            let mid = ((end - start) / (entry_size * 2)) * entry_size + start;
            let compare = m.get_generated_column(mid).wrapping_sub(target);
            if compare == 0 {
                return mid;
            } else if compare < 0 {
                start = mid + entry_size;
                if start > end {
                    return end;
                }
            } else {
                end = mid - entry_size;
                if end < start {
                    return end;
                }
            }
        }
    }
    // port: SourceMapConsumerV3#getPreviousMapping
    fn get_previous_mapping(&self, mut line_number: i32) -> Option<OriginalMapping> {
        let mappings = self.mappings.as_ref().unwrap();
        loop {
            if line_number == 0 {
                return None;
            }
            line_number -= 1;
            if mappings.get_line_start(line_number) != mappings.get_line_start(line_number + 1) {
                break;
            }
        }
        let index = mappings.get_line_start(line_number + 1) - mappings.get_entry_size();
        self.get_original_mapping_for_entry(index, Precision::APPROXIMATE_LINE)
    }
    // port: SourceMapConsumerV3#getOriginalMappingForEntry
    fn get_original_mapping_for_entry(
        &self,
        index: i32,
        precision: Precision,
    ) -> Option<OriginalMapping> {
        let mappings = self.mappings.as_ref().unwrap();
        let source_file_id = mappings.get_source_file_id(index);
        if source_file_id == UNMAPPED {
            None
        } else {
            let mut x = OriginalMapping::new_builder();
            x.set_original_file(
                array_get(self.sources.as_ref().unwrap(), source_file_id)
                    .as_ref()
                    .unwrap_or_else(|| panic!("java.lang.NullPointerException")),
            )
            .set_line_number(mappings.get_source_line(index).wrapping_add(1))
            .set_column_position(mappings.get_source_column(index).wrapping_add(1))
            .set_precision(precision);
            let name_id = mappings.get_name_id(index);
            if name_id != UNMAPPED {
                x.set_identifier(
                    array_get(self.names.as_ref().unwrap(), name_id)
                        .as_ref()
                        .unwrap_or_else(|| panic!("java.lang.NullPointerException")),
                );
            }
            Some(x.build())
        }
    }
    // Preserve the Java statement order.
    #[allow(clippy::unwrap_or_default)]
    // port: SourceMapConsumerV3#createReverseMapping
    fn create_reverse_mapping(&mut self) {
        self.reverse_source_mapping = Some(IndexMap::new());
        let mappings = self.mappings.as_ref().unwrap();
        for target_line in 0..mappings.get_parsed_line_count() {
            let start = mappings.get_line_start(target_line);
            let end = mappings.get_line_start(target_line + 1);
            let mut i = start;
            while i < end {
                let source_file_id = mappings.get_source_file_id(i);
                let source_line = mappings.get_source_line(i);
                if source_file_id != UNMAPPED && source_line != UNMAPPED {
                    let original_file =
                        array_get(self.sources.as_ref().unwrap(), source_file_id).clone();
                    let line_to_collection_map = self
                        .reverse_source_mapping
                        .as_mut()
                        .unwrap()
                        .entry(original_file)
                        .or_insert_with(IndexMap::new);
                    if !line_to_collection_map.contains_key(&source_line) {
                        line_to_collection_map.insert(source_line, Vec::with_capacity(1));
                    }
                    let mappings_line = line_to_collection_map.get_mut(&source_line).unwrap();
                    let mut builder = OriginalMapping::new_builder();
                    builder
                        .set_line_number(target_line)
                        .set_column_position(mappings.get_generated_column(i));
                    mappings_line.push(builder.build());
                }
                i += mappings.get_entry_size();
            }
        }
    }
    // port: SourceMapConsumerV3#visitMappings
    pub fn visit_mappings(&self, visitor: &mut dyn EntryVisitor) {
        let mut pending = false;
        let mut source_name = None;
        let mut symbol_name = None;
        let mut source_start_position = None;
        let mut start_position: Option<FilePosition> = None;
        let mappings = self.mappings.as_ref().unwrap();
        for i in 0..mappings.get_parsed_line_count() {
            let start = mappings.get_line_start(i);
            let end = mappings.get_line_start(i + 1);
            if start != end {
                let mut j = start;
                while j < end {
                    if pending {
                        let end_position = FilePosition::new(i, mappings.get_generated_column(j));
                        visitor.visit(
                            source_name,
                            symbol_name,
                            source_start_position.unwrap(),
                            start_position.unwrap(),
                            end_position,
                        );
                        pending = false;
                    }
                    let source_file_id = mappings.get_source_file_id(j);
                    if source_file_id != UNMAPPED {
                        pending = true;
                        source_name =
                            array_get(self.sources.as_ref().unwrap(), source_file_id).as_ref();
                        let name_id = mappings.get_name_id(j);
                        symbol_name = if name_id != UNMAPPED {
                            array_get(self.names.as_ref().unwrap(), name_id).as_ref()
                        } else {
                            None
                        };
                        source_start_position = Some(FilePosition::new(
                            mappings.get_source_line(j),
                            mappings.get_source_column(j),
                        ));
                        start_position =
                            Some(FilePosition::new(i, mappings.get_generated_column(j)));
                    }
                    j += mappings.get_entry_size();
                }
            }
        }
        if pending {
            let start_position = start_position.unwrap();
            let end_position = FilePosition::new(
                start_position.get_line(),
                start_position.get_column().wrapping_add(1),
            );
            visitor.visit(
                source_name,
                symbol_name,
                source_start_position.unwrap(),
                start_position,
                end_position,
            );
        }
    }
}

pub trait EntryVisitor {
    // port: SourceMapConsumerV3.EntryVisitor#visit
    fn visit(
        &mut self,
        source_name: Option<&JsString>,
        symbol_name: Option<&JsString>,
        source_start_position: FilePosition,
        start_position: FilePosition,
        end_position: FilePosition,
    );
}
impl<F: FnMut(Option<&JsString>, Option<&JsString>, FilePosition, FilePosition, FilePosition)>
    EntryVisitor for F
{
    // Rust closure adapter.
    fn visit(
        &mut self,
        s: Option<&JsString>,
        n: Option<&JsString>,
        o: FilePosition,
        b: FilePosition,
        e: FilePosition,
    ) {
        self(s, n, o, b, e)
    }
}
impl SourceMapping for SourceMapConsumerV3 {
    // Rust trait dispatch to the single Java method body.
    fn get_mapping_for_line(&self, l: i32, c: i32) -> Option<OriginalMapping> {
        self.get_mapping_for_line(l, c)
    }
}
impl SourceMapConsumer for SourceMapConsumerV3 {
    // Rust trait dispatch to the single Java method body.
    fn parse(&mut self, c: &JsString) -> Result<(), Error> {
        self.parse(c)
    }
}
impl SourceMappingReversable for SourceMapConsumerV3 {
    // Rust trait dispatch to the single Java method body.
    fn get_original_sources(&self) -> &[Option<JsString>] {
        self.get_original_sources()
    }
    // Rust trait dispatch to the single Java method body.
    fn get_reverse_mapping(&mut self, s: Option<&JsString>, l: i32, c: i32) -> &[OriginalMapping] {
        self.get_reverse_mapping(s.cloned(), l, c)
    }
}
// Trivial array access bridge preserving Java exception text.
fn array_get<T>(array: &[T], index: i32) -> &T {
    array.get(index as usize).unwrap_or_else(|| {
        panic!(
            "java.lang.ArrayIndexOutOfBoundsException: Index {index} out of bounds for length {}",
            array.len()
        )
    })
}
pub struct StringCharIterator {
    content: JsString,
    length: usize,
    current: usize,
}
impl StringCharIterator {
    // port: SourceMapConsumerV3.StringCharIterator#StringCharIterator
    pub fn new(content: impl Into<JsString>) -> Self {
        let content = content.into();
        let length = content.length();
        Self {
            content,
            length,
            current: 0,
        }
    }
    // port: SourceMapConsumerV3.StringCharIterator#peek
    fn peek(&self) -> u16 {
        self.content.char_at(self.current)
    }
}
impl CharIterator for StringCharIterator {
    // port: SourceMapConsumerV3.StringCharIterator#next
    fn next(&mut self) -> u16 {
        let index = self.current;
        self.current += 1;
        *self.content.as_units().get(index).unwrap_or_else(||panic!("java.lang.StringIndexOutOfBoundsException: Index {index} out of bounds for length {}",self.content.length()))
    }
    // port: SourceMapConsumerV3.StringCharIterator#hasNext
    fn has_next(&self) -> bool {
        self.current < self.length
    }
}
struct Mappings {
    flat_entries: Vec<i32>,
    line_start: Vec<i32>,
    line_count: i32,
    entry_size: i32,
}
impl Mappings {
    // port: SourceMapConsumerV3.Mappings#Mappings
    fn new(flat_entries: Vec<i32>, line_start: Vec<i32>, line_count: i32, entry_size: i32) -> Self {
        Self {
            flat_entries,
            line_start,
            line_count,
            entry_size,
        }
    }
    // port: SourceMapConsumerV3.Mappings#getEntrySize
    fn get_entry_size(&self) -> i32 {
        self.entry_size
    }
    // port: SourceMapConsumerV3.Mappings#getGeneratedColumn
    fn get_generated_column(&self, index: i32) -> i32 {
        *array_get(&self.flat_entries, index)
    }
    // port: SourceMapConsumerV3.Mappings#getSourceFileId
    fn get_source_file_id(&self, index: i32) -> i32 {
        if self.entry_size == 4 {
            let val = *array_get(&self.flat_entries, index + 3) >> 16;
            return if val == -1 || (val & 0xffff) == 0xffff {
                UNMAPPED
            } else {
                val & 0xffff
            };
        }
        *array_get(&self.flat_entries, index + 3)
    }
    // port: SourceMapConsumerV3.Mappings#getSourceLine
    fn get_source_line(&self, index: i32) -> i32 {
        *array_get(&self.flat_entries, index + 1)
    }
    // port: SourceMapConsumerV3.Mappings#getSourceColumn
    fn get_source_column(&self, index: i32) -> i32 {
        *array_get(&self.flat_entries, index + 2)
    }
    // port: SourceMapConsumerV3.Mappings#getNameId
    fn get_name_id(&self, index: i32) -> i32 {
        if self.entry_size == 4 {
            let val = *array_get(&self.flat_entries, index + 3) & 0xffff;
            return if val == 0xffff { UNMAPPED } else { val };
        }
        *array_get(&self.flat_entries, index + 4)
    }
    // port: SourceMapConsumerV3.Mappings#getParsedLineCount
    fn get_parsed_line_count(&self) -> i32 {
        self.line_count
    }
    // port: SourceMapConsumerV3.Mappings#getLineStart
    fn get_line_start(&self, line: i32) -> i32 {
        *array_get(&self.line_start, line)
    }
}
struct MappingBuilder<'a> {
    content: StringCharIterator,
    flat_entries: Vec<i32>,
    line_start: Vec<i32>,
    line_count: i32,
    entry_size: i32,
    line: i32,
    previous_col: i32,
    previous_src_id: i32,
    previous_src_line: i32,
    previous_src_column: i32,
    previous_name_id: i32,
    // Borrowed fields of Java's enclosing SourceMapConsumerV3.
    sources: &'a [Option<JsString>],
    names: Option<&'a [Option<JsString>]>,
}
impl<'a> MappingBuilder<'a> {
    const MAX_ENTRY_VALUES: usize = 5;
    // port: SourceMapConsumerV3.MappingBuilder#MappingBuilder
    fn new(
        line_map: JsString,
        line_count: i32,
        use_compact_mappings: bool,
        estimated_entries: i32,
        sources: &'a [Option<JsString>],
        names: Option<&'a [Option<JsString>]>,
    ) -> Self {
        let content = StringCharIterator::new(line_map);
        let entry_size = if use_compact_mappings { 4 } else { 5 };
        let length = estimated_entries.wrapping_mul(entry_size);
        if length < 0 {
            panic!("java.lang.NegativeArraySizeException: {length}");
        }
        let flat_entries = vec![0; length as usize];
        let estimated_lines = if line_count >= 0 { line_count } else { 1000 };
        let length = estimated_lines.wrapping_add(1);
        if length < 0 {
            panic!("java.lang.NegativeArraySizeException: {length}");
        }
        let line_start = vec![0; length as usize];
        Self {
            content,
            line_count,
            entry_size,
            flat_entries,
            line_start,
            line: 0,
            previous_col: 0,
            previous_src_id: 0,
            previous_src_line: 0,
            previous_src_column: 0,
            previous_name_id: 0,
            sources,
            names,
        }
    }
    // port: SourceMapConsumerV3.MappingBuilder#build
    fn build(mut self) -> Result<Mappings, Error> {
        let mut temp = [0; Self::MAX_ENTRY_VALUES];
        let mut entries_count = 0;
        let mut lines_count = 0;
        self.set_line_start(0, 0);
        while self.content.has_next() {
            if self.try_consume_token(b';' as u16) {
                lines_count += 1;
                self.set_line_start(lines_count, entries_count);
                self.line += 1;
                self.previous_col = 0;
            } else {
                let mut entry_values = 0;
                while !self.entry_complete() {
                    let value = self.next_value();
                    *array_get_mut(&mut temp, entry_values) = value;
                    entry_values += 1;
                }
                self.set_entry(
                    entries_count,
                    &temp,
                    entry_values,
                    self.previous_col,
                    self.previous_src_id,
                    self.previous_src_line,
                    self.previous_src_column,
                    self.previous_name_id,
                )?;
                self.validate_entry(entries_count, self.line);
                self.previous_col = self.get_generated_column(entries_count);
                if entry_values >= 4 {
                    self.previous_src_id = self.get_source_file_id(entries_count);
                    self.previous_src_line = self.get_source_line(entries_count);
                    self.previous_src_column = self.get_source_column(entries_count);
                }
                if entry_values == 5 {
                    self.previous_name_id = self.get_name_id(entries_count);
                }
                entries_count += self.entry_size;
                self.try_consume_token(b',' as u16);
            }
        }
        if entries_count > self.get_line_start(lines_count) {
            lines_count += 1;
            self.set_line_start(lines_count, entries_count);
        }
        self.trim(entries_count, lines_count);
        Ok(Mappings::new(
            self.flat_entries,
            self.line_start,
            self.line_count,
            self.entry_size,
        ))
    }
    // port: SourceMapConsumerV3.MappingBuilder#getLineStart
    fn get_line_start(&self, line: i32) -> i32 {
        *array_get(&self.line_start, line)
    }
    // port: SourceMapConsumerV3.MappingBuilder#setLineStart
    fn set_line_start(&mut self, line: i32, entries_count: i32) {
        if line >= self.line_start.len() as i32 {
            self.line_start.resize(self.line_start.len() * 2, 0);
        }
        *array_get_mut(&mut self.line_start, line) = entries_count;
    }
    // port: SourceMapConsumerV3.MappingBuilder#setEntry
    #[allow(clippy::too_many_arguments)]
    fn set_entry(
        &mut self,
        entries_count: i32,
        vals: &[i32; 5],
        entry_values: i32,
        previous_col: i32,
        previous_src_id: i32,
        previous_src_line: i32,
        previous_src_column: i32,
        previous_name_id: i32,
    ) -> Result<(), Error> {
        if entries_count + self.entry_size > self.flat_entries.len() as i32 {
            self.flat_entries.resize(self.flat_entries.len() * 2, 0);
        }
        if self.entry_size == 4 {
            self.set_entry_compact(
                entries_count,
                vals,
                entry_values,
                previous_col,
                previous_src_id,
                previous_src_line,
                previous_src_column,
                previous_name_id,
            )?;
        } else {
            self.set_entry_normal(
                entries_count,
                vals,
                entry_values,
                previous_col,
                previous_src_id,
                previous_src_line,
                previous_src_column,
                previous_name_id,
            )?;
        }
        Ok(())
    }
    // port: SourceMapConsumerV3.MappingBuilder#setEntryCompact
    #[allow(clippy::too_many_arguments)]
    fn set_entry_compact(
        &mut self,
        entries_count: i32,
        vals: &[i32; 5],
        entry_values: i32,
        previous_col: i32,
        previous_src_id: i32,
        previous_src_line: i32,
        previous_src_column: i32,
        previous_name_id: i32,
    ) -> Result<(), Error> {
        match entry_values {
            1 => {
                *array_get_mut(&mut self.flat_entries, entries_count) =
                    vals[0].wrapping_add(previous_col);
                *array_get_mut(&mut self.flat_entries, entries_count + 1) = UNMAPPED;
                *array_get_mut(&mut self.flat_entries, entries_count + 2) = UNMAPPED;
                *array_get_mut(&mut self.flat_entries, entries_count + 3) = UNMAPPED;
            }
            4 => {
                *array_get_mut(&mut self.flat_entries, entries_count) =
                    vals[0].wrapping_add(previous_col);
                *array_get_mut(&mut self.flat_entries, entries_count + 1) =
                    vals[2].wrapping_add(previous_src_line);
                *array_get_mut(&mut self.flat_entries, entries_count + 2) =
                    vals[3].wrapping_add(previous_src_column);
                let src_id = vals[1].wrapping_add(previous_src_id);
                *array_get_mut(&mut self.flat_entries, entries_count + 3) =
                    src_id.wrapping_shl(16) | 0xffff;
            }
            5 => {
                *array_get_mut(&mut self.flat_entries, entries_count) =
                    vals[0].wrapping_add(previous_col);
                *array_get_mut(&mut self.flat_entries, entries_count + 1) =
                    vals[2].wrapping_add(previous_src_line);
                *array_get_mut(&mut self.flat_entries, entries_count + 2) =
                    vals[3].wrapping_add(previous_src_column);
                let src_id = vals[1].wrapping_add(previous_src_id);
                let name_id = vals[4].wrapping_add(previous_name_id);
                *array_get_mut(&mut self.flat_entries, entries_count + 3) =
                    src_id.wrapping_shl(16) | (name_id & 0xffff);
            }
            _ => {
                return Err(Error::new(format!(
                    "Unexpected number of values for entry:{entry_values}"
                )));
            }
        }
        Ok(())
    }
    // port: SourceMapConsumerV3.MappingBuilder#setEntryNormal
    #[allow(clippy::too_many_arguments)]
    fn set_entry_normal(
        &mut self,
        entries_count: i32,
        vals: &[i32; 5],
        entry_values: i32,
        previous_col: i32,
        previous_src_id: i32,
        previous_src_line: i32,
        previous_src_column: i32,
        previous_name_id: i32,
    ) -> Result<(), Error> {
        match entry_values {
            1 => {
                *array_get_mut(&mut self.flat_entries, entries_count) =
                    vals[0].wrapping_add(previous_col);
                *array_get_mut(&mut self.flat_entries, entries_count + 1) = UNMAPPED;
                *array_get_mut(&mut self.flat_entries, entries_count + 2) = UNMAPPED;
                *array_get_mut(&mut self.flat_entries, entries_count + 3) = UNMAPPED;
                *array_get_mut(&mut self.flat_entries, entries_count + 4) = UNMAPPED;
            }
            4 => {
                *array_get_mut(&mut self.flat_entries, entries_count) =
                    vals[0].wrapping_add(previous_col);
                *array_get_mut(&mut self.flat_entries, entries_count + 1) =
                    vals[2].wrapping_add(previous_src_line);
                *array_get_mut(&mut self.flat_entries, entries_count + 2) =
                    vals[3].wrapping_add(previous_src_column);
                *array_get_mut(&mut self.flat_entries, entries_count + 3) =
                    vals[1].wrapping_add(previous_src_id);
                *array_get_mut(&mut self.flat_entries, entries_count + 4) = UNMAPPED;
            }
            5 => {
                *array_get_mut(&mut self.flat_entries, entries_count) =
                    vals[0].wrapping_add(previous_col);
                *array_get_mut(&mut self.flat_entries, entries_count + 1) =
                    vals[2].wrapping_add(previous_src_line);
                *array_get_mut(&mut self.flat_entries, entries_count + 2) =
                    vals[3].wrapping_add(previous_src_column);
                *array_get_mut(&mut self.flat_entries, entries_count + 3) =
                    vals[1].wrapping_add(previous_src_id);
                *array_get_mut(&mut self.flat_entries, entries_count + 4) =
                    vals[4].wrapping_add(previous_name_id);
            }
            _ => {
                return Err(Error::new(format!(
                    "Unexpected number of values for entry:{entry_values}"
                )));
            }
        }
        Ok(())
    }
    // port: SourceMapConsumerV3.MappingBuilder#getGeneratedColumn
    fn get_generated_column(&self, index: i32) -> i32 {
        *array_get(&self.flat_entries, index)
    }
    // port: SourceMapConsumerV3.MappingBuilder#getSourceFileId
    fn get_source_file_id(&self, index: i32) -> i32 {
        if self.entry_size == 4 {
            let val = *array_get(&self.flat_entries, index + 3) >> 16;
            return if val == -1 || (val & 0xffff) == 0xffff {
                UNMAPPED
            } else {
                val & 0xffff
            };
        }
        *array_get(&self.flat_entries, index + 3)
    }
    // port: SourceMapConsumerV3.MappingBuilder#getSourceLine
    fn get_source_line(&self, index: i32) -> i32 {
        *array_get(&self.flat_entries, index + 1)
    }
    // port: SourceMapConsumerV3.MappingBuilder#getSourceColumn
    fn get_source_column(&self, index: i32) -> i32 {
        *array_get(&self.flat_entries, index + 2)
    }
    // port: SourceMapConsumerV3.MappingBuilder#getNameId
    fn get_name_id(&self, index: i32) -> i32 {
        if self.entry_size == 4 {
            let val = *array_get(&self.flat_entries, index + 3) & 0xffff;
            return if val == 0xffff { UNMAPPED } else { val };
        }
        *array_get(&self.flat_entries, index + 4)
    }
    // port: SourceMapConsumerV3.MappingBuilder#validateEntry
    fn validate_entry(&self, entries_count: i32, line: i32) {
        assert!(
            self.line_count < 0 || line < self.line_count,
            "java.lang.IllegalStateException: line={line}, lineCount={}",
            self.line_count
        );
        let source_file_id = self.get_source_file_id(entries_count);
        let name_id = self.get_name_id(entries_count);
        assert!(
            source_file_id == UNMAPPED || source_file_id < self.sources.len() as i32,
            "java.lang.IllegalStateException"
        );
        assert!(name_id == UNMAPPED || name_id < self.names.unwrap_or_else(|| panic!("java.lang.NullPointerException: Cannot read the array length because \"this.this$0.names\" is null")).len() as i32, "java.lang.IllegalStateException");
    }
    // port: SourceMapConsumerV3.MappingBuilder#trim
    fn trim(&mut self, entries_count: i32, lines_count: i32) {
        if self.flat_entries.len() > entries_count as usize {
            self.flat_entries.truncate(entries_count as usize);
        }
        if self.line_start.len() > lines_count as usize + 1 {
            self.line_start.truncate(lines_count as usize + 1);
        }
        self.line_count = lines_count;
    }
    // port: SourceMapConsumerV3.MappingBuilder#tryConsumeToken
    fn try_consume_token(&mut self, token: u16) -> bool {
        if self.content.has_next() && self.content.peek() == token {
            self.content.next();
            return true;
        }
        false
    }
    // port: SourceMapConsumerV3.MappingBuilder#entryComplete
    fn entry_complete(&self) -> bool {
        if !self.content.has_next() {
            return true;
        }
        let c = self.content.peek();
        c == b';' as u16 || c == b',' as u16
    }
    // port: SourceMapConsumerV3.MappingBuilder#nextValue
    fn next_value(&mut self) -> i32 {
        Base64VLQ::decode(&mut self.content)
    }
}
// Trivial mutable array access bridge preserving Java exception text.
fn array_get_mut<T>(array: &mut [T], index: i32) -> &mut T {
    let length = array.len();
    array.get_mut(index as usize).unwrap_or_else(|| panic!("java.lang.ArrayIndexOutOfBoundsException: Index {index} out of bounds for length {length}"))
}
