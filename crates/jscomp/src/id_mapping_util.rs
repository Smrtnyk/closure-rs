/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/IdMappingUtil.java.

//! Port of `IdMappingUtil.java`.

use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;

/// A utility class for generating and parsing id mappings held by `ReplaceIdGenerators`.
pub struct IdMappingUtil;

// port: IdMappingUtil#NEW_LINE
pub const NEW_LINE: char = '\n';

/// Guava's `HashBiMap<String, String>` (insertion-ordered, with an inverse view): `put` throws
/// `IllegalArgumentException` when the value is already bound to a different key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BiMap {
    forward: IndexMap<JsString, JsString>,
    backward: IndexMap<JsString, JsString>,
}

impl BiMap {
    // port: HashBiMap#create
    pub fn new() -> Self {
        Self::default()
    }
    // port: HashBiMap#put
    pub fn put(&mut self, key: JsString, value: JsString) {
        if let Some(old_key) = self.backward.get(&value) {
            if *old_key == key {
                return;
            }
            panic!("IllegalArgumentException: value already present: {value}");
        }
        if let Some(old_value) = self.forward.insert(key.clone(), value.clone()) {
            self.backward.shift_remove(&old_value);
        }
        self.backward.insert(value, key);
    }
    // port: HashBiMap#get
    pub fn get(&self, key: &JsString) -> Option<&JsString> {
        self.forward.get(key)
    }
    // port: HashBiMap#size
    pub fn size(&self) -> usize {
        self.forward.len()
    }
    // port: HashBiMap#isEmpty
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }
    // port: HashBiMap#keySet
    pub fn key_set(&self) -> IndexSet<JsString> {
        self.forward.keys().cloned().collect()
    }
    // port: HashBiMap#inverse
    pub fn inverse(&self) -> IndexMap<JsString, JsString> {
        self.backward.clone()
    }
    // port: HashBiMap#entrySet
    pub fn entries(&self) -> impl Iterator<Item = (&JsString, &JsString)> {
        self.forward.iter()
    }
}

/// `java.io.BufferedReader#readLine` over UTF-16 text: lines end at `\n`, `\r` or `\r\n`.
struct LineReader {
    text: Vec<u16>,
    pos: usize,
}

impl LineReader {
    fn new(text: &JsString) -> Self {
        Self {
            text: text.as_units().to_vec(),
            pos: 0,
        }
    }
    // port: BufferedReader#readLine
    fn read_line(&mut self) -> Option<JsString> {
        if self.pos >= self.text.len() {
            return None;
        }
        let start = self.pos;
        while self.pos < self.text.len() {
            let c = self.text[self.pos];
            if c == u16::from(b'\n') || c == u16::from(b'\r') {
                let line = JsString::from_units(&self.text[start..self.pos]);
                self.pos += 1;
                if c == u16::from(b'\r')
                    && self.pos < self.text.len()
                    && self.text[self.pos] == u16::from(b'\n')
                {
                    self.pos += 1;
                }
                return Some(line);
            }
            self.pos += 1;
        }
        Some(JsString::from_units(&self.text[start..]))
    }
}

/// A stateful pull parser for reading id mapping sections and entries line by line.
pub struct MappingReader {
    reader: LineReader,
    current_section: Option<JsString>,
    current_key: Option<JsString>,
    current_value: Option<JsString>,
    line_index: i32,
}

impl MappingReader {
    // port: IdMappingUtil.MappingReader#MappingReader
    fn new(reader: LineReader) -> Self {
        Self {
            reader,
            current_section: None,
            current_key: None,
            current_value: None,
            line_index: 0,
        }
    }

    // port: IdMappingUtil.MappingReader#next
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        while let Some(line) = self.reader.read_line() {
            self.line_index += 1;
            if line.is_empty() {
                continue;
            }
            if line.char_at(0) == u16::from(b'[') {
                self.current_section = Some(line.substring(1, line.length() - 1));
                self.current_key = None;
                self.current_value = None;
                return true;
            } else {
                let split = line.index_of_char(u16::from(b':'));
                if split != -1 {
                    let split = split as usize;
                    self.current_key = Some(line.substring(0, split));
                    self.current_value = Some(line.substring_from(split + 1));
                    return true;
                } else {
                    panic!(
                        "IllegalArgumentException: {}",
                        format_args!(
                            "Cannot parse id map.\n Line: {}, lineIndex: {}",
                            line, self.line_index
                        )
                    );
                }
            }
        }
        false
    }

    // port: IdMappingUtil.MappingReader#isSection
    pub fn is_section(&self) -> bool {
        self.current_key.is_none()
    }

    // port: IdMappingUtil.MappingReader#getSection
    pub fn get_section(&self) -> Option<&JsString> {
        self.current_section.as_ref()
    }

    // port: IdMappingUtil.MappingReader#getKey
    pub fn get_key(&self) -> Option<&JsString> {
        self.current_key.as_ref()
    }

    // port: IdMappingUtil.MappingReader#getValue
    pub fn get_value(&self) -> Option<&JsString> {
        self.current_value.as_ref()
    }

    // port: IdMappingUtil.MappingReader#getLineIndex
    pub fn get_line_index(&self) -> i32 {
        self.line_index
    }
}

impl IdMappingUtil {
    /// Returns the serialize map of generators and their ids and their replacements.
    // port: IdMappingUtil#generateSerializedIdMappings
    pub fn generate_serialized_id_mappings(
        id_generator_maps: &IndexMap<Option<JsString>, IndexMap<JsString, JsString>>,
    ) -> String {
        let mut sb = String::new();
        for (key, value) in id_generator_maps {
            if !value.is_empty() {
                sb.push('[');
                match key {
                    Some(key) => sb.push_str(&key.to_string()),
                    None => sb.push_str("null"),
                }
                sb.push(']');
                sb.push(NEW_LINE);
                sb.push(NEW_LINE);

                for (replacement_key, replacement_value) in value {
                    sb.push_str(&replacement_key.to_string());
                    sb.push(':');
                    sb.push_str(&replacement_value.to_string());
                    sb.push(NEW_LINE);
                }
                sb.push(NEW_LINE);
            }
        }
        sb
    }

    /// The `InputStream` is read as UTF-8, as Java's `InputStreamReader(stream, UTF_8)`.
    // port: IdMappingUtil#parseSectionAsStream
    pub fn parse_section_as_stream(
        stream: &mut dyn std::io::Read,
        section_filter: &str,
    ) -> std::io::Result<IndexMap<JsString, JsString>> {
        let mut map_builder: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();

        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        let br = LineReader::new(&JsString::from(
            String::from_utf8_lossy(&bytes).into_owned(),
        ));
        let mut mr = MappingReader::new(br);
        let mut in_section = false;

        while mr.next() {
            if mr.is_section() {
                if in_section {
                    break; // Next section found, stop
                }
                if mr.get_section().is_some_and(|s| *s == section_filter) {
                    in_section = true;
                }
            } else if in_section {
                // buildKeepingLast: a later duplicate key replaces the value in place.
                map_builder.insert(
                    mr.get_key().unwrap().clone(),
                    mr.get_value().unwrap().clone(),
                );
            }
        }
        Ok(map_builder)
    }

    // port: IdMappingUtil#parseSerializedIdMappings(BufferedReader)
    fn parse_serialized_id_mappings_reader(br: LineReader) -> IndexMap<JsString, BiMap> {
        let mut result_map: IndexMap<JsString, BiMap> = IndexMap::<_, _>::default();
        let mut mr = MappingReader::new(br);
        let mut current_section: Option<JsString> = None;
        let mut section_names: IndexSet<JsString> = IndexSet::<_>::default();

        while mr.next() {
            if mr.is_section() {
                let section_name = mr.get_section().unwrap().clone();
                if !section_names.insert(section_name.clone()) {
                    panic!(
                        "IllegalArgumentException: {}",
                        format_args!(
                            "Cannot parse id map: Duplicate section {}\n lineIndex: {}",
                            section_name,
                            mr.get_line_index()
                        )
                    );
                }
                result_map.insert(section_name.clone(), BiMap::new());
                current_section = Some(section_name);
            } else {
                let Some(section) = &current_section else {
                    panic!("IllegalArgumentException: Mapping entry outside of section");
                };
                // throws if duplicate values
                result_map[section].put(
                    mr.get_key().unwrap().clone(),
                    mr.get_value().unwrap().clone(),
                );
            }
        }
        result_map
    }

    // port: IdMappingUtil#parseSerializedIdMappings(String)
    pub fn parse_serialized_id_mappings(id_mappings: Option<&str>) -> IndexMap<JsString, BiMap> {
        match id_mappings {
            None | Some("") => IndexMap::<_, _>::default(),
            Some(id_mappings) => {
                let br = LineReader::new(&JsString::from(id_mappings));
                Self::parse_serialized_id_mappings_reader(br)
            }
        }
    }
}
