// Protocol Buffers - Google's data interchange format
// Copyright 2008 Google Inc.  All rights reserved.
//
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file or at
// https://developers.google.com/open-source/licenses/bsd

// Ported from Protocol Buffers for Java 4.30.2 (https://github.com/protocolbuffers/protobuf):
//   java/core/src/main/java/com/google/protobuf/CodedInputStream.java,
//   java/core/src/main/java/com/google/protobuf/CodedOutputStream.java,
//   java/core/src/main/java/com/google/protobuf/Descriptors.java,
//   java/core/src/main/java/com/google/protobuf/Internal.java,
//   java/core/src/main/java/com/google/protobuf/InvalidProtocolBufferException.java,
//   java/core/src/main/java/com/google/protobuf/MessageLite.java,
//   java/core/src/main/java/com/google/protobuf/Utf8.java,
//   java/core/src/main/java/com/google/protobuf/WireFormat.java.

//! Rust-only: the subset of the protobuf-java runtime (`com.google.protobuf`) that the generated
//! TypedAST and compiler-state messages and the serialization classes use: `CodedInputStream`,
//! `CodedOutputStream`, `WireFormat`, `InvalidProtocolBufferException` and the `Message` API
//! (`parseFrom`, `parseDelimitedFrom`, `writeTo`, `writeDelimitedTo`, `toByteArray`,
//! `getSerializedSize`). No protoc on the host (D-003), so the messages are generated once into
//! `*_proto.rs` and use these primitives; the wire format is protobuf's, so Java-written files
//! (`runtime_libs.typedast`) read back unchanged and Rust-written files read in Java.

use std::fmt;

/// port: com.google.protobuf.WireFormat
pub struct WireFormat;

impl WireFormat {
    pub const WIRETYPE_VARINT: u32 = 0;
    pub const WIRETYPE_FIXED64: u32 = 1;
    pub const WIRETYPE_LENGTH_DELIMITED: u32 = 2;
    pub const WIRETYPE_START_GROUP: u32 = 3;
    pub const WIRETYPE_END_GROUP: u32 = 4;
    pub const WIRETYPE_FIXED32: u32 = 5;
    const TAG_TYPE_BITS: u32 = 3;
    const TAG_TYPE_MASK: u32 = (1 << Self::TAG_TYPE_BITS) - 1;

    // port: WireFormat#getTagWireType
    pub fn get_tag_wire_type(tag: u32) -> u32 {
        tag & Self::TAG_TYPE_MASK
    }
    // port: WireFormat#getTagFieldNumber
    pub fn get_tag_field_number(tag: u32) -> u32 {
        tag >> Self::TAG_TYPE_BITS
    }
    // port: WireFormat#makeTag
    pub fn make_tag(field_number: u32, wire_type: u32) -> u32 {
        (field_number << Self::TAG_TYPE_BITS) | wire_type
    }
}

/// port: com.google.protobuf.InvalidProtocolBufferException
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidProtocolBufferException {
    pub message: String,
}

impl InvalidProtocolBufferException {
    // port: InvalidProtocolBufferException#<init>(String)
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
    // port: InvalidProtocolBufferException#truncatedMessage
    pub fn truncated_message() -> Self {
        Self::new(
            "While parsing a protocol message, the input ended unexpectedly in the middle of a field.  This could mean either that the input has been truncated or that an embedded message misreported its own length.",
        )
    }
    // port: InvalidProtocolBufferException#negativeSize
    pub fn negative_size() -> Self {
        Self::new(
            "CodedInputStream encountered an embedded string or message which claimed to have negative size.",
        )
    }
    // port: InvalidProtocolBufferException#malformedVarint
    pub fn malformed_varint() -> Self {
        Self::new("CodedInputStream encountered a malformed varint.")
    }
    // port: InvalidProtocolBufferException#invalidTag
    pub fn invalid_tag() -> Self {
        Self::new("Protocol message contained an invalid tag (zero).")
    }
    // port: InvalidProtocolBufferException#invalidEndTag
    pub fn invalid_end_tag() -> Self {
        Self::new("Protocol message end-group tag did not match expected tag.")
    }
    // port: InvalidProtocolBufferException#invalidWireType
    pub fn invalid_wire_type() -> Self {
        Self::new("Protocol message tag had invalid wire type.")
    }
    // port: InvalidProtocolBufferException#recursionLimitExceeded
    pub fn recursion_limit_exceeded() -> Self {
        Self::new(
            "Protocol message had too many levels of nesting.  May be malicious.  Use CodedInputStream.setRecursionLimit() to increase the depth limit.",
        )
    }
    // port: Throwable#getMessage
    pub fn get_message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InvalidProtocolBufferException {
    // port: Throwable#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.protobuf.InvalidProtocolBufferException: {}",
            self.message
        )
    }
}

impl std::error::Error for InvalidProtocolBufferException {}

pub type ProtoResult<T> = Result<T, InvalidProtocolBufferException>;

/// port: com.google.protobuf.CodedInputStream (array-backed)
pub struct CodedInputStream<'a> {
    buffer: &'a [u8],
    pos: usize,
    /// Absolute position the current (innermost) message ends at.
    current_limit: usize,
    last_tag: u32,
    recursion_depth: i32,
    recursion_limit: i32,
}

impl<'a> CodedInputStream<'a> {
    pub const DEFAULT_RECURSION_LIMIT: i32 = 100;

    // port: CodedInputStream#newInstance(byte[])
    pub fn new_instance(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            pos: 0,
            current_limit: buffer.len(),
            last_tag: 0,
            recursion_depth: 0,
            recursion_limit: Self::DEFAULT_RECURSION_LIMIT,
        }
    }

    // port: CodedInputStream#setRecursionLimit
    pub fn set_recursion_limit(&mut self, limit: i32) -> i32 {
        let old = self.recursion_limit;
        self.recursion_limit = limit;
        old
    }

    // port: CodedInputStream#isAtEnd
    pub fn is_at_end(&self) -> bool {
        self.pos == self.current_limit
    }

    // port: CodedInputStream#resetSizeCounter
    pub fn reset_size_counter(&mut self) {}

    // port: CodedInputStream#getTotalBytesRead
    pub fn get_total_bytes_read(&self) -> usize {
        self.pos
    }

    // port: CodedInputStream#getLastTag
    pub fn get_last_tag(&self) -> u32 {
        self.last_tag
    }

    // port: CodedInputStream#readTag
    pub fn read_tag(&mut self) -> ProtoResult<u32> {
        if self.is_at_end() {
            self.last_tag = 0;
            return Ok(0);
        }
        self.last_tag = self.read_raw_varint32()? as u32;
        if WireFormat::get_tag_field_number(self.last_tag) == 0 {
            // If we actually read zero (or any tag number corresponding to field
            // number zero), that's not a valid tag.
            return Err(InvalidProtocolBufferException::invalid_tag());
        }
        Ok(self.last_tag)
    }

    // port: CodedInputStream#checkLastTagWas
    pub fn check_last_tag_was(&self, value: u32) -> ProtoResult<()> {
        if self.last_tag != value {
            return Err(InvalidProtocolBufferException::invalid_end_tag());
        }
        Ok(())
    }

    // port: CodedInputStream#skipField(int)
    pub fn skip_field(&mut self, tag: u32) -> ProtoResult<bool> {
        match WireFormat::get_tag_wire_type(tag) {
            WireFormat::WIRETYPE_VARINT => {
                self.read_raw_varint64()?;
                Ok(true)
            }
            WireFormat::WIRETYPE_FIXED64 => {
                self.read_raw_bytes(8)?;
                Ok(true)
            }
            WireFormat::WIRETYPE_LENGTH_DELIMITED => {
                let size = self.read_raw_varint32()?;
                if size < 0 {
                    return Err(InvalidProtocolBufferException::negative_size());
                }
                self.read_raw_bytes(size as usize)?;
                Ok(true)
            }
            WireFormat::WIRETYPE_START_GROUP => {
                self.skip_message()?;
                self.check_last_tag_was(WireFormat::make_tag(
                    WireFormat::get_tag_field_number(tag),
                    WireFormat::WIRETYPE_END_GROUP,
                ))?;
                Ok(true)
            }
            WireFormat::WIRETYPE_END_GROUP => Ok(false),
            WireFormat::WIRETYPE_FIXED32 => {
                self.read_raw_bytes(4)?;
                Ok(true)
            }
            _ => Err(InvalidProtocolBufferException::invalid_wire_type()),
        }
    }

    // port: CodedInputStream#skipMessage
    pub fn skip_message(&mut self) -> ProtoResult<()> {
        loop {
            let tag = self.read_tag()?;
            if tag == 0 || !self.skip_field(tag)? {
                return Ok(());
            }
        }
    }

    // port: CodedInputStream#readDouble
    pub fn read_double(&mut self) -> ProtoResult<f64> {
        Ok(f64::from_bits(self.read_raw_little_endian64()?))
    }
    // port: CodedInputStream#readInt32
    pub fn read_int32(&mut self) -> ProtoResult<i32> {
        self.read_raw_varint32()
    }
    // port: CodedInputStream#readUInt32
    pub fn read_uint32(&mut self) -> ProtoResult<i32> {
        self.read_raw_varint32()
    }
    // port: CodedInputStream#readSInt32
    pub fn read_sint32(&mut self) -> ProtoResult<i32> {
        Ok(Self::decode_zig_zag32(self.read_raw_varint32()?))
    }
    // port: CodedInputStream#readInt64
    pub fn read_int64(&mut self) -> ProtoResult<i64> {
        self.read_raw_varint64()
    }
    // port: CodedInputStream#readBool
    pub fn read_bool(&mut self) -> ProtoResult<bool> {
        Ok(self.read_raw_varint64()? != 0)
    }
    // port: CodedInputStream#readEnum
    pub fn read_enum(&mut self) -> ProtoResult<i32> {
        self.read_raw_varint32()
    }
    // port: CodedInputStream#readStringRequireUtf8
    pub fn read_string_require_utf8(&mut self) -> ProtoResult<String> {
        let bytes = self.read_bytes()?;
        match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err(InvalidProtocolBufferException::new(
                "Protocol message had invalid UTF-8.",
            )),
        }
    }
    // port: CodedInputStream#readBytes
    pub fn read_bytes(&mut self) -> ProtoResult<Vec<u8>> {
        let size = self.read_raw_varint32()?;
        if size < 0 {
            return Err(InvalidProtocolBufferException::negative_size());
        }
        Ok(self.read_raw_bytes(size as usize)?.to_vec())
    }

    // port: CodedInputStream#readMessage(MessageLite.Builder,ExtensionRegistryLite)
    pub fn read_message<M: Message>(&mut self, builder: &mut M) -> ProtoResult<()> {
        let length = self.read_raw_varint32()?;
        if self.recursion_depth >= self.recursion_limit {
            return Err(InvalidProtocolBufferException::recursion_limit_exceeded());
        }
        let old_limit = self.push_limit(length)?;
        self.recursion_depth += 1;
        builder.merge_from(self)?;
        self.check_last_tag_was(0)?;
        self.recursion_depth -= 1;
        if self.get_bytes_until_limit() != 0 {
            return Err(InvalidProtocolBufferException::truncated_message());
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    // port: CodedInputStream#pushLimit
    pub fn push_limit(&mut self, byte_limit: i32) -> ProtoResult<usize> {
        if byte_limit < 0 {
            return Err(InvalidProtocolBufferException::negative_size());
        }
        let new_limit = self.pos.checked_add(byte_limit as usize);
        let old_limit = self.current_limit;
        match new_limit {
            Some(new_limit) if new_limit <= old_limit => {
                self.current_limit = new_limit;
                Ok(old_limit)
            }
            _ => Err(InvalidProtocolBufferException::truncated_message()),
        }
    }

    // port: CodedInputStream#popLimit
    pub fn pop_limit(&mut self, old_limit: usize) {
        self.current_limit = old_limit;
    }

    // port: CodedInputStream#getBytesUntilLimit
    pub fn get_bytes_until_limit(&self) -> usize {
        self.current_limit - self.pos
    }

    // port: CodedInputStream#readRawVarint32
    pub fn read_raw_varint32(&mut self) -> ProtoResult<i32> {
        Ok(self.read_raw_varint64()? as i32)
    }

    // port: CodedInputStream#readRawVarint64
    pub fn read_raw_varint64(&mut self) -> ProtoResult<i64> {
        let mut result: i64 = 0;
        let mut shift = 0;
        while shift < 64 {
            let b = self.read_raw_byte()?;
            result |= ((b & 0x7F) as i64) << shift;
            if (b & 0x80) == 0 {
                return Ok(result);
            }
            shift += 7;
        }
        Err(InvalidProtocolBufferException::malformed_varint())
    }

    // port: CodedInputStream#readRawLittleEndian64
    pub fn read_raw_little_endian64(&mut self) -> ProtoResult<u64> {
        let bytes = self.read_raw_bytes(8)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(array))
    }

    // port: CodedInputStream#readRawByte
    pub fn read_raw_byte(&mut self) -> ProtoResult<u8> {
        if self.pos == self.current_limit {
            return Err(InvalidProtocolBufferException::truncated_message());
        }
        let b = self.buffer[self.pos];
        self.pos += 1;
        Ok(b)
    }

    // port: CodedInputStream#readRawBytes
    pub fn read_raw_bytes(&mut self, size: usize) -> ProtoResult<&'a [u8]> {
        if size > self.current_limit - self.pos {
            return Err(InvalidProtocolBufferException::truncated_message());
        }
        let bytes = &self.buffer[self.pos..self.pos + size];
        self.pos += size;
        Ok(bytes)
    }

    // port: CodedInputStream#decodeZigZag32
    pub fn decode_zig_zag32(n: i32) -> i32 {
        ((n as u32) >> 1) as i32 ^ -(n & 1)
    }

    /// Reads one packed or unpacked element run of a repeated varint field into `out`; `read`
    /// decodes one element (the generated parsers' `case <packed tag>` / `case <tag>` pair).
    pub fn read_repeated<T>(
        &mut self,
        tag: u32,
        out: &mut Vec<T>,
        mut read: impl FnMut(&mut Self) -> ProtoResult<T>,
    ) -> ProtoResult<()> {
        if WireFormat::get_tag_wire_type(tag) == WireFormat::WIRETYPE_LENGTH_DELIMITED {
            let length = self.read_raw_varint32()?;
            let limit = self.push_limit(length)?;
            while self.get_bytes_until_limit() > 0 {
                out.push(read(self)?);
            }
            self.pop_limit(limit);
        } else {
            out.push(read(self)?);
        }
        Ok(())
    }
}

/// port: com.google.protobuf.CodedOutputStream (writing into a growable byte array)
#[derive(Default)]
pub struct CodedOutputStream {
    buffer: Vec<u8>,
}

impl CodedOutputStream {
    // port: CodedOutputStream#newInstance(OutputStream)
    pub fn new_instance() -> Self {
        Self::default()
    }
    /// The bytes written so far (Java: the wrapped stream's contents after `flush()`).
    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    // port: CodedOutputStream#writeTag
    pub fn write_tag(&mut self, field_number: u32, wire_type: u32) {
        self.write_uint32_no_tag(WireFormat::make_tag(field_number, wire_type) as i32);
    }
    // port: CodedOutputStream#writeInt32NoTag
    pub fn write_int32_no_tag(&mut self, value: i32) {
        if value >= 0 {
            self.write_uint32_no_tag(value);
        } else {
            // Must sign-extend.
            self.write_uint64_no_tag(value as i64);
        }
    }
    // port: CodedOutputStream#writeUInt32NoTag
    pub fn write_uint32_no_tag(&mut self, value: i32) {
        self.write_uint64_no_tag((value as u32) as i64);
    }
    // port: CodedOutputStream#writeSInt32NoTag
    pub fn write_sint32_no_tag(&mut self, value: i32) {
        self.write_uint32_no_tag(Self::encode_zig_zag32(value));
    }
    // port: CodedOutputStream#writeUInt64NoTag
    pub fn write_uint64_no_tag(&mut self, value: i64) {
        let mut value = value as u64;
        loop {
            if (value & !0x7F) == 0 {
                self.buffer.push(value as u8);
                return;
            }
            self.buffer.push(((value & 0x7F) | 0x80) as u8);
            value >>= 7;
        }
    }
    // port: CodedOutputStream#writeDoubleNoTag
    pub fn write_double_no_tag(&mut self, value: f64) {
        self.buffer
            .extend_from_slice(&value.to_bits().to_le_bytes());
    }
    // port: CodedOutputStream#writeBoolNoTag
    pub fn write_bool_no_tag(&mut self, value: bool) {
        self.buffer.push(u8::from(value));
    }
    // port: CodedOutputStream#writeByteArrayNoTag
    pub fn write_byte_array_no_tag(&mut self, value: &[u8]) {
        self.write_uint32_no_tag(value.len() as i32);
        self.buffer.extend_from_slice(value);
    }
    // port: CodedOutputStream#writeRawBytes
    pub fn write_raw_bytes(&mut self, value: &[u8]) {
        self.buffer.extend_from_slice(value);
    }
    // port: CodedOutputStream#writeMessageNoTag
    pub fn write_message_no_tag<M: Message>(&mut self, value: &M) {
        self.write_uint32_no_tag(value.get_serialized_size() as i32);
        value.write_to(self);
    }

    // port: CodedOutputStream#encodeZigZag32
    pub fn encode_zig_zag32(n: i32) -> i32 {
        (n << 1) ^ (n >> 31)
    }
    // port: CodedOutputStream#computeUInt32SizeNoTag
    pub fn compute_uint32_size_no_tag(value: i32) -> usize {
        Self::compute_uint64_size_no_tag((value as u32) as i64)
    }
    // port: CodedOutputStream#computeInt32SizeNoTag
    pub fn compute_int32_size_no_tag(value: i32) -> usize {
        if value >= 0 {
            Self::compute_uint32_size_no_tag(value)
        } else {
            // Must sign-extend.
            10
        }
    }
    // port: CodedOutputStream#computeSInt32SizeNoTag
    pub fn compute_sint32_size_no_tag(value: i32) -> usize {
        Self::compute_uint32_size_no_tag(Self::encode_zig_zag32(value))
    }
    // port: CodedOutputStream#computeUInt64SizeNoTag
    pub fn compute_uint64_size_no_tag(value: i64) -> usize {
        let value = value as u64;
        let bits = 64 - value.leading_zeros() as usize;
        if bits == 0 { 1 } else { bits.div_ceil(7) }
    }
    // port: CodedOutputStream#computeTagSize
    pub fn compute_tag_size(field_number: u32) -> usize {
        Self::compute_uint32_size_no_tag(WireFormat::make_tag(field_number, 0) as i32)
    }
    // port: CodedOutputStream#computeByteArraySizeNoTag
    pub fn compute_byte_array_size_no_tag(value: &[u8]) -> usize {
        Self::compute_uint32_size_no_tag(value.len() as i32) + value.len()
    }
    // port: CodedOutputStream#computeMessageSizeNoTag
    pub fn compute_message_size_no_tag<M: Message>(value: &M) -> usize {
        let size = value.get_serialized_size();
        Self::compute_uint32_size_no_tag(size as i32) + size
    }
}

/// port: com.google.protobuf.MessageLite (with its Builder: the Rust messages are their own
/// builders).
pub trait Message: Default + Clone + PartialEq {
    /// port: MessageLite.Builder#mergeFrom(CodedInputStream): reads fields until the end of the
    /// current limit or an end-group tag.
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()>;
    /// port: MessageLite#writeTo(CodedOutputStream)
    fn write_to(&self, output: &mut CodedOutputStream);
    /// port: MessageLite#getSerializedSize
    fn get_serialized_size(&self) -> usize;

    // port: MessageLite#parseFrom(byte[])
    fn parse_from(bytes: &[u8]) -> ProtoResult<Self> {
        let mut input = CodedInputStream::new_instance(bytes);
        Self::parse_from_stream(&mut input)
    }
    // port: MessageLite#parseFrom(CodedInputStream)
    fn parse_from_stream(input: &mut CodedInputStream<'_>) -> ProtoResult<Self> {
        let mut message = Self::default();
        message.merge_from(input)?;
        input.check_last_tag_was(0)?;
        Ok(message)
    }
    // port: MessageLite#parseDelimitedFrom(InputStream): `None` at end of input.
    fn parse_delimited_from(input: &mut CodedInputStream<'_>) -> ProtoResult<Option<Self>> {
        if input.is_at_end() {
            return Ok(None);
        }
        let size = input.read_raw_varint32()?;
        let old_limit = input.push_limit(size)?;
        let message = Self::parse_from_stream(input)?;
        if input.get_bytes_until_limit() != 0 {
            return Err(InvalidProtocolBufferException::truncated_message());
        }
        input.pop_limit(old_limit);
        Ok(Some(message))
    }
    // port: MessageLite#toByteArray
    fn to_byte_array(&self) -> Vec<u8> {
        let mut output = CodedOutputStream::new_instance();
        self.write_to(&mut output);
        output.into_bytes()
    }
    // port: MessageLite#writeDelimitedTo(OutputStream)
    fn write_delimited_to(&self, out: &mut Vec<u8>) {
        let mut output = CodedOutputStream::new_instance();
        output.write_uint32_no_tag(self.get_serialized_size() as i32);
        self.write_to(&mut output);
        out.extend_from_slice(&output.into_bytes());
    }
}

/// port: com.google.protobuf.Internal.EnumLite
pub trait ProtoEnum: Copy + Eq {
    /// port: EnumLite#getNumber
    fn get_number(self) -> i32;
    /// port: Enum#forNumber, mapping an unknown number to `UNRECOGNIZED` (proto3 open enums).
    fn for_number_or_unrecognized(value: i32) -> Self;
}

/// port: com.google.protobuf.Utf8#encode (Java `String` -> UTF-8 bytes; an unpaired surrogate
/// becomes '?', as protobuf-java's `Utf8.encodeUtf8` does through `String.getBytes(UTF_8)`).
pub fn encode_utf8_java(value: &closure_rhino::js_string::JsString) -> Vec<u8> {
    let units = value.as_units().iter().copied();
    let mut out = String::with_capacity(units.len());
    for decoded in char::decode_utf16(units) {
        out.push(decoded.unwrap_or('?'));
    }
    out.into_bytes()
}

/// port: com.google.protobuf.Descriptors.FieldDescriptor.Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    DOUBLE,
    FLOAT,
    INT64,
    UINT64,
    INT32,
    FIXED64,
    FIXED32,
    BOOL,
    STRING,
    GROUP,
    MESSAGE,
    BYTES,
    UINT32,
    ENUM,
    SFIXED32,
    SFIXED64,
    SINT32,
    SINT64,
}

/// port: com.google.protobuf.Descriptors.FieldDescriptor (the parts generated code embeds:
/// name, number, type, label, containing oneof and the message/enum type's full name, empty for
/// scalars).
#[derive(Debug)]
pub struct FieldDescriptor {
    pub name: &'static str,
    pub number: u32,
    pub field_type: FieldType,
    pub repeated: bool,
    pub containing_oneof: Option<&'static str>,
    pub type_name: &'static str,
}

/// port: com.google.protobuf.Descriptors.Descriptor (fields in declaration order, as
/// `Descriptor#getFields` returns them).
#[derive(Debug)]
pub struct Descriptor {
    pub full_name: &'static str,
    pub fields: &'static [FieldDescriptor],
}

/// port: com.google.protobuf.Descriptors.EnumDescriptor (values in declaration order).
#[derive(Debug)]
pub struct EnumDescriptor {
    pub full_name: &'static str,
    pub name: &'static str,
    pub values: &'static [(&'static str, i32)],
}

impl EnumDescriptor {
    // port: Descriptors.EnumDescriptor#findValueByNumberCreatingIfUnknown (the value's name)
    pub fn find_value_name_by_number_creating_if_unknown(&self, number: i32) -> String {
        match self.values.iter().find(|(_, value)| *value == number) {
            Some((name, _)) => (*name).to_string(),
            None => format!("UNKNOWN_ENUM_VALUE_{}_{}", self.name, number),
        }
    }
}
