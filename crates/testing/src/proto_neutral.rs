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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch).

//! The protobuf neutral encoding of `UnitRecorder` (corpus/unit/FORMAT.md "Neutral encodings"):
//! `{"proto": <descriptor full name>, "fields": {<field name>: value}}` with exactly the fields
//! `MessageOrBuilder#getAllFields` returns, built from a Rust message's wire bytes and the
//! descriptors the generated `*_proto.rs` files embed.
//!
//! Rust-only reading of `getAllFields` from the wire: protobuf-java serializes exactly the fields
//! `getAllFields` reports (proto3 implicit-presence scalars only when non-default; explicit
//! presence, message and oneof fields when set; repeated fields when non-empty), so decoding the
//! serialized bytes against the descriptor yields the same field set and values. Fields are
//! listed in descriptor declaration order (`Descriptor#getFields`, the iteration order of
//! `GeneratedMessage#getAllFieldsMutable`).
use crate::{
    json::{JsString, JsonNumber},
    value::{ProtoMessage, ProtoValue},
};
use closure_jscomp::{
    compiler_state_proto,
    serialization::{
        optimization_jsdoc_proto,
        protobuf::{
            CodedInputStream, Descriptor, EnumDescriptor, FieldDescriptor, FieldType, Message,
            WireFormat,
        },
        source_file_proto, typed_ast_proto, types_proto,
    },
};
use indexmap::IndexMap;

/// The descriptor tables of every generated file (all in package `jscomp`).
fn message_descriptor_tables() -> [&'static [Descriptor]; 5] {
    [
        typed_ast_proto::MESSAGE_DESCRIPTORS,
        types_proto::MESSAGE_DESCRIPTORS,
        optimization_jsdoc_proto::MESSAGE_DESCRIPTORS,
        source_file_proto::MESSAGE_DESCRIPTORS,
        compiler_state_proto::MESSAGE_DESCRIPTORS,
    ]
}

fn enum_descriptor_tables() -> [&'static [EnumDescriptor]; 5] {
    [
        typed_ast_proto::ENUM_DESCRIPTORS,
        types_proto::ENUM_DESCRIPTORS,
        optimization_jsdoc_proto::ENUM_DESCRIPTORS,
        source_file_proto::ENUM_DESCRIPTORS,
        compiler_state_proto::ENUM_DESCRIPTORS,
    ]
}

fn find_message_type(full_name: &str) -> Result<&'static Descriptor, String> {
    message_descriptor_tables()
        .into_iter()
        .flatten()
        .find(|d| d.full_name == full_name)
        .ok_or_else(|| format!("no protobuf descriptor for message {full_name}"))
}

fn find_enum_type(full_name: &str) -> Result<&'static EnumDescriptor, String> {
    enum_descriptor_tables()
        .into_iter()
        .flatten()
        .find(|d| d.full_name == full_name)
        .ok_or_else(|| format!("no protobuf descriptor for enum {full_name}"))
}

// port: UnitRecorder#dump (MessageOrBuilder: {"proto", "fields"})
pub fn to_proto_message<M: Message + InMemoryStrings>(
    full_name: &str,
    message: &M,
) -> Result<ProtoMessage, String> {
    let mut neutral = from_bytes(full_name, &message.to_byte_array())?;
    message.overlay_in_memory_strings(&mut neutral);
    Ok(neutral)
}

/// Rust-only: the recorder dumps the in-memory Java message, so a string field keeps the UTF-16
/// value it holds, including a lone surrogate that protobuf's UTF-8 encoding (`Utf8#encode`)
/// writes as '?'. The wire bytes lose that, so string fields whose in-memory value is a
/// [`closure_rhino::js_string::JsString`] are taken from the message itself. The only such
/// generated field is `SourceFileProto.preloaded_contents`; every other string field is a Rust
/// `String`, whose wire form is exact.
pub trait InMemoryStrings {
    // port: UnitRecorder#dump (string field values of the in-memory message)
    fn overlay_in_memory_strings(&self, _neutral: &mut ProtoMessage) {}
}

impl InMemoryStrings for types_proto::TypePool {}

impl InMemoryStrings for typed_ast_proto::TypedAst {
    // port: UnitRecorder#dump (string field values of the in-memory message)
    fn overlay_in_memory_strings(&self, neutral: &mut ProtoMessage) {
        if let (Some(pool), Some(ProtoValue::Message(dump))) = (
            &self.source_file_pool,
            neutral.fields.get_mut("source_file_pool"),
        ) {
            pool.overlay_in_memory_strings(dump);
        }
    }
}

impl InMemoryStrings for source_file_proto::SourceFilePool {
    // port: UnitRecorder#dump (string field values of the in-memory message)
    fn overlay_in_memory_strings(&self, neutral: &mut ProtoMessage) {
        if let Some(ProtoValue::Repeated(dumps)) = neutral.fields.get_mut("source_file") {
            for (file, dump) in self.source_file.iter().zip(dumps.iter_mut()) {
                if let ProtoValue::Message(dump) = dump {
                    file.overlay_in_memory_strings(dump);
                }
            }
        }
    }
}

impl InMemoryStrings for source_file_proto::SourceFileProto {
    // port: UnitRecorder#dump (string field values of the in-memory message)
    fn overlay_in_memory_strings(&self, neutral: &mut ProtoMessage) {
        if let source_file_proto::Loader::PRELOADED_CONTENTS(contents) = &self.loader
            && let Some(value) = neutral.fields.get_mut("preloaded_contents")
        {
            *value = ProtoValue::String(JsString(contents.as_units().to_vec()));
        }
    }
}

// port: UnitRecorder#dump (MessageOrBuilder: {"proto", "fields"}, from the message's bytes)
pub fn from_bytes(full_name: &str, bytes: &[u8]) -> Result<ProtoMessage, String> {
    let descriptor = find_message_type(full_name)?;
    // Values per field number in wire order; singular message fields keep their concatenated
    // bytes (a repeated occurrence merges, as parsing the concatenation does).
    let mut values: IndexMap<u32, Vec<ProtoValue>> = IndexMap::new();
    let mut message_bytes: IndexMap<u32, Vec<u8>> = IndexMap::new();
    let mut input = CodedInputStream::new_instance(bytes);
    loop {
        let tag = input.read_tag().map_err(|e| e.get_message().to_string())?;
        if tag == 0 {
            break;
        }
        let number = WireFormat::get_tag_field_number(tag);
        let wire_type = WireFormat::get_tag_wire_type(tag);
        let Some(field) = descriptor.fields.iter().find(|f| f.number == number) else {
            // Unknown fields are not part of getAllFields.
            input
                .skip_field(tag)
                .map_err(|e| e.get_message().to_string())?;
            continue;
        };
        // Setting a oneof member clears the oneof's other members.
        if let Some(oneof) = field.containing_oneof {
            for other in descriptor.fields {
                if other.number != number && other.containing_oneof == Some(oneof) {
                    values.shift_remove(&other.number);
                    message_bytes.shift_remove(&other.number);
                }
            }
        }
        if field.field_type == FieldType::MESSAGE && !field.repeated {
            let bytes = input
                .read_bytes()
                .map_err(|e| e.get_message().to_string())?;
            message_bytes.entry(number).or_default().extend(bytes);
            values.insert(number, vec![]);
            continue;
        }
        let packed = wire_type == WireFormat::WIRETYPE_LENGTH_DELIMITED
            && !matches!(
                field.field_type,
                FieldType::STRING | FieldType::BYTES | FieldType::MESSAGE
            );
        let mut read = vec![];
        if packed {
            let data = input
                .read_bytes()
                .map_err(|e| e.get_message().to_string())?;
            let mut packed_input = CodedInputStream::new_instance(&data);
            while !packed_input.is_at_end() {
                read.push(read_value(full_name, field, &mut packed_input)?);
            }
        } else {
            read.push(read_value(full_name, field, &mut input)?);
        }
        let slot = values.entry(number).or_default();
        if !field.repeated {
            slot.clear();
        }
        slot.extend(read);
    }
    let mut fields = IndexMap::new();
    for field in descriptor.fields {
        let Some(mut read) = values.shift_remove(&field.number) else {
            continue;
        };
        let value = if field.field_type == FieldType::MESSAGE && !field.repeated {
            let bytes = message_bytes
                .shift_remove(&field.number)
                .unwrap_or_default();
            ProtoValue::Message(Box::new(from_bytes(field.type_name, &bytes)?))
        } else if field.repeated {
            ProtoValue::Repeated(read)
        } else {
            read.pop().ok_or("empty protobuf field")?
        };
        fields.insert(field.name.to_string(), value);
    }
    Ok(ProtoMessage {
        name: full_name.to_string(),
        fields,
    })
}

// port: UnitRecorder#dump (protobuf field values)
fn read_value(
    containing: &str,
    field: &FieldDescriptor,
    input: &mut CodedInputStream<'_>,
) -> Result<ProtoValue, String> {
    let e = |e: closure_jscomp::serialization::protobuf::InvalidProtocolBufferException| {
        e.get_message().to_string()
    };
    Ok(match field.field_type {
        FieldType::INT32 | FieldType::UINT32 => ProtoValue::Number(JsonNumber::from_i64(
            i64::from(input.read_int32().map_err(e)?),
        )),
        FieldType::SINT32 => ProtoValue::Number(JsonNumber::from_i64(i64::from(
            input.read_sint32().map_err(e)?,
        ))),
        FieldType::FIXED32 | FieldType::SFIXED32 => {
            let b = input.read_raw_bytes(4).map_err(e)?;
            let v = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            ProtoValue::Number(JsonNumber::from_i64(i64::from(v)))
        }
        FieldType::INT64 | FieldType::UINT64 => {
            ProtoValue::Long(input.read_int64().map_err(e)?.to_string())
        }
        FieldType::SINT64 => {
            let n = input.read_raw_varint64().map_err(e)?;
            ProtoValue::Long((((n as u64) >> 1) as i64 ^ -(n & 1)).to_string())
        }
        FieldType::FIXED64 | FieldType::SFIXED64 => {
            ProtoValue::Long((input.read_raw_little_endian64().map_err(e)? as i64).to_string())
        }
        FieldType::DOUBLE => ProtoValue::Double(closure_rhino::java_lang::double_to_string(
            input.read_double().map_err(e)?,
        )),
        FieldType::FLOAT => {
            let b = input.read_raw_bytes(4).map_err(e)?;
            let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            ProtoValue::Double(closure_rhino::java_lang::double_to_string(f64::from(v)))
        }
        FieldType::BOOL => ProtoValue::Bool(input.read_bool().map_err(e)?),
        FieldType::STRING => ProtoValue::String(JsString::from_rust_str(
            input.read_string_require_utf8().map_err(e)?.as_str(),
        )),
        FieldType::ENUM => {
            let number = input.read_enum().map_err(e)?;
            let descriptor = find_enum_type(field.type_name)?;
            ProtoValue::String(JsString::from_rust_str(
                descriptor
                    .find_value_name_by_number_creating_if_unknown(number)
                    .as_str(),
            ))
        }
        FieldType::BYTES => {
            let bytes = input.read_bytes().map_err(e)?;
            bytes_value(containing, field, bytes)?
        }
        FieldType::MESSAGE => {
            let bytes = input.read_bytes().map_err(e)?;
            ProtoValue::Message(Box::new(from_bytes(field.type_name, &bytes)?))
        }
        FieldType::GROUP => return Err("protobuf groups are not supported".into()),
    })
}

/// `bytes` -> `{"bytes": base64}`, with the two FORMAT.md exceptions.
// port: UnitRecorder#dump (ByteString fields)
fn bytes_value(
    containing: &str,
    field: &FieldDescriptor,
    bytes: Vec<u8>,
) -> Result<ProtoValue, String> {
    if containing == "jscomp.StringPoolProto" && field.name == "strings" {
        // ByteString#toStringUtf8 (malformed input becomes U+FFFD)
        return Ok(ProtoValue::String(new_string_utf8(&bytes)));
    }
    if containing == "jscomp.LazyAst"
        && field.name == "script"
        && let Ok(ast) = from_bytes("jscomp.AstNode", &bytes)
    {
        return Ok(ProtoValue::AstNode(Box::new(ast)));
    }
    Ok(ProtoValue::Bytes(base64_encode(&bytes)))
}

// `new String(bytes, UTF_8)` and `Base64.Encoder#encodeToString`, ported from OpenJDK (GPL-2.0
// with the Classpath exception), are in their own file.
#[path = "proto_neutral_jdk.rs"]
mod jdk;
use jdk::{base64_encode, new_string_utf8};

#[cfg(test)]
mod tests {
    use super::to_proto_message;
    use crate::value::ProtoValue;
    use closure_jscomp::serialization::{
        source_file_proto::{SourceFilePool, SourceFileProto},
        typed_ast_proto::TypedAst,
    };

    #[test]
    fn string_fields_keep_the_in_memory_lone_surrogate() {
        // Java dumps the in-memory message: '\ud800' stays, where the wire bytes hold '?'.
        let contents = closure_rhino::js_string::JsString::from_units(vec![
            u16::from(b'\''),
            0xd800,
            u16::from(b'\''),
        ]);
        let ast = TypedAst {
            source_file_pool: Some(SourceFilePool {
                source_file: vec![
                    SourceFileProto::default()
                        .set_filename("a.js")
                        .set_preloaded_contents(contents),
                ],
            }),
            ..TypedAst::default()
        };
        let neutral = to_proto_message("jscomp.TypedAst", &ast).unwrap();
        let Some(ProtoValue::Message(pool)) = neutral.fields.get("source_file_pool") else {
            panic!("{neutral:?}");
        };
        let Some(ProtoValue::Repeated(files)) = pool.fields.get("source_file") else {
            panic!("{pool:?}");
        };
        let ProtoValue::Message(file) = &files[0] else {
            panic!("{files:?}");
        };
        assert_eq!(
            file.fields.get("preloaded_contents"),
            Some(&ProtoValue::String(crate::json::JsString(vec![
                u16::from(b'\''),
                0xd800,
                u16::from(b'\'')
            ])))
        );
        assert_eq!(
            file.fields.get("filename"),
            Some(&ProtoValue::String(crate::json::JsString::from_rust_str(
                "a.js"
            )))
        );
    }
}
