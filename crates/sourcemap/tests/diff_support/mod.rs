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

use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;
use closure_sourcemap::{
    base64::Base64,
    base64_vlq::Base64VLQ,
    file_position::FilePosition as P,
    gson::{Gson, JsonElement, Target},
    proto::mapping::OriginalMapping,
    source_map_consumer_factory::SourceMapConsumerFactory,
    source_map_consumer_v3::{SourceMapConsumerV3, StringCharIterator},
    source_map_generator_v3::{ExtensionValue, SourceMapGeneratorV3},
    source_map_parse_exception::SourceMapParseException as Error,
    source_map_section::SourceMapSection,
    util::Util,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};
// port: SourcemapDiffDriver#attempt
fn attempt(f: impl FnOnce() -> Result<Value, Error>) -> Value {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => json!({"error":text_value(&e.java_to_string())}),
        Err(p) => {
            if let Some(s) = p.downcast_ref::<closure_sourcemap::JavaException>() {
                return json!({"error":text_value(&s.0)});
            }
            let msg = if let Some(s) = p.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = p.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "java.lang.RuntimeException".into()
            };
            json!({"error":msg})
        }
    }
}
// port: SourcemapDiffDriver#mapping
fn mapping(m: Option<OriginalMapping>) -> Value {
    let Some(m) = m else {
        return Value::Null;
    };
    json!({"originalFile":m.has_original_file().then(||text_value(&m.get_original_file())),"lineNumber":m.has_line_number().then(||m.get_line_number()),"columnPosition":m.has_column_position().then(||m.get_column_position()),"identifier":m.has_identifier().then(||text_value(&m.get_identifier())),"precision":m.has_precision().then(||m.get_precision()as i32)})
}

// UTF-16 transport preserves unpaired surrogates without changing expectations.
fn text_value(text: &JsString) -> Value {
    let units = text.as_units();
    match String::from_utf16(units) {
        Ok(s) => json!(s),
        Err(_) => json!({"$utf16":units}),
    }
}
fn nullable_text(units: Option<&JsString>) -> Value {
    units.map_or(Value::Null, text_value)
}
fn text_array(units: Option<&[Option<JsString>]>) -> Value {
    units.map_or(Value::Null, |v| {
        Value::Array(v.iter().map(|v| nullable_text(v.as_ref())).collect())
    })
}
fn extension_text(v: &ExtensionValue) -> Value {
    match v {
        ExtensionValue::JsonElement(v) => text_value(&v.to_js_string()),
        _ => json!(v.to_string()),
    }
}
#[derive(Clone)]
struct Visit {
    source: Option<JsString>,
    symbol: Option<JsString>,
    original: [i32; 2],
    start: [i32; 2],
    end: [i32; 2],
}
impl Visit {
    fn value(&self) -> Value {
        json!([
            nullable_text(self.source.as_ref()),
            nullable_text(self.symbol.as_ref()),
            self.original,
            self.start,
            self.end
        ])
    }
}
fn visits(c: &SourceMapConsumerV3) -> Vec<Visit> {
    let mut v = Vec::new();
    c.visit_mappings(&mut |s: Option<&JsString>, n: Option<&JsString>, o, b, e| {
        v.push(Visit {
            source: s.cloned(),
            symbol: n.cloned(),
            original: pos(o),
            start: pos(b),
            end: pos(e),
        })
    });
    v
}
fn metadata(c: &SourceMapConsumerV3) -> Value {
    let raw = c.get_extensions();
    let extensions = if raw.keys().all(|k| String::from_utf16(k.as_units()).is_ok()) {
        Value::Object(
            raw.iter()
                .map(|(k, v)| (String::from_utf16(k.as_units()).unwrap(), extension_text(v)))
                .collect(),
        )
    } else {
        json!({"$entries":raw.iter().map(|(k,v)|json!([text_value(k),extension_text(v)])).collect::<Vec<_>>()})
    };
    json!({"file":nullable_text(c.get_file().as_ref()),"lineCount":c.get_line_count(),"sourceRoot":nullable_text(c.get_source_root().as_ref()),"sources":text_array(Some(c.get_original_sources())),"sourcesContent":text_array(c.get_original_sources_content()),"names":attempt(||{c.get_original_names();Ok(text_array(Some(c.get_original_names())))}),"extensions":extensions})
}
// port: SourcemapDiffDriver#pos
fn pos(p: P) -> [i32; 2] {
    [p.get_line(), p.get_column()]
}

// port: SourcemapDiffDriver#summary
fn summary(input: &str, fast: bool) -> Result<Value, Error> {
    let mut c = if fast {
        SourceMapConsumerFactory::parse_fast(input)?
    } else {
        let mut c = SourceMapConsumerV3::new();
        c.parse(input)?;
        c
    };
    let vs = visits(&c);
    let mut positions = IndexSet::<_>::default();
    positions.insert([0, 0]);
    positions.insert([1, 1]);
    for v in &vs {
        let b = v.start;
        let e = v.end;
        for delta in -1..=2 {
            positions.insert([b[0] + 1, b[1] + 1 + delta]);
        }
        positions.insert([e[0] + 1, e[1] + 1]);
        positions.insert([b[0] + 2, 1]);
        if e[0] == b[0] {
            positions.insert([b[0] + 1, (b[1] + e[1]) / 2 + 1]);
        }
    }
    let past = vs.last().map_or(c.get_line_count() + 3, |v| v.end[0] + 4);
    positions.insert([past, 100]);
    let queries: Vec<_> = positions
        .into_iter()
        .map(|p| {
            json!([
                p,
                attempt(|| Ok(mapping(c.get_mapping_for_line(p[0], p[1]))))
            ])
        })
        .collect();
    let reverse: Vec<_> = vs
        .iter()
        .take(10)
        .map(|v| {
            let ms: Vec<_> = c
                .get_reverse_mapping(v.source.clone(), v.original[0], v.original[1])
                .iter()
                .map(|m| mapping(Some(m.clone())))
                .collect();
            json!([nullable_text(v.source.as_ref()), v.original, ms])
        })
        .collect();
    let mut result = metadata(&c);
    result["visits"] = json!(vs.iter().map(Visit::value).collect::<Vec<_>>());
    result["queries"] = json!(queries);
    result["reverse"] = json!(reverse);
    result["regenerated"] = attempt(|| regenerate(input, c.get_file().as_ref(), 0, 0, ""));
    result["offset"] = attempt(|| regenerate(input, c.get_file().as_ref(), 2, 7, "wrap😀\n  "));
    Ok(result)
}
// port: SourcemapDiffDriver#regenerate
fn regenerate(
    input: &str,
    file: Option<&JsString>,
    line: i32,
    column: i32,
    prefix: &str,
) -> Result<Value, Error> {
    let mut g = SourceMapGeneratorV3::new();
    g.merge_map_section(line, column, input)?;
    g.set_wrapper_prefix(prefix);
    Ok(text_value(
        &g.append_to(&mut String::new(), file.cloned()).unwrap(),
    ))
}
// port: SourcemapDiffDriver#str
fn string<'a>(o: &'a Value, key: &str) -> Option<&'a str> {
    o[key].as_str()
}
// port: SourcemapDiffDriver#integer
fn integer(o: &Value, key: &str) -> i32 {
    o[key].as_i64().unwrap() as i32
}
// port: SourcemapDiffDriver#position
fn position(v: &Value) -> P {
    P::new(v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32)
}
// port: SourcemapDiffDriver#script
fn script(ops: &Value) -> Result<Value, Error> {
    let mut g = SourceMapGeneratorV3::new();
    let mut outputs = Vec::new();
    for op in ops.as_array().unwrap() {
        match string(op, "op").unwrap() {
            "mapping" => g.add_mapping(
                string(op, "source").map(JsString::from),
                string(op, "symbol").map(JsString::from),
                position(&op["original"]),
                position(&op["start"]),
                position(&op["end"]),
            ),
            "offset" => g.set_starting_position(integer(op, "line"), integer(op, "column")),
            "prefix" => g.set_wrapper_prefix(string(op, "value").unwrap()),
            "content" => g.add_sources_content(
                string(op, "source").unwrap(),
                Some(string(op, "value").unwrap().into()),
            ),
            "root" => g.set_source_root(string(op, "value").unwrap()),
            "reset" => g.reset(),
            "validate" => g.validate(true),
            "remove" => g.remove_extension(string(op, "name").unwrap()),
            "extension" => {
                let v = match string(op, "type").unwrap() {
                    "string" => ExtensionValue::String(string(op, "value").unwrap().into()),
                    "int" => ExtensionValue::Integer(integer(op, "value")),
                    _ => ExtensionValue::JsonElement(
                        Gson::new()
                            .from_json(string(op, "value").unwrap(), Target::JsonElement)
                            .unwrap()
                            .unwrap_or(JsonElement::Null),
                    ),
                };
                g.add_extension(string(op, "name").unwrap(), v)?;
            }
            "append" => {
                let name = string(op, "name").map(JsString::from);
                let units = g.append_to(&mut String::new(), name).unwrap();
                outputs.push(text_value(&units));
            }
            "index" => {
                let sections: Vec<_> = op["sections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| {
                        if string(s, "type") == Some("URL") {
                            SourceMapSection::for_url(
                                string(s, "value").unwrap(),
                                integer(s, "line"),
                                integer(s, "column"),
                            )
                        } else {
                            SourceMapSection::for_map(
                                string(s, "value").unwrap(),
                                integer(s, "line"),
                                integer(s, "column"),
                            )
                        }
                    })
                    .collect();
                let mut out = String::new();
                g.append_index_map_to(&mut out, string(op, "name").unwrap(), &sections)
                    .unwrap();
                outputs.push(json!(out));
            }
            other => panic!("Unknown operation: {other}"),
        }
    }
    Ok(json!(outputs))
}
// port: SourcemapDiffDriver#edges
fn edges(record: &Value) -> Result<Value, Error> {
    let v = integer(record, "value");
    let mut encoded = String::new();
    Base64VLQ::encode(&mut encoded, v).unwrap();
    let decoded = Base64VLQ::decode(&mut StringCharIterator::new(encoded.as_str()));
    Ok(
        json!({"encoded":encoded,"decoded":decoded,"base64":Base64::base64_encode_int(v).to_string_lossy(),"escaped":if let Some(units)=record["units"].as_array(){Util::escape_string(JsString::from_units(units.iter().map(|v|v.as_u64().unwrap()as u16).collect::<Vec<_>>())).to_string_lossy()}else{Util::escape_string(string(record,"text").unwrap()).to_string_lossy()}}),
    )
}
// port: SourcemapDiffDriver#main (record dispatch)
pub fn replay(record: &Value) -> Value {
    match string(record, "kind").unwrap() {
        "map" | "parse" => {
            json!({"slow":attempt(||summary(string(record,"input").unwrap(),false)),"fast":attempt(||summary(string(record,"input").unwrap(),true))})
        }
        "generator" => attempt(|| script(&record["ops"])),
        "edge" => attempt(|| edges(record)),
        other => panic!("Unknown kind {other}"),
    }
}
// Differential comparison reports the first differing field, including exact strings.
pub fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (k, v) in e {
                if let Some(d) =
                    first_difference(v, a.get(k).unwrap_or(&Value::Null), &format!("{path}.{k}"))
                {
                    return Some(d);
                }
            }
            Some(format!("{path}: keys differ"))
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                return Some(format!(
                    "{path}: expected length {}, actual {}",
                    e.len(),
                    a.len()
                ));
            }
            for (i, (e, a)) in e.iter().zip(a).enumerate() {
                if let Some(d) = first_difference(e, a, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            None
        }
        _ => Some(format!("{path}: expected {expected}, actual {actual}")),
    }
}

struct GoldenState {
    input: String,
    consumers: [SourceMapConsumerV3; 2],
    visits: [Vec<Visit>; 2],
    matched: bool,
}
impl GoldenState {
    fn new(input: &str) -> Self {
        let mut slow = SourceMapConsumerV3::new();
        slow.parse(input).unwrap();
        let fast = SourceMapConsumerFactory::parse_fast(input).unwrap();
        let vs = [visits(&slow), visits(&fast)];
        Self {
            input: input.into(),
            consumers: [slow, fast],
            visits: vs,
            matched: true,
        }
    }
    fn replay(&mut self, r: &Value) -> Value {
        match string(r, "part").unwrap() {
            "header" => {
                json!({"slow":metadata(&self.consumers[0]),"fast":metadata(&self.consumers[1])})
            }
            "visits" => {
                let start = r["start"].as_u64().unwrap() as usize;
                let count = r["expected"].as_array().unwrap().len();
                let values: Vec<_> = self.visits[0][start..start + count]
                    .iter()
                    .map(Visit::value)
                    .collect();
                let fast: Vec<_> = self.visits[1][start..start + count]
                    .iter()
                    .map(Visit::value)
                    .collect();
                assert_eq!(values, fast);
                json!(values)
            }
            "queries" => json!(
                r["expected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|q| {
                        let p = &q[0];
                        let l = p[0].as_i64().unwrap() as i32;
                        let col = p[1].as_i64().unwrap() as i32;
                        let slow =
                            attempt(|| Ok(mapping(self.consumers[0].get_mapping_for_line(l, col))));
                        let fast =
                            attempt(|| Ok(mapping(self.consumers[1].get_mapping_for_line(l, col))));
                        assert_eq!(slow, fast);
                        json!([p, slow])
                    })
                    .collect::<Vec<_>>()
            ),
            "reverse" => {
                let start = r["start"].as_u64().unwrap() as usize;
                let count = r["expected"].as_array().unwrap().len();
                let source = string(r, "source");
                let l = r["original"][0].as_i64().unwrap() as i32;
                let col = r["original"][1].as_i64().unwrap() as i32;
                let slow: Vec<_> =
                    self.consumers[0].get_reverse_mapping(source.map(JsString::from), l, col)
                        [start..start + count]
                        .iter()
                        .map(|m| mapping(Some(m.clone())))
                        .collect();
                let fast: Vec<_> =
                    self.consumers[1].get_reverse_mapping(source.map(JsString::from), l, col)
                        [start..start + count]
                        .iter()
                        .map(|m| mapping(Some(m.clone())))
                        .collect();
                assert_eq!(slow, fast);
                json!(slow)
            }
            "end" => {
                assert_eq!(
                    r["visitsCount"].as_u64().unwrap() as usize,
                    self.visits[0].len()
                );
                assert_eq!(self.visits[0].len(), self.visits[1].len());
                let c = &self.consumers[0];
                json!({"regenerated":attempt(||regenerate(&self.input,c.get_file().as_ref(),0,0,"")),"offset":attempt(||regenerate(&self.input,c.get_file().as_ref(),2,7,"wrap😀\n  "))})
            }
            other => panic!("Unknown map chunk {other}"),
        }
    }
}
pub fn expand_transport(value: Value) -> Value {
    match value {
        Value::Object(mut o) => {
            if let Some(v) = o.remove("$rangeValues") {
                let start = v[0].as_i64().unwrap();
                let count = v[1].as_u64().unwrap();
                return Value::Array((0..count).map(|i| json!(start + i as i64)).collect());
            }
            if let Some(v) = o.remove("$repeatValues") {
                let n = v[0].as_u64().unwrap() as usize;
                return Value::Array(vec![expand_transport(v[1].clone()); n]);
            }
            if let Some(v) = o.remove("$repeatText") {
                return json!(
                    v[1].as_str()
                        .unwrap()
                        .repeat(v[0].as_u64().unwrap() as usize)
                );
            }
            if let Some(v) = o.remove("$concat") {
                return json!(
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|s| expand_transport(s.clone()).as_str().unwrap().to_owned())
                        .collect::<String>()
                );
            }
            Value::Object(
                o.into_iter()
                    .map(|(k, v)| (k, expand_transport(v)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.into_iter().map(expand_transport).collect()),
        v => v,
    }
}
pub fn replay_file(path: &Path, print: bool) -> (usize, usize) {
    let mut matched = 0;
    let mut total = 0;
    let mut mismatches = 0;
    let mut state: Option<GoldenState> = None;
    let file = BufReader::new(std::fs::File::open(path).unwrap());
    for (line_number, line) in file.lines().enumerate() {
        let record: Value = expand_transport(serde_json::from_str(&line.unwrap()).unwrap());
        let golden = string(&record, "kind") == Some("golden");
        if golden && string(&record, "part") == Some("header") {
            state = Some(GoldenState::new(string(&record, "input").unwrap()));
        }
        let actual = if golden {
            state.as_mut().unwrap().replay(&record)
        } else {
            replay(&record)
        };
        let equal = actual == record["expected"];
        if !equal {
            mismatches += 1;
            if golden {
                state.as_mut().unwrap().matched = false;
            }
            if print && mismatches <= 10 {
                eprintln!(
                    "{}:{} id={} {}",
                    path.display(),
                    line_number + 1,
                    record["id"],
                    first_difference(&record["expected"], &actual, "$").unwrap()
                );
            }
        }
        if golden {
            if string(&record, "part") == Some("end") {
                total += 1;
                matched += usize::from(state.take().unwrap().matched);
            }
        } else {
            total += 1;
            matched += usize::from(equal);
        }
    }
    assert!(state.is_none(), "Truncated golden map stream");
    if print {
        println!(
            "{}: {matched}/{total} matched ({:.2}%)",
            path.display(),
            matched as f64 / total as f64 * 100.
        );
    }
    (matched, total)
}
