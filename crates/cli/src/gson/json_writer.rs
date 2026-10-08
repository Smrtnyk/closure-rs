/*
 * Copyright (C) 2010 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * Copyright (C) 2008 Google Inc.
 * Copyright (C) 2010 Google Inc.
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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/Gson.java,
//   com/google/gson/internal/Streams.java, com/google/gson/stream/JsonWriter.java.

use crate::abstract_command_line_runner::JsonFileSpec;
use closure_rhino::js_string::JsString;
// port: JsonWriter#string
pub fn string(value: &JsString) -> JsString {
    let mut out = vec![34];
    for &c in value.as_units() {
        let escape = match c {
            34 => Some("\\\""),
            92 => Some("\\\\"),
            9 => Some("\\t"),
            8 => Some("\\b"),
            10 => Some("\\n"),
            13 => Some("\\r"),
            12 => Some("\\f"),
            0x2028 => Some("\\u2028"),
            0x2029 => Some("\\u2029"),
            _ => None,
        };
        if let Some(escape) = escape {
            out.extend(escape.encode_utf16());
        } else if c < 32 {
            out.extend(format!("\\u{c:04x}").encode_utf16());
        } else {
            out.push(c);
        }
    }
    out.push(34);
    JsString::from_units(out)
}
// port: Gson#toJson(Object, Type, JsonWriter)
// The supplied JsonWriter retains its empty indent despite GsonBuilder.setPrettyPrinting().
pub fn to_json(files: &[Option<JsonFileSpec>]) -> JsString {
    if files.is_empty() {
        return JsString::from("[]");
    }
    let mut out: Vec<u16> = "[".encode_utf16().collect();
    for (i, file) in files.iter().enumerate() {
        if i > 0 {
            out.extend(",".encode_utf16());
        }
        let Some(file) = file else {
            out.extend("null".encode_utf16());
            continue;
        };
        let fields: Vec<_> = [
            ("src", &file.src),
            ("path", &file.path),
            ("source_map", &file.source_map),
            ("webpack_id", &file.webpack_id),
        ]
        .into_iter()
        .filter_map(|(n, v)| v.as_ref().map(|v| (n, v)))
        .collect();
        if fields.is_empty() {
            out.extend("{}".encode_utf16());
            continue;
        }
        out.extend("{".encode_utf16());
        for (i, (name, value)) in fields.iter().enumerate() {
            if i > 0 {
                out.extend(",".encode_utf16());
            }
            out.extend(string(&JsString::from(*name)).as_units());
            out.extend(":".encode_utf16());
            out.extend(string(value).as_units());
        }
        out.extend("}".encode_utf16());
    }
    out.extend("]".encode_utf16());
    JsString::from_units(out)
}

// port: Streams#write (the JsonElement subset used by JSChunkGraph.toJson)
pub fn element_to_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => string(&value.as_str().into()).to_string_lossy(),
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(element_to_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        serde_json::Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!(
                    "{}:{}",
                    string(&key.as_str().into()).to_string_lossy(),
                    element_to_json(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
