/*
 * Copyright 2005 The Closure Compiler Authors.
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
/*
 * Copyright (C) 2008 The Guava Authors
 * Copyright (C) 2018 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/VariableMap.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/collect/Hashing.java, com/google/common/collect/ImmutableMap.java,
//   com/google/common/collect/JdkBackedImmutableBiMap.java,
//   com/google/common/collect/JdkBackedImmutableMap.java.

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    java_lang::{parse_exception::ParseException, utf_8},
    js_string::JsString,
};
use std::{
    fmt,
    io::{self, Read},
};

const SEPARATOR: u16 = b':' as u16;

#[derive(Debug, Clone)]
pub struct VariableMap {
    map: IndexMap<JsString, JsString>,
    inverse: IndexMap<JsString, JsString>,
}

#[derive(Debug)]
pub enum FromStreamError {
    Parse(ParseException),
    Io(io::Error),
}
impl fmt::Display for FromStreamError {
    // port: java.lang.Throwable#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for FromStreamError {
    // port: java.lang.Throwable#getCause
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Parse(e) => e,
            Self::Io(e) => e,
        })
    }
}
impl From<ParseException> for FromStreamError {
    // port: VariableMap#fromStream (throws ParseException)
    fn from(e: ParseException) -> Self {
        Self::Parse(e)
    }
}
impl From<io::Error> for FromStreamError {
    // port: VariableMap#fromStream (throws IOException)
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl VariableMap {
    // port: VariableMap#VariableMap
    pub fn new(map: &IndexMap<JsString, JsString>) -> Self {
        let mask = closed_table_size(map.len() as i32, 1.2) - 1;
        let mut key_table: IndexMap<i32, Vec<usize>> = IndexMap::<_, _>::default();
        let mut value_table: IndexMap<i32, Vec<usize>> = IndexMap::<_, _>::default();
        let mut inverse = IndexMap::with_capacity_and_hasher(map.len(), Default::default());
        for (i, (k, v)) in map.iter().enumerate() {
            let key_bucket = key_table.entry(smear(k.hash_code()) & mask).or_default();
            // RegularImmutableBiMap checks key-bucket overflow before checking values.
            if key_bucket.len() > 8 {
                return Self {
                    map: map.clone(),
                    inverse: Self::jdk_backed_inverse(map),
                };
            }
            let value_bucket = value_table.entry(smear(v.hash_code()) & mask).or_default();
            let mut bucket_size: i32 = 0;
            for &previous_index in value_bucket.iter().rev() {
                let (previous, previous_value) = map.get_index(previous_index).unwrap();
                if v == previous_value {
                    panic!("Multiple entries with same value: {k}={v} and {previous}={v}");
                }
                bucket_size += 1;
                if bucket_size > 8 {
                    return Self {
                        map: map.clone(),
                        inverse: Self::jdk_backed_inverse(map),
                    };
                }
            }
            key_bucket.push(i);
            value_bucket.push(i);
            inverse.insert(v.clone(), k.clone());
        }
        Self {
            map: map.clone(),
            inverse,
        }
    }
    // port: com.google.common.collect.JdkBackedImmutableBiMap#create
    fn jdk_backed_inverse(map: &IndexMap<JsString, JsString>) -> IndexMap<JsString, JsString> {
        let mut backward_delegate =
            IndexMap::with_capacity_and_hasher(map.len(), Default::default());
        for (k, v) in map {
            if let Some(previous) = backward_delegate.get(v) {
                panic!("Multiple entries with same value: {previous}={v} and {k}={v}");
            }
            backward_delegate.insert(v.clone(), k.clone());
        }
        backward_delegate
    }
    // port: VariableMap#lookupNewName
    pub fn lookup_new_name(&self, source_name: &JsString) -> Option<JsString> {
        self.map.get(source_name).cloned()
    }
    // port: VariableMap#lookupSourceName
    pub fn lookup_source_name(&self, new_name: &JsString) -> Option<JsString> {
        self.inverse.get(new_name).cloned()
    }
    // port: VariableMap#getOriginalNameToNewNameMap
    pub fn get_original_name_to_new_name_map(&self) -> IndexMap<JsString, JsString> {
        let mut result = self.map.clone();
        result.sort_keys();
        result
    }
    // port: VariableMap#getNewNameToOriginalNameMap
    pub fn get_new_name_to_original_name_map(&self) -> IndexMap<JsString, JsString> {
        self.inverse.clone()
    }
    // port: VariableMap#save
    pub fn save(&self, filename: &str) -> io::Result<()> {
        std::fs::write(filename, self.to_bytes())
    }
    // port: VariableMap#load
    pub fn load(filename: &str) -> io::Result<Self> {
        Self::from_bytes(&std::fs::read(filename)?).map_err(io::Error::other)
    }
    // port: VariableMap#toBytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Vec::new();
        for (k, v) in self.get_original_name_to_new_name_map() {
            writer.extend_from_slice(Self::escape(&k).as_units());
            writer.push(SEPARATOR);
            writer.extend_from_slice(Self::escape(&v).as_units());
            writer.push(10);
        }
        // StreamEncoder keeps a pending high surrogate between write calls. Encoding the
        // complete writer contents preserves that behavior, with '?' replacement on close.
        utf_8::encode(&JsString::from_units(writer))
    }
    // port: VariableMap#fromBytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ParseException> {
        Self::from_lines(&utf_8::decode(bytes))
    }
    // port: VariableMap#fromStream
    pub fn from_stream(stream: impl Read) -> Result<Self, FromStreamError> {
        let mut map = Vec::new();
        let mut br = utf_8::BufferedReader::new(stream);
        while let Some(line) = br.read_line()? {
            if line.is_empty() {
                continue;
            }
            let pos = Self::find_index_of_unescaped_char(&line, SEPARATOR);
            if pos <= 0 {
                return Err(ParseException::new(format!("Bad line: {line}"), 0).into());
            }
            map.push((
                Self::unescape(&line.substring(0, pos as usize)),
                if pos == line.length() as i32 - 1 {
                    JsString::from("")
                } else {
                    Self::unescape(&line.substring_from((pos + 1) as usize))
                },
            ));
        }
        Ok(Self::new(&Self::build_or_throw(map)))
    }
    // port: VariableMap#fromLines
    pub fn from_lines(string: &JsString) -> Result<Self, ParseException> {
        let mut map = Vec::new();
        let mut start_of_line: i32 = 0;
        while start_of_line < string.length() as i32 {
            let mut new_line = string.index_of_from("\n", start_of_line);
            if new_line == -1 {
                new_line = string.length() as i32;
            }
            let end_of_line = new_line;
            if Self::char_at(string, new_line - 1) == 13 {
                new_line -= 1;
            }
            let line = string.substring(start_of_line as usize, new_line as usize);
            start_of_line = end_of_line.wrapping_add(1);
            if line.is_empty() {
                continue;
            }
            let pos = Self::find_index_of_unescaped_char(&line, SEPARATOR);
            if pos <= 0 {
                return Err(ParseException::new(format!("Bad line: {line}"), 0));
            }
            map.push((
                Self::unescape(&line.substring(0, pos as usize)),
                if pos == line.length() as i32 - 1 {
                    JsString::from("")
                } else {
                    Self::unescape(&line.substring_from((pos + 1) as usize))
                },
            ));
        }
        Ok(Self::new(&Self::build_or_throw(map)))
    }
    // port: com.google.common.collect.ImmutableMap.Builder#buildOrThrow
    fn build_or_throw(entries: Vec<(JsString, JsString)>) -> IndexMap<JsString, JsString> {
        // RegularImmutableMap builds buckets from the last entry toward the first.
        let mask = closed_table_size(entries.len() as i32, 1.2) - 1;
        let mut table: IndexMap<i32, Vec<usize>> = IndexMap::<_, _>::default();
        for (i, (k, v)) in entries.iter().enumerate().rev() {
            let bucket = table.entry(smear(k.hash_code()) & mask).or_default();
            let mut bucket_size: i32 = 0;
            for &previous_index in bucket.iter().rev() {
                let (previous_key, previous_value) = &entries[previous_index];
                if k == previous_key {
                    panic!(
                        "Multiple entries with same key: {previous_key}={previous_value} and {k}={v}"
                    );
                }
                bucket_size += 1;
                if bucket_size > 8 {
                    return Self::jdk_backed_map(entries);
                }
            }
            bucket.push(i);
        }
        entries.into_iter().collect()
    }
    // port: com.google.common.collect.JdkBackedImmutableMap#create
    fn jdk_backed_map(entries: Vec<(JsString, JsString)>) -> IndexMap<JsString, JsString> {
        let mut map = IndexMap::<_, _>::default();
        for (k, v) in entries {
            if let Some(previous) = map.get(&k) {
                panic!("Multiple entries with same key: {k}={v} and {k}={previous}");
            }
            map.insert(k, v);
        }
        map
    }
    // port: java.lang.String#charAt
    fn char_at(value: &JsString, i: i32) -> u16 {
        if i < 0 || i as usize >= value.length() {
            panic!("Index {i} out of bounds for length {}", value.length());
        }
        value.char_at(i as usize)
    }
    // port: VariableMap#escape
    pub fn escape(value: &JsString) -> JsString {
        replace(
            &replace(&replace(value, "\\", "\\\\"), ":", "\\:"),
            "\n",
            "\\n",
        )
    }
    // port: VariableMap#findIndexOfUnescapedChar
    pub fn find_index_of_unescaped_char(value: &JsString, stop_char: u16) -> i32 {
        let len = value.length() as i32;
        let mut i: i32 = 0;
        while i < len {
            let stop_char_index = value.index_of_from(JsString::from_units(vec![stop_char]), i);
            if stop_char_index == -1 {
                return -1;
            }
            if Self::char_at(value, stop_char_index - 1) != 92 {
                return stop_char_index;
            }
            i = stop_char_index.wrapping_add(1);
        }
        -1
    }
    // port: VariableMap#unescape
    pub fn unescape(value: &JsString) -> JsString {
        let slash_index = value.index_of_char(92);
        if slash_index == -1 {
            return value.clone();
        }
        let mut sb = value.as_units()[..slash_index as usize].to_vec();
        let len = value.length() as i32;
        let mut i = slash_index;
        while i < len {
            let mut c = value.char_at(i as usize);
            if c == 92 {
                i = i.wrapping_add(1);
                if i < len {
                    c = value.char_at(i as usize);
                }
            }
            sb.push(c);
            i = i.wrapping_add(1);
        }
        JsString::from_units(sb)
    }
    // port: VariableMap#fromMap
    pub fn from_map(map: &IndexMap<JsString, JsString>) -> Self {
        Self::new(map)
    }
    // port: VariableMap#toMap
    pub fn to_map(&self) -> IndexMap<JsString, JsString> {
        self.map.clone()
    }
}

// port: com.google.common.collect.Hashing#smear
fn smear(hash_code: i32) -> i32 {
    hash_code
        .wrapping_mul(-862048943)
        .rotate_left(15)
        .wrapping_mul(461845907)
}

// port: com.google.common.collect.Hashing#closedTableSize
fn closed_table_size(expected_entries: i32, load_factor: f64) -> i32 {
    let expected_entries = expected_entries.max(2);
    let mut table_size = 1i32 << (31 - expected_entries.leading_zeros());
    if expected_entries > (load_factor * table_size as f64) as i32 {
        table_size = table_size.wrapping_shl(1);
        return if table_size > 0 { table_size } else { 1 << 30 };
    }
    table_size
}

// port: java.lang.String#replace(CharSequence,CharSequence)
fn replace(value: &JsString, target: &str, replacement: &str) -> JsString {
    let target = JsString::from(target);
    let replacement = JsString::from(replacement);
    let mut j = value.index_of(&target);
    if j < 0 {
        return value.clone();
    }
    let target_len = target.length();
    let mut i: usize = 0;
    let mut sb: Vec<u16> = Vec::new();
    loop {
        sb.extend_from_slice(&value.as_units()[i..j as usize]);
        sb.extend_from_slice(replacement.as_units());
        i = j as usize + target_len;
        j = value.index_of_from(&target, i as i32);
        if j < 0 {
            break;
        }
    }
    sb.extend_from_slice(&value.as_units()[i..]);
    JsString::from_units(sb)
}
