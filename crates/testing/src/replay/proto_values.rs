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
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Protobuf option values in the recorder's neutral encoding (FORMAT.md "Protobuf message or
//! builder": `{"proto": <full name>, "fields": {<field name>: value}}` with the fields of
//! `getAllFields()` in field-number order). Decoding and encoding go through the generated
//! messages' reflection surface (`closure_jscomp::protobuf::text_format::Message`).

use crate::json::JsString;
use crate::replay::options_values::OptionValue;
use crate::replay::replay_dsl::DslValue;
use crate::throwable::Throwable;
use crate::value::{ProtoMessage, ProtoValue};
use closure_jscomp::conformance_config::ConformanceConfig;
use closure_jscomp::protobuf::text_format::{
    FieldKind, FieldValue, Message, ParseException, get_all_fields,
};
use closure_rhino::fx_hash::IndexMap;

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("undecodable protobuf value: {what}"))
}

// port: UnitRecorder#value (MessageOrBuilder)
pub fn encode_message(message: &dyn Message) -> ProtoMessage {
    let mut fields = IndexMap::<_, _>::default();
    for field in get_all_fields(message) {
        let values: Vec<ProtoValue> = message
            .get_field(field.get_number())
            .into_iter()
            .map(|v| match v {
                FieldValue::String(s) => ProtoValue::String(JsString::from(s)),
                FieldValue::Bool(b) => ProtoValue::Bool(b),
                FieldValue::Enum(number) => {
                    let FieldKind::Enum(_, values) = field.kind() else {
                        unreachable!()
                    };
                    let name = values
                        .iter()
                        .find(|(_, n)| *n == number)
                        .map(|(name, _)| *name)
                        .unwrap();
                    ProtoValue::String(JsString::from(name))
                }
                FieldValue::Message(m) => ProtoValue::Message(Box::new(encode_message(m))),
            })
            .collect();
        let value = if field.is_repeated() {
            ProtoValue::Repeated(values)
        } else {
            values.into_iter().next().unwrap()
        };
        fields.insert(field.get_name().to_string(), value);
    }
    ProtoMessage {
        name: message.full_name().to_string(),
        fields,
    }
}

// port: ReplayValues#decode (protobuf message)
pub fn decode_message(encoded: &ProtoMessage, target: &mut dyn Message) -> Result<(), Throwable> {
    if encoded.name != target.full_name() {
        return Err(bad(&format!(
            "expected {}, found {}",
            target.full_name(),
            encoded.name
        )));
    }
    for (name, value) in &encoded.fields {
        let field = target
            .find_field_by_name(name)
            .ok_or_else(|| bad(&format!("{}.{name}", encoded.name)))?;
        let values: Vec<&ProtoValue> = match (field.is_repeated(), value) {
            (true, ProtoValue::Repeated(vs)) => vs.iter().collect(),
            (false, v) => vec![v],
            _ => return Err(bad(name)),
        };
        for v in values {
            match (field.kind(), v) {
                (FieldKind::String, ProtoValue::String(s)) => {
                    target.set_string(field.get_number(), s.to_string_lossy())
                }
                (FieldKind::Bool, ProtoValue::Bool(b)) => target.set_bool(field.get_number(), *b),
                (FieldKind::Enum(_, enum_values), ProtoValue::String(s)) => {
                    let s = s.to_string_lossy();
                    let number = enum_values
                        .iter()
                        .find(|(n, _)| *n == s)
                        .map(|(_, number)| *number)
                        .ok_or_else(|| bad(&s))?;
                    target.set_enum(field.get_number(), number);
                }
                (FieldKind::Message, ProtoValue::Message(m)) => {
                    let mut error = None;
                    let _ = target.merge_message_field(field.get_number(), &mut |sub| {
                        decode_message(m, sub).map_err(|e| {
                            error = Some(e);
                            ParseException::new(-1, -1, "")
                        })
                    });
                    if let Some(e) = error {
                        return Err(e);
                    }
                }
                _ => return Err(bad(name)),
            }
        }
    }
    Ok(())
}

impl OptionValue for ConformanceConfig {
    // port: ReplayValues#decode (ConformanceConfig)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        match value.untyped() {
            DslValue::Proto(p) => {
                let mut builder = ConformanceConfig::new_builder();
                decode_message(p, &mut builder)?;
                Ok(builder.build())
            }
            _ => Err(bad("ConformanceConfig")),
        }
    }
    // port: UnitRecorder#value (ConformanceConfig)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Proto(encode_message(self)))
    }
}
