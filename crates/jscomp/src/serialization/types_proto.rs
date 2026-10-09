/*
 * Copyright The Closure Compiler Authors.
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
//   src/com/google/javascript/rhino/typed_ast/types.proto.

// Generated from src/com/google/javascript/rhino/typed_ast/types.proto (protoc's Java API shape; D-003, no
// protoc on the host). Wire format and defaults follow proto3; see protobuf.rs.
#![allow(clippy::all, unused_imports, unused_variables, dead_code)]
use super::protobuf::{
    CodedInputStream, CodedOutputStream, Descriptor, EnumDescriptor, FieldDescriptor, FieldType,
    Message, ProtoEnum, ProtoResult, WireFormat,
};
use closure_rhino::js_string::JsString;
use std::fmt;
use std::sync::LazyLock;
// port: PrimitiveType (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum PrimitiveType {
    #[default]
    UNKNOWN_TYPE = 0,
    BOOLEAN_TYPE = 1,
    STRING_TYPE = 2,
    NUMBER_TYPE = 3,
    NULL_OR_VOID_TYPE = 4,
    SYMBOL_TYPE = 5,
    BIGINT_TYPE = 6,
    TOP_OBJECT = 7,
    TOP_FUNCTION = 8,
    GBIGINT_TYPE = 9,
    UNRECOGNIZED = -1,
}
impl PrimitiveType {
    pub const VALUES: [Self; 11] = [
        Self::UNKNOWN_TYPE,
        Self::BOOLEAN_TYPE,
        Self::STRING_TYPE,
        Self::NUMBER_TYPE,
        Self::NULL_OR_VOID_TYPE,
        Self::SYMBOL_TYPE,
        Self::BIGINT_TYPE,
        Self::TOP_OBJECT,
        Self::TOP_FUNCTION,
        Self::GBIGINT_TYPE,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: PrimitiveType#getNumber
    pub fn get_number(self) -> i32 {
        assert!(
            self != Self::UNRECOGNIZED,
            "Can't get the number of an unknown enum value."
        );
        self as i32
    }
    // port: Enum#name
    pub fn name(self) -> &'static str {
        match self {
            Self::UNKNOWN_TYPE => "UNKNOWN_TYPE",
            Self::BOOLEAN_TYPE => "BOOLEAN_TYPE",
            Self::STRING_TYPE => "STRING_TYPE",
            Self::NUMBER_TYPE => "NUMBER_TYPE",
            Self::NULL_OR_VOID_TYPE => "NULL_OR_VOID_TYPE",
            Self::SYMBOL_TYPE => "SYMBOL_TYPE",
            Self::BIGINT_TYPE => "BIGINT_TYPE",
            Self::TOP_OBJECT => "TOP_OBJECT",
            Self::TOP_FUNCTION => "TOP_FUNCTION",
            Self::GBIGINT_TYPE => "GBIGINT_TYPE",
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: PrimitiveType#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "UNKNOWN_TYPE" => Some(Self::UNKNOWN_TYPE),
            "BOOLEAN_TYPE" => Some(Self::BOOLEAN_TYPE),
            "STRING_TYPE" => Some(Self::STRING_TYPE),
            "NUMBER_TYPE" => Some(Self::NUMBER_TYPE),
            "NULL_OR_VOID_TYPE" => Some(Self::NULL_OR_VOID_TYPE),
            "SYMBOL_TYPE" => Some(Self::SYMBOL_TYPE),
            "BIGINT_TYPE" => Some(Self::BIGINT_TYPE),
            "TOP_OBJECT" => Some(Self::TOP_OBJECT),
            "TOP_FUNCTION" => Some(Self::TOP_FUNCTION),
            "GBIGINT_TYPE" => Some(Self::GBIGINT_TYPE),
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: PrimitiveType#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::UNKNOWN_TYPE),
            1 => Some(Self::BOOLEAN_TYPE),
            2 => Some(Self::STRING_TYPE),
            3 => Some(Self::NUMBER_TYPE),
            4 => Some(Self::NULL_OR_VOID_TYPE),
            5 => Some(Self::SYMBOL_TYPE),
            6 => Some(Self::BIGINT_TYPE),
            7 => Some(Self::TOP_OBJECT),
            8 => Some(Self::TOP_FUNCTION),
            9 => Some(Self::GBIGINT_TYPE),
            _ => None,
        }
    }
}
impl ProtoEnum for PrimitiveType {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for PrimitiveType {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: TypePool (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypePool {
    pub type_: Vec<TypeProto>,
    pub disambiguation_edges: Vec<SubtypingEdge>,
    pub debug_info: Option<TypePoolDebugInfo>,
}
static TYPEPOOL_DEFAULT_INSTANCE: LazyLock<TypePool> = LazyLock::new(TypePool::default);
impl TypePool {
    pub const TYPE_FIELD_NUMBER: i32 = 1;
    pub const DISAMBIGUATION_EDGES_FIELD_NUMBER: i32 = 2;
    pub const DEBUG_INFO_FIELD_NUMBER: i32 = 3;
    // port: TypePool#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypePool#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEPOOL_DEFAULT_INSTANCE
    }
    // port: TypePool#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypePool.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypePool.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypePool#getTypeList
    pub fn get_type_list(&self) -> &[TypeProto] {
        &self.type_
    }
    // port: TypePool#getTypeCount
    pub fn get_type_count(&self) -> i32 {
        self.type_.len() as i32
    }
    // port: TypePool#getType(int)
    pub fn get_type(&self, index: i32) -> &TypeProto {
        &self.type_[index as usize]
    }
    // port: TypePool.Builder#addType
    pub fn add_type(mut self, value: TypeProto) -> Self {
        self.type_.push(value);
        self
    }
    // port: TypePool.Builder#addAllType
    pub fn add_all_type(mut self, values: impl IntoIterator<Item = TypeProto>) -> Self {
        self.type_.extend(values);
        self
    }
    // port: TypePool.Builder#clearType
    pub fn clear_type(mut self) -> Self {
        self.type_.clear();
        self
    }
    // port: TypePool#getDisambiguationEdgesList
    pub fn get_disambiguation_edges_list(&self) -> &[SubtypingEdge] {
        &self.disambiguation_edges
    }
    // port: TypePool#getDisambiguationEdgesCount
    pub fn get_disambiguation_edges_count(&self) -> i32 {
        self.disambiguation_edges.len() as i32
    }
    // port: TypePool#getDisambiguationEdges(int)
    pub fn get_disambiguation_edges(&self, index: i32) -> &SubtypingEdge {
        &self.disambiguation_edges[index as usize]
    }
    // port: TypePool.Builder#addDisambiguationEdges
    pub fn add_disambiguation_edges(mut self, value: SubtypingEdge) -> Self {
        self.disambiguation_edges.push(value);
        self
    }
    // port: TypePool.Builder#addAllDisambiguationEdges
    pub fn add_all_disambiguation_edges(
        mut self,
        values: impl IntoIterator<Item = SubtypingEdge>,
    ) -> Self {
        self.disambiguation_edges.extend(values);
        self
    }
    // port: TypePool.Builder#clearDisambiguationEdges
    pub fn clear_disambiguation_edges(mut self) -> Self {
        self.disambiguation_edges.clear();
        self
    }
    // port: TypePool#hasDebugInfo
    pub fn has_debug_info(&self) -> bool {
        self.debug_info.is_some()
    }
    // port: TypePool#getDebugInfo
    pub fn get_debug_info(&self) -> &TypePoolDebugInfo {
        self.debug_info
            .as_ref()
            .unwrap_or_else(|| TypePoolDebugInfo::default_instance_ref())
    }
    // port: TypePool.Builder#setDebugInfo
    pub fn set_debug_info(mut self, value: TypePoolDebugInfo) -> Self {
        self.debug_info = Some(value);
        self
    }
    // port: TypePool.Builder#clearDebugInfo
    pub fn clear_debug_info(mut self) -> Self {
        self.debug_info = None;
        self
    }
}
impl Message for TypePool {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = TypeProto::default();
                    input.read_message(&mut v)?;
                    self.type_.push(v);
                }
                18 => {
                    let mut v = SubtypingEdge::default();
                    input.read_message(&mut v)?;
                    self.disambiguation_edges.push(v);
                }
                26 => {
                    let mut v = self.debug_info.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.debug_info = Some(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        for v in &self.type_ {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.disambiguation_edges {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.debug_info {
            output.write_tag(3, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.type_ {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.disambiguation_edges {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.debug_info {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: TypePool.DebugInfo (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypePoolDebugInfo {
    pub mismatch: Vec<TypePoolDebugInfoMismatch>,
}
static TYPEPOOLDEBUGINFO_DEFAULT_INSTANCE: LazyLock<TypePoolDebugInfo> =
    LazyLock::new(TypePoolDebugInfo::default);
impl TypePoolDebugInfo {
    pub const MISMATCH_FIELD_NUMBER: i32 = 1;
    // port: TypePoolDebugInfo#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypePoolDebugInfo#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEPOOLDEBUGINFO_DEFAULT_INSTANCE
    }
    // port: TypePoolDebugInfo#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypePoolDebugInfo.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypePoolDebugInfo.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypePoolDebugInfo#getMismatchList
    pub fn get_mismatch_list(&self) -> &[TypePoolDebugInfoMismatch] {
        &self.mismatch
    }
    // port: TypePoolDebugInfo#getMismatchCount
    pub fn get_mismatch_count(&self) -> i32 {
        self.mismatch.len() as i32
    }
    // port: TypePoolDebugInfo#getMismatch(int)
    pub fn get_mismatch(&self, index: i32) -> &TypePoolDebugInfoMismatch {
        &self.mismatch[index as usize]
    }
    // port: TypePoolDebugInfo.Builder#addMismatch
    pub fn add_mismatch(mut self, value: TypePoolDebugInfoMismatch) -> Self {
        self.mismatch.push(value);
        self
    }
    // port: TypePoolDebugInfo.Builder#addAllMismatch
    pub fn add_all_mismatch(
        mut self,
        values: impl IntoIterator<Item = TypePoolDebugInfoMismatch>,
    ) -> Self {
        self.mismatch.extend(values);
        self
    }
    // port: TypePoolDebugInfo.Builder#clearMismatch
    pub fn clear_mismatch(mut self) -> Self {
        self.mismatch.clear();
        self
    }
}
impl Message for TypePoolDebugInfo {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = TypePoolDebugInfoMismatch::default();
                    input.read_message(&mut v)?;
                    self.mismatch.push(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        for v in &self.mismatch {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.mismatch {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: TypePool.DebugInfo.Mismatch (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypePoolDebugInfoMismatch {
    pub source_ref: String,
    pub involved_color: Vec<i32>,
}
static TYPEPOOLDEBUGINFOMISMATCH_DEFAULT_INSTANCE: LazyLock<TypePoolDebugInfoMismatch> =
    LazyLock::new(TypePoolDebugInfoMismatch::default);
impl TypePoolDebugInfoMismatch {
    pub const SOURCE_REF_FIELD_NUMBER: i32 = 1;
    pub const INVOLVED_COLOR_FIELD_NUMBER: i32 = 2;
    // port: TypePoolDebugInfoMismatch#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypePoolDebugInfoMismatch#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEPOOLDEBUGINFOMISMATCH_DEFAULT_INSTANCE
    }
    // port: TypePoolDebugInfoMismatch#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypePoolDebugInfoMismatch.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypePoolDebugInfoMismatch.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypePoolDebugInfoMismatch#getSourceRef
    pub fn get_source_ref(&self) -> &str {
        &self.source_ref
    }
    // port: TypePoolDebugInfoMismatch.Builder#setSourceRef
    pub fn set_source_ref(mut self, value: impl Into<String>) -> Self {
        self.source_ref = value.into();
        self
    }
    // port: TypePoolDebugInfoMismatch#getInvolvedColorList
    pub fn get_involved_color_list(&self) -> &[i32] {
        &self.involved_color
    }
    // port: TypePoolDebugInfoMismatch#getInvolvedColorCount
    pub fn get_involved_color_count(&self) -> i32 {
        self.involved_color.len() as i32
    }
    // port: TypePoolDebugInfoMismatch#getInvolvedColor(int)
    pub fn get_involved_color(&self, index: i32) -> i32 {
        self.involved_color[index as usize]
    }
    // port: TypePoolDebugInfoMismatch.Builder#addInvolvedColor
    pub fn add_involved_color(mut self, value: i32) -> Self {
        self.involved_color.push(value);
        self
    }
    // port: TypePoolDebugInfoMismatch.Builder#addAllInvolvedColor
    pub fn add_all_involved_color(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.involved_color.extend(values);
        self
    }
    // port: TypePoolDebugInfoMismatch.Builder#clearInvolvedColor
    pub fn clear_involved_color(mut self) -> Self {
        self.involved_color.clear();
        self
    }
}
impl Message for TypePoolDebugInfoMismatch {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.source_ref = input.read_string_require_utf8()?;
                }
                16 | 18 => {
                    input.read_repeated(tag, &mut self.involved_color, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if !self.source_ref.is_empty() {
            let v = &self.source_ref;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if !self.involved_color.is_empty() {
            let data_size: usize = self
                .involved_color
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(2, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.involved_color {
                output.write_int32_no_tag(*v);
            }
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.source_ref.is_empty() {
            let v = &self.source_ref;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if !self.involved_color.is_empty() {
            let data_size: usize = self
                .involved_color
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        size
    }
}
// port: SubtypingEdge (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SubtypingEdge {
    pub subtype: i32,
    pub supertype: i32,
}
static SUBTYPINGEDGE_DEFAULT_INSTANCE: LazyLock<SubtypingEdge> =
    LazyLock::new(SubtypingEdge::default);
impl SubtypingEdge {
    pub const SUBTYPE_FIELD_NUMBER: i32 = 1;
    pub const SUPERTYPE_FIELD_NUMBER: i32 = 2;
    // port: SubtypingEdge#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: SubtypingEdge#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &SUBTYPINGEDGE_DEFAULT_INSTANCE
    }
    // port: SubtypingEdge#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: SubtypingEdge.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: SubtypingEdge.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: SubtypingEdge#getSubtype
    pub fn get_subtype(&self) -> i32 {
        self.subtype
    }
    // port: SubtypingEdge.Builder#setSubtype
    pub fn set_subtype(mut self, value: i32) -> Self {
        self.subtype = value;
        self
    }
    // port: SubtypingEdge#getSupertype
    pub fn get_supertype(&self) -> i32 {
        self.supertype
    }
    // port: SubtypingEdge.Builder#setSupertype
    pub fn set_supertype(mut self, value: i32) -> Self {
        self.supertype = value;
        self
    }
}
impl Message for SubtypingEdge {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 => {
                    self.subtype = input.read_int32()?;
                }
                16 => {
                    self.supertype = input.read_int32()?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if self.subtype != 0 {
            let v = &self.subtype;
            output.write_tag(1, 0);
            output.write_int32_no_tag(*v);
        }
        if self.supertype != 0 {
            let v = &self.supertype;
            output.write_tag(2, 0);
            output.write_int32_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if self.subtype != 0 {
            let v = &self.subtype;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.supertype != 0 {
            let v = &self.supertype;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        size
    }
}
// port: UnionTypeProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UnionTypeProto {
    pub union_member: Vec<i32>,
}
static UNIONTYPEPROTO_DEFAULT_INSTANCE: LazyLock<UnionTypeProto> =
    LazyLock::new(UnionTypeProto::default);
impl UnionTypeProto {
    pub const UNION_MEMBER_FIELD_NUMBER: i32 = 1;
    // port: UnionTypeProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: UnionTypeProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &UNIONTYPEPROTO_DEFAULT_INSTANCE
    }
    // port: UnionTypeProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: UnionTypeProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: UnionTypeProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: UnionTypeProto#getUnionMemberList
    pub fn get_union_member_list(&self) -> &[i32] {
        &self.union_member
    }
    // port: UnionTypeProto#getUnionMemberCount
    pub fn get_union_member_count(&self) -> i32 {
        self.union_member.len() as i32
    }
    // port: UnionTypeProto#getUnionMember(int)
    pub fn get_union_member(&self, index: i32) -> i32 {
        self.union_member[index as usize]
    }
    // port: UnionTypeProto.Builder#addUnionMember
    pub fn add_union_member(mut self, value: i32) -> Self {
        self.union_member.push(value);
        self
    }
    // port: UnionTypeProto.Builder#addAllUnionMember
    pub fn add_all_union_member(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.union_member.extend(values);
        self
    }
    // port: UnionTypeProto.Builder#clearUnionMember
    pub fn clear_union_member(mut self) -> Self {
        self.union_member.clear();
        self
    }
}
impl Message for UnionTypeProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 | 10 => {
                    input.read_repeated(tag, &mut self.union_member, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if !self.union_member.is_empty() {
            let data_size: usize = self
                .union_member
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(1, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.union_member {
                output.write_int32_no_tag(*v);
            }
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.union_member.is_empty() {
            let data_size: usize = self
                .union_member
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        size
    }
}
// port: ObjectTypeProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ObjectTypeProto {
    pub is_invalidating: bool,
    pub uuid: Vec<u8>,
    pub properties_keep_original_name: bool,
    pub instance_type: Vec<i32>,
    pub prototype: Vec<i32>,
    pub marked_constructor: bool,
    pub own_property: Vec<i32>,
    pub closure_assert: bool,
    pub debug_info: Option<ObjectTypeProtoDebugInfo>,
}
static OBJECTTYPEPROTO_DEFAULT_INSTANCE: LazyLock<ObjectTypeProto> =
    LazyLock::new(ObjectTypeProto::default);
impl ObjectTypeProto {
    pub const IS_INVALIDATING_FIELD_NUMBER: i32 = 2;
    pub const UUID_FIELD_NUMBER: i32 = 3;
    pub const PROPERTIES_KEEP_ORIGINAL_NAME_FIELD_NUMBER: i32 = 4;
    pub const INSTANCE_TYPE_FIELD_NUMBER: i32 = 5;
    pub const PROTOTYPE_FIELD_NUMBER: i32 = 6;
    pub const MARKED_CONSTRUCTOR_FIELD_NUMBER: i32 = 7;
    pub const OWN_PROPERTY_FIELD_NUMBER: i32 = 8;
    pub const CLOSURE_ASSERT_FIELD_NUMBER: i32 = 9;
    pub const DEBUG_INFO_FIELD_NUMBER: i32 = 15;
    // port: ObjectTypeProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ObjectTypeProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &OBJECTTYPEPROTO_DEFAULT_INSTANCE
    }
    // port: ObjectTypeProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ObjectTypeProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ObjectTypeProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ObjectTypeProto#getIsInvalidating
    pub fn get_is_invalidating(&self) -> bool {
        self.is_invalidating
    }
    // port: ObjectTypeProto.Builder#setIsInvalidating
    pub fn set_is_invalidating(mut self, value: bool) -> Self {
        self.is_invalidating = value;
        self
    }
    // port: ObjectTypeProto#getUuid
    pub fn get_uuid(&self) -> &[u8] {
        &self.uuid
    }
    // port: ObjectTypeProto.Builder#setUuid
    pub fn set_uuid(mut self, value: Vec<u8>) -> Self {
        self.uuid = value;
        self
    }
    // port: ObjectTypeProto#getPropertiesKeepOriginalName
    pub fn get_properties_keep_original_name(&self) -> bool {
        self.properties_keep_original_name
    }
    // port: ObjectTypeProto.Builder#setPropertiesKeepOriginalName
    pub fn set_properties_keep_original_name(mut self, value: bool) -> Self {
        self.properties_keep_original_name = value;
        self
    }
    // port: ObjectTypeProto#getInstanceTypeList
    pub fn get_instance_type_list(&self) -> &[i32] {
        &self.instance_type
    }
    // port: ObjectTypeProto#getInstanceTypeCount
    pub fn get_instance_type_count(&self) -> i32 {
        self.instance_type.len() as i32
    }
    // port: ObjectTypeProto#getInstanceType(int)
    pub fn get_instance_type(&self, index: i32) -> i32 {
        self.instance_type[index as usize]
    }
    // port: ObjectTypeProto.Builder#addInstanceType
    pub fn add_instance_type(mut self, value: i32) -> Self {
        self.instance_type.push(value);
        self
    }
    // port: ObjectTypeProto.Builder#addAllInstanceType
    pub fn add_all_instance_type(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.instance_type.extend(values);
        self
    }
    // port: ObjectTypeProto.Builder#clearInstanceType
    pub fn clear_instance_type(mut self) -> Self {
        self.instance_type.clear();
        self
    }
    // port: ObjectTypeProto#getPrototypeList
    pub fn get_prototype_list(&self) -> &[i32] {
        &self.prototype
    }
    // port: ObjectTypeProto#getPrototypeCount
    pub fn get_prototype_count(&self) -> i32 {
        self.prototype.len() as i32
    }
    // port: ObjectTypeProto#getPrototype(int)
    pub fn get_prototype(&self, index: i32) -> i32 {
        self.prototype[index as usize]
    }
    // port: ObjectTypeProto.Builder#addPrototype
    pub fn add_prototype(mut self, value: i32) -> Self {
        self.prototype.push(value);
        self
    }
    // port: ObjectTypeProto.Builder#addAllPrototype
    pub fn add_all_prototype(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.prototype.extend(values);
        self
    }
    // port: ObjectTypeProto.Builder#clearPrototype
    pub fn clear_prototype(mut self) -> Self {
        self.prototype.clear();
        self
    }
    // port: ObjectTypeProto#getMarkedConstructor
    pub fn get_marked_constructor(&self) -> bool {
        self.marked_constructor
    }
    // port: ObjectTypeProto.Builder#setMarkedConstructor
    pub fn set_marked_constructor(mut self, value: bool) -> Self {
        self.marked_constructor = value;
        self
    }
    // port: ObjectTypeProto#getOwnPropertyList
    pub fn get_own_property_list(&self) -> &[i32] {
        &self.own_property
    }
    // port: ObjectTypeProto#getOwnPropertyCount
    pub fn get_own_property_count(&self) -> i32 {
        self.own_property.len() as i32
    }
    // port: ObjectTypeProto#getOwnProperty(int)
    pub fn get_own_property(&self, index: i32) -> i32 {
        self.own_property[index as usize]
    }
    // port: ObjectTypeProto.Builder#addOwnProperty
    pub fn add_own_property(mut self, value: i32) -> Self {
        self.own_property.push(value);
        self
    }
    // port: ObjectTypeProto.Builder#addAllOwnProperty
    pub fn add_all_own_property(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.own_property.extend(values);
        self
    }
    // port: ObjectTypeProto.Builder#clearOwnProperty
    pub fn clear_own_property(mut self) -> Self {
        self.own_property.clear();
        self
    }
    // port: ObjectTypeProto#getClosureAssert
    pub fn get_closure_assert(&self) -> bool {
        self.closure_assert
    }
    // port: ObjectTypeProto.Builder#setClosureAssert
    pub fn set_closure_assert(mut self, value: bool) -> Self {
        self.closure_assert = value;
        self
    }
    // port: ObjectTypeProto#hasDebugInfo
    pub fn has_debug_info(&self) -> bool {
        self.debug_info.is_some()
    }
    // port: ObjectTypeProto#getDebugInfo
    pub fn get_debug_info(&self) -> &ObjectTypeProtoDebugInfo {
        self.debug_info
            .as_ref()
            .unwrap_or_else(|| ObjectTypeProtoDebugInfo::default_instance_ref())
    }
    // port: ObjectTypeProto.Builder#setDebugInfo
    pub fn set_debug_info(mut self, value: ObjectTypeProtoDebugInfo) -> Self {
        self.debug_info = Some(value);
        self
    }
    // port: ObjectTypeProto.Builder#clearDebugInfo
    pub fn clear_debug_info(mut self) -> Self {
        self.debug_info = None;
        self
    }
}
impl Message for ObjectTypeProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                16 => {
                    self.is_invalidating = input.read_bool()?;
                }
                26 => {
                    self.uuid = input.read_bytes()?;
                }
                32 => {
                    self.properties_keep_original_name = input.read_bool()?;
                }
                40 | 42 => {
                    input.read_repeated(tag, &mut self.instance_type, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                48 | 50 => {
                    input
                        .read_repeated(tag, &mut self.prototype, |input| Ok(input.read_int32()?))?;
                }
                56 => {
                    self.marked_constructor = input.read_bool()?;
                }
                64 | 66 => {
                    input.read_repeated(tag, &mut self.own_property, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                72 => {
                    self.closure_assert = input.read_bool()?;
                }
                122 => {
                    let mut v = self.debug_info.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.debug_info = Some(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if self.is_invalidating {
            let v = &self.is_invalidating;
            output.write_tag(2, 0);
            output.write_bool_no_tag(*v);
        }
        if !self.uuid.is_empty() {
            let v = &self.uuid;
            output.write_tag(3, 2);
            output.write_byte_array_no_tag(v);
        }
        if self.properties_keep_original_name {
            let v = &self.properties_keep_original_name;
            output.write_tag(4, 0);
            output.write_bool_no_tag(*v);
        }
        if !self.instance_type.is_empty() {
            let data_size: usize = self
                .instance_type
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(5, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.instance_type {
                output.write_int32_no_tag(*v);
            }
        }
        if !self.prototype.is_empty() {
            let data_size: usize = self
                .prototype
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(6, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.prototype {
                output.write_int32_no_tag(*v);
            }
        }
        if self.marked_constructor {
            let v = &self.marked_constructor;
            output.write_tag(7, 0);
            output.write_bool_no_tag(*v);
        }
        if !self.own_property.is_empty() {
            let data_size: usize = self
                .own_property
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(8, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.own_property {
                output.write_int32_no_tag(*v);
            }
        }
        if self.closure_assert {
            let v = &self.closure_assert;
            output.write_tag(9, 0);
            output.write_bool_no_tag(*v);
        }
        if let Some(v) = &self.debug_info {
            output.write_tag(15, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if self.is_invalidating {
            let v = &self.is_invalidating;
            size += CodedOutputStream::compute_tag_size(2) + 1;
        }
        if !self.uuid.is_empty() {
            let v = &self.uuid;
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_byte_array_size_no_tag(v);
        }
        if self.properties_keep_original_name {
            let v = &self.properties_keep_original_name;
            size += CodedOutputStream::compute_tag_size(4) + 1;
        }
        if !self.instance_type.is_empty() {
            let data_size: usize = self
                .instance_type
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        if !self.prototype.is_empty() {
            let data_size: usize = self
                .prototype
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        if self.marked_constructor {
            let v = &self.marked_constructor;
            size += CodedOutputStream::compute_tag_size(7) + 1;
        }
        if !self.own_property.is_empty() {
            let data_size: usize = self
                .own_property
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(8)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        if self.closure_assert {
            let v = &self.closure_assert;
            size += CodedOutputStream::compute_tag_size(9) + 1;
        }
        if let Some(v) = &self.debug_info {
            size += CodedOutputStream::compute_tag_size(15)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: ObjectTypeProto.DebugInfo (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ObjectTypeProtoDebugInfo {
    pub typename_pointer: Vec<i32>,
}
static OBJECTTYPEPROTODEBUGINFO_DEFAULT_INSTANCE: LazyLock<ObjectTypeProtoDebugInfo> =
    LazyLock::new(ObjectTypeProtoDebugInfo::default);
impl ObjectTypeProtoDebugInfo {
    pub const TYPENAME_POINTER_FIELD_NUMBER: i32 = 1;
    // port: ObjectTypeProtoDebugInfo#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ObjectTypeProtoDebugInfo#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &OBJECTTYPEPROTODEBUGINFO_DEFAULT_INSTANCE
    }
    // port: ObjectTypeProtoDebugInfo#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ObjectTypeProtoDebugInfo.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ObjectTypeProtoDebugInfo.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ObjectTypeProtoDebugInfo#getTypenamePointerList
    pub fn get_typename_pointer_list(&self) -> &[i32] {
        &self.typename_pointer
    }
    // port: ObjectTypeProtoDebugInfo#getTypenamePointerCount
    pub fn get_typename_pointer_count(&self) -> i32 {
        self.typename_pointer.len() as i32
    }
    // port: ObjectTypeProtoDebugInfo#getTypenamePointer(int)
    pub fn get_typename_pointer(&self, index: i32) -> i32 {
        self.typename_pointer[index as usize]
    }
    // port: ObjectTypeProtoDebugInfo.Builder#addTypenamePointer
    pub fn add_typename_pointer(mut self, value: i32) -> Self {
        self.typename_pointer.push(value);
        self
    }
    // port: ObjectTypeProtoDebugInfo.Builder#addAllTypenamePointer
    pub fn add_all_typename_pointer(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.typename_pointer.extend(values);
        self
    }
    // port: ObjectTypeProtoDebugInfo.Builder#clearTypenamePointer
    pub fn clear_typename_pointer(mut self) -> Self {
        self.typename_pointer.clear();
        self
    }
}
impl Message for ObjectTypeProtoDebugInfo {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 | 10 => {
                    input.read_repeated(tag, &mut self.typename_pointer, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if !self.typename_pointer.is_empty() {
            let data_size: usize = self
                .typename_pointer
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(1, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.typename_pointer {
                output.write_int32_no_tag(*v);
            }
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.typename_pointer.is_empty() {
            let data_size: usize = self
                .typename_pointer
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        size
    }
}
// port: TypeProto#kind (oneof)
#[derive(Clone, Debug, PartialEq, Default)]
pub enum TypeProtoKind {
    OBJECT(ObjectTypeProto),
    UNION(UnionTypeProto),
    #[default]
    KIND_NOT_SET,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeProtoKindCase {
    OBJECT,
    UNION,
    KIND_NOT_SET,
}
// port: TypeProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypeProto {
    pub kind: TypeProtoKind,
}
static TYPEPROTO_DEFAULT_INSTANCE: LazyLock<TypeProto> = LazyLock::new(TypeProto::default);
impl TypeProto {
    pub const OBJECT_FIELD_NUMBER: i32 = 2;
    pub const UNION_FIELD_NUMBER: i32 = 4;
    // port: TypeProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypeProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEPROTO_DEFAULT_INSTANCE
    }
    // port: TypeProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypeProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypeProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypeProto#hasObject
    pub fn has_object(&self) -> bool {
        matches!(self.kind, TypeProtoKind::OBJECT(_))
    }
    // port: TypeProto#getObject
    pub fn get_object(&self) -> &ObjectTypeProto {
        if let TypeProtoKind::OBJECT(v) = &self.kind {
            v
        } else {
            ObjectTypeProto::default_instance_ref()
        }
    }
    // port: TypeProto.Builder#setObject
    pub fn set_object(mut self, value: ObjectTypeProto) -> Self {
        self.kind = TypeProtoKind::OBJECT(value);
        self
    }
    // port: TypeProto#hasUnion
    pub fn has_union(&self) -> bool {
        matches!(self.kind, TypeProtoKind::UNION(_))
    }
    // port: TypeProto#getUnion
    pub fn get_union(&self) -> &UnionTypeProto {
        if let TypeProtoKind::UNION(v) = &self.kind {
            v
        } else {
            UnionTypeProto::default_instance_ref()
        }
    }
    // port: TypeProto.Builder#setUnion
    pub fn set_union(mut self, value: UnionTypeProto) -> Self {
        self.kind = TypeProtoKind::UNION(value);
        self
    }
    // port: TypeProto#getKindCase
    pub fn get_kind_case(&self) -> TypeProtoKindCase {
        match self.kind {
            TypeProtoKind::OBJECT(_) => TypeProtoKindCase::OBJECT,
            TypeProtoKind::UNION(_) => TypeProtoKindCase::UNION,
            TypeProtoKind::KIND_NOT_SET => TypeProtoKindCase::KIND_NOT_SET,
        }
    }
}
impl Message for TypeProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                18 => {
                    let mut v = if let TypeProtoKind::OBJECT(v) = std::mem::take(&mut self.kind) {
                        v
                    } else {
                        ObjectTypeProto::default()
                    };
                    input.read_message(&mut v)?;
                    self.kind = TypeProtoKind::OBJECT(v);
                }
                34 => {
                    let mut v = if let TypeProtoKind::UNION(v) = std::mem::take(&mut self.kind) {
                        v
                    } else {
                        UnionTypeProto::default()
                    };
                    input.read_message(&mut v)?;
                    self.kind = TypeProtoKind::UNION(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if let TypeProtoKind::OBJECT(v) = &self.kind {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
        if let TypeProtoKind::UNION(v) = &self.kind {
            output.write_tag(4, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if let TypeProtoKind::OBJECT(v) = &self.kind {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let TypeProtoKind::UNION(v) = &self.kind {
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: TypePoolList (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypePoolList {
    pub type_pool: Vec<TypePool>,
}
static TYPEPOOLLIST_DEFAULT_INSTANCE: LazyLock<TypePoolList> = LazyLock::new(TypePoolList::default);
impl TypePoolList {
    pub const TYPE_POOL_FIELD_NUMBER: i32 = 1;
    // port: TypePoolList#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypePoolList#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEPOOLLIST_DEFAULT_INSTANCE
    }
    // port: TypePoolList#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypePoolList.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypePoolList.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypePoolList#getTypePoolList
    pub fn get_type_pool_list(&self) -> &[TypePool] {
        &self.type_pool
    }
    // port: TypePoolList#getTypePoolCount
    pub fn get_type_pool_count(&self) -> i32 {
        self.type_pool.len() as i32
    }
    // port: TypePoolList#getTypePool(int)
    pub fn get_type_pool(&self, index: i32) -> &TypePool {
        &self.type_pool[index as usize]
    }
    // port: TypePoolList.Builder#addTypePool
    pub fn add_type_pool(mut self, value: TypePool) -> Self {
        self.type_pool.push(value);
        self
    }
    // port: TypePoolList.Builder#addAllTypePool
    pub fn add_all_type_pool(mut self, values: impl IntoIterator<Item = TypePool>) -> Self {
        self.type_pool.extend(values);
        self
    }
    // port: TypePoolList.Builder#clearTypePool
    pub fn clear_type_pool(mut self) -> Self {
        self.type_pool.clear();
        self
    }
}
impl Message for TypePoolList {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = TypePool::default();
                    input.read_message(&mut v)?;
                    self.type_pool.push(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        for v in &self.type_pool {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.type_pool {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: Descriptors.FileDescriptor of types.proto (the message descriptors protoc embeds)
pub static MESSAGE_DESCRIPTORS: &[Descriptor] = &[
    Descriptor {
        full_name: "jscomp.TypePool",
        fields: &[
            FieldDescriptor {
                name: "type",
                number: 1,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.TypeProto",
            },
            FieldDescriptor {
                name: "disambiguation_edges",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.SubtypingEdge",
            },
            FieldDescriptor {
                name: "debug_info",
                number: 3,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.TypePool.DebugInfo",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.TypePool.DebugInfo",
        fields: &[FieldDescriptor {
            name: "mismatch",
            number: 1,
            field_type: FieldType::MESSAGE,
            repeated: true,
            containing_oneof: None,
            type_name: "jscomp.TypePool.DebugInfo.Mismatch",
        }],
    },
    Descriptor {
        full_name: "jscomp.TypePool.DebugInfo.Mismatch",
        fields: &[
            FieldDescriptor {
                name: "source_ref",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "involved_color",
                number: 2,
                field_type: FieldType::INT32,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.SubtypingEdge",
        fields: &[
            FieldDescriptor {
                name: "subtype",
                number: 1,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "supertype",
                number: 2,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.UnionTypeProto",
        fields: &[FieldDescriptor {
            name: "union_member",
            number: 1,
            field_type: FieldType::INT32,
            repeated: true,
            containing_oneof: None,
            type_name: "",
        }],
    },
    Descriptor {
        full_name: "jscomp.ObjectTypeProto",
        fields: &[
            FieldDescriptor {
                name: "is_invalidating",
                number: 2,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "uuid",
                number: 3,
                field_type: FieldType::BYTES,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "properties_keep_original_name",
                number: 4,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "instance_type",
                number: 5,
                field_type: FieldType::INT32,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "prototype",
                number: 6,
                field_type: FieldType::INT32,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "marked_constructor",
                number: 7,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "own_property",
                number: 8,
                field_type: FieldType::INT32,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "closure_assert",
                number: 9,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "debug_info",
                number: 15,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.ObjectTypeProto.DebugInfo",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.ObjectTypeProto.DebugInfo",
        fields: &[FieldDescriptor {
            name: "typename_pointer",
            number: 1,
            field_type: FieldType::INT32,
            repeated: true,
            containing_oneof: None,
            type_name: "",
        }],
    },
    Descriptor {
        full_name: "jscomp.TypeProto",
        fields: &[
            FieldDescriptor {
                name: "object",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: Some("kind"),
                type_name: "jscomp.ObjectTypeProto",
            },
            FieldDescriptor {
                name: "union",
                number: 4,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: Some("kind"),
                type_name: "jscomp.UnionTypeProto",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.TypePoolList",
        fields: &[FieldDescriptor {
            name: "type_pool",
            number: 1,
            field_type: FieldType::MESSAGE,
            repeated: true,
            containing_oneof: None,
            type_name: "jscomp.TypePool",
        }],
    },
];
// port: Descriptors.FileDescriptor of types.proto (the enum descriptors protoc embeds)
pub static ENUM_DESCRIPTORS: &[EnumDescriptor] = &[EnumDescriptor {
    full_name: "jscomp.PrimitiveType",
    name: "PrimitiveType",
    values: &[
        ("UNKNOWN_TYPE", 0),
        ("BOOLEAN_TYPE", 1),
        ("STRING_TYPE", 2),
        ("NUMBER_TYPE", 3),
        ("NULL_OR_VOID_TYPE", 4),
        ("SYMBOL_TYPE", 5),
        ("BIGINT_TYPE", 6),
        ("TOP_OBJECT", 7),
        ("TOP_FUNCTION", 8),
        ("GBIGINT_TYPE", 9),
    ],
}];
