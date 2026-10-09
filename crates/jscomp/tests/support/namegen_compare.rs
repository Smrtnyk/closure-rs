/*
 * Copyright 2026 The closure-rs Authors.
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

use closure_jscomp::{
    default_name_generator::DefaultNameGenerator,
    name_generator::{NameGenerator, ReservedNames},
    variable_map::{FromStreamError, VariableMap},
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{java_lang::parse_exception::ParseException, js_string::JsString};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, RwLock},
};

pub fn string(hex: &str) -> JsString {
    if hex == "~" {
        return "".into();
    }
    JsString::from_units(
        (0..hex.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&hex[i..i + 4], 16).unwrap())
            .collect::<Vec<_>>(),
    )
}
pub fn hex(s: &JsString) -> String {
    if s.is_empty() {
        return "~".into();
    }
    s.as_units().iter().map(|c| format!("{c:04x}")).collect()
}
fn bytes(h: &str) -> Vec<u8> {
    if h == "~" {
        return Vec::new();
    }
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
        .collect()
}
fn bytes_hex(b: &[u8]) -> String {
    if b.is_empty() {
        "~".into()
    } else {
        b.iter().map(|c| format!("{c:02x}")).collect()
    }
}
fn chars(h: &str) -> IndexSet<u16> {
    string(h).as_units().iter().copied().collect()
}
fn names(h: &str) -> ReservedNames {
    Arc::new(RwLock::new(if h == "-" {
        IndexSet::<_>::default()
    } else {
        h.split(',').map(string).collect()
    }))
}
fn map(h: &str) -> IndexMap<JsString, JsString> {
    if h == "-" {
        return IndexMap::<_, _>::default();
    }
    h.split(',')
        .map(|e| {
            let (k, v) = e.split_once('=').unwrap();
            (string(k), string(v))
        })
        .collect()
}
fn entries(m: &IndexMap<JsString, JsString>) -> String {
    if m.is_empty() {
        return "-".into();
    }
    m.iter()
        .map(|(k, v)| format!("{}={}", hex(k), hex(v)))
        .collect::<Vec<_>>()
        .join(",")
}
fn parse_error(e: ParseException) -> String {
    format!(
        "ERR|java.text.ParseException|{}|{}",
        hex(&JsString::from(e.get_message())),
        e.get_error_offset()
    )
}
fn panic_error(e: Box<dyn std::any::Any + Send>) -> String {
    let msg = e
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| e.downcast_ref::<&str>().copied())
        .expect("unexpected panic payload");
    if msg.starts_with("Index ") {
        "ERR|java.lang.StringIndexOutOfBoundsException|-|-".into()
    } else {
        format!(
            "ERR|java.lang.IllegalArgumentException|{}|-",
            hex(&JsString::from(msg))
        )
    }
}

pub fn variable(line: &str) -> String {
    variable_with_stream_chunk(line, usize::MAX)
}

struct ChunkedRead<'a> {
    bytes: &'a [u8],
    chunk_size: usize,
}
impl std::io::Read for ChunkedRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let len = out.len().min(self.chunk_size);
        std::io::Read::read(&mut self.bytes, &mut out[..len])
    }
}

pub fn variable_with_stream_chunk(line: &str, chunk_size: usize) -> String {
    let p: Vec<_> = line.split('\t').collect();
    match catch_unwind(AssertUnwindSafe(|| {
        let result = match p[1] {
            "M" => Ok(VariableMap::new(&map(p[2]))),
            "B" => VariableMap::from_bytes(&bytes(p[2])).map_err(parse_error),
            "S" => VariableMap::from_stream(ChunkedRead {
                bytes: &bytes(p[2]),
                chunk_size,
            })
            .map_err(|e| match e {
                FromStreamError::Parse(e) => parse_error(e),
                FromStreamError::Io(e) => panic!("unexpected io error: {e}"),
            }),
            "L" => VariableMap::from_lines(&string(p[2])).map_err(parse_error),
            _ => panic!("unknown case mode"),
        };
        match result {
            Ok(v) => format!(
                "OK|{}|{}|{}|{}",
                bytes_hex(&v.to_bytes()),
                entries(&v.to_map()),
                entries(&v.get_new_name_to_original_name_map()),
                entries(&v.get_original_name_to_new_name_map())
            ),
            Err(e) => e,
        }
    })) {
        Ok(s) => s,
        Err(e) => panic_error(e),
    }
}

pub fn generate(line: &str) -> Vec<u8> {
    match catch_unwind(AssertUnwindSafe(|| {
        let p: Vec<_> = line.split('\t').collect();
        let reserved = names(p[3]);
        let a = match p[1] {
            "0" => {
                let mut a = DefaultNameGenerator::new();
                a.reset(reserved.clone(), string(p[2]), &chars(p[4]));
                a
            }
            "3" => DefaultNameGenerator::with_reserved_characters(
                reserved.clone(),
                string(p[2]),
                &chars(p[4]),
            ),
            "4" => DefaultNameGenerator::with_first_and_non_first_characters(
                reserved.clone(),
                string(p[2]),
                &chars(p[4]),
                &chars(p[5]),
            ),
            _ => panic!("unknown constructor"),
        };
        let mut gs = IndexMap::<_, _>::from_iter([("A", a)]);
        let mut result = Vec::new();
        for op in p[6].split(';') {
            let q: Vec<_> = op.split(':').collect();
            match q[0] {
                "F" => gs.get_mut(q[1]).unwrap().favors(&string(q[2])),
                "R3" => {
                    gs.get_mut(q[1])
                        .unwrap()
                        .reset(reserved.clone(), string(q[2]), &chars(q[3]))
                }
                "R4" => gs
                    .get_mut(q[1])
                    .unwrap()
                    .reset_with_first_and_non_first_characters(
                        reserved.clone(),
                        string(q[2]),
                        &chars(q[3]),
                        &chars(q[4]),
                    ),
                "C" => {
                    let copy =
                        gs[q[1]].clone_generator(reserved.clone(), string(q[3]), &chars(q[4]));
                    gs.insert(q[2], copy);
                }
                "M+" => {
                    reserved.write().unwrap().insert(string(q[1]));
                }
                "M-" => {
                    reserved.write().unwrap().shift_remove(&string(q[1]));
                }
                "G" => {
                    let g = gs.get_mut(q[1]).unwrap();
                    for _ in 0..q[2].parse::<i32>().unwrap() {
                        result.extend_from_slice(hex(&g.generate_next_name()).as_bytes());
                        result.push(b'\n');
                    }
                }
                _ => panic!("unknown operation"),
            }
        }
        result
    })) {
        Ok(bytes) => bytes,
        Err(e) => format!("{}\n", panic_error(e)).into_bytes(),
    }
}
