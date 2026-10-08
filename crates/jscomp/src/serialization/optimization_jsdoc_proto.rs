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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/typed_ast/optimization_jsdoc.proto.

// Generated from src/com/google/javascript/rhino/typed_ast/optimization_jsdoc.proto (protoc's Java API shape; D-003, no
// protoc on the host). Wire format and defaults follow proto3; see protobuf.rs.
#![allow(clippy::all, unused_imports, unused_variables, dead_code)]
use super::protobuf::{
    CodedInputStream, CodedOutputStream, Descriptor, EnumDescriptor, FieldDescriptor, FieldType,
    Message, ProtoEnum, ProtoResult, WireFormat,
};
use closure_rhino::js_string::JsString;
use std::fmt;
use std::sync::LazyLock;
// port: JsdocTag (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum JsdocTag {
    #[default]
    JSDOC_UNSPECIFIED = 0,
    JSDOC_NO_INLINE = 1,
    JSDOC_DEFINE = 2,
    JSDOC_MODIFIES_THIS = 3,
    JSDOC_MODIFIES_ARGUMENTS = 4,
    JSDOC_THROWS = 6,
    JSDOC_NO_SIDE_EFFECTS = 7,
    JSDOC_CONSTRUCTOR = 8,
    JSDOC_INTERFACE = 9,
    JSDOC_ENUM = 10,
    JSDOC_NO_COLLAPSE = 11,
    JSDOC_THIS = 12,
    JSDOC_CONST = 13,
    JSDOC_ID_GENERATOR_CONSISTENT = 14,
    JSDOC_ID_GENERATOR_INCONSISTENT = 15,
    JSDOC_ID_GENERATOR_STABLE = 16,
    JSDOC_ID_GENERATOR_MAPPED = 17,
    JSDOC_ID_GENERATOR_XID = 18,
    JSDOC_ABSTRACT = 19,
    JSDOC_SUPPRESS_MESSAGE_CONVENTION = 22,
    JSDOC_SUPPRESS_PARTIAL_ALIAS = 23,
    JSDOC_PURE_OR_BREAK_MY_CODE = 24,
    JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE = 25,
    JSDOC_FILEOVERVIEW = 26,
    JSDOC_PROVIDE_GOOG = 31,
    JSDOC_TYPE_SUMMARY_FILE = 32,
    JSDOC_PROVIDE_ALREADY_PROVIDED = 33,
    JSDOC_SASS_GENERATED_CSS_TS = 34,
    JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES = 36,
    JSDOC_NO_COVERAGE = 37,
    JSDOC_REQUIRE_INLINING = 38,
    JSDOC_USED_VIA_DOT_CONSTRUCTOR = 39,
    JSDOC_ENCOURAGE_INLINING = 40,
    UNRECOGNIZED = -1,
}
impl JsdocTag {
    pub const VALUES: [Self; 34] = [
        Self::JSDOC_UNSPECIFIED,
        Self::JSDOC_NO_INLINE,
        Self::JSDOC_DEFINE,
        Self::JSDOC_MODIFIES_THIS,
        Self::JSDOC_MODIFIES_ARGUMENTS,
        Self::JSDOC_THROWS,
        Self::JSDOC_NO_SIDE_EFFECTS,
        Self::JSDOC_CONSTRUCTOR,
        Self::JSDOC_INTERFACE,
        Self::JSDOC_ENUM,
        Self::JSDOC_NO_COLLAPSE,
        Self::JSDOC_THIS,
        Self::JSDOC_CONST,
        Self::JSDOC_ID_GENERATOR_CONSISTENT,
        Self::JSDOC_ID_GENERATOR_INCONSISTENT,
        Self::JSDOC_ID_GENERATOR_STABLE,
        Self::JSDOC_ID_GENERATOR_MAPPED,
        Self::JSDOC_ID_GENERATOR_XID,
        Self::JSDOC_ABSTRACT,
        Self::JSDOC_SUPPRESS_MESSAGE_CONVENTION,
        Self::JSDOC_SUPPRESS_PARTIAL_ALIAS,
        Self::JSDOC_PURE_OR_BREAK_MY_CODE,
        Self::JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE,
        Self::JSDOC_FILEOVERVIEW,
        Self::JSDOC_PROVIDE_GOOG,
        Self::JSDOC_TYPE_SUMMARY_FILE,
        Self::JSDOC_PROVIDE_ALREADY_PROVIDED,
        Self::JSDOC_SASS_GENERATED_CSS_TS,
        Self::JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES,
        Self::JSDOC_NO_COVERAGE,
        Self::JSDOC_REQUIRE_INLINING,
        Self::JSDOC_USED_VIA_DOT_CONSTRUCTOR,
        Self::JSDOC_ENCOURAGE_INLINING,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: JsdocTag#getNumber
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
            Self::JSDOC_UNSPECIFIED => "JSDOC_UNSPECIFIED",
            Self::JSDOC_NO_INLINE => "JSDOC_NO_INLINE",
            Self::JSDOC_DEFINE => "JSDOC_DEFINE",
            Self::JSDOC_MODIFIES_THIS => "JSDOC_MODIFIES_THIS",
            Self::JSDOC_MODIFIES_ARGUMENTS => "JSDOC_MODIFIES_ARGUMENTS",
            Self::JSDOC_THROWS => "JSDOC_THROWS",
            Self::JSDOC_NO_SIDE_EFFECTS => "JSDOC_NO_SIDE_EFFECTS",
            Self::JSDOC_CONSTRUCTOR => "JSDOC_CONSTRUCTOR",
            Self::JSDOC_INTERFACE => "JSDOC_INTERFACE",
            Self::JSDOC_ENUM => "JSDOC_ENUM",
            Self::JSDOC_NO_COLLAPSE => "JSDOC_NO_COLLAPSE",
            Self::JSDOC_THIS => "JSDOC_THIS",
            Self::JSDOC_CONST => "JSDOC_CONST",
            Self::JSDOC_ID_GENERATOR_CONSISTENT => "JSDOC_ID_GENERATOR_CONSISTENT",
            Self::JSDOC_ID_GENERATOR_INCONSISTENT => "JSDOC_ID_GENERATOR_INCONSISTENT",
            Self::JSDOC_ID_GENERATOR_STABLE => "JSDOC_ID_GENERATOR_STABLE",
            Self::JSDOC_ID_GENERATOR_MAPPED => "JSDOC_ID_GENERATOR_MAPPED",
            Self::JSDOC_ID_GENERATOR_XID => "JSDOC_ID_GENERATOR_XID",
            Self::JSDOC_ABSTRACT => "JSDOC_ABSTRACT",
            Self::JSDOC_SUPPRESS_MESSAGE_CONVENTION => "JSDOC_SUPPRESS_MESSAGE_CONVENTION",
            Self::JSDOC_SUPPRESS_PARTIAL_ALIAS => "JSDOC_SUPPRESS_PARTIAL_ALIAS",
            Self::JSDOC_PURE_OR_BREAK_MY_CODE => "JSDOC_PURE_OR_BREAK_MY_CODE",
            Self::JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE => "JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE",
            Self::JSDOC_FILEOVERVIEW => "JSDOC_FILEOVERVIEW",
            Self::JSDOC_PROVIDE_GOOG => "JSDOC_PROVIDE_GOOG",
            Self::JSDOC_TYPE_SUMMARY_FILE => "JSDOC_TYPE_SUMMARY_FILE",
            Self::JSDOC_PROVIDE_ALREADY_PROVIDED => "JSDOC_PROVIDE_ALREADY_PROVIDED",
            Self::JSDOC_SASS_GENERATED_CSS_TS => "JSDOC_SASS_GENERATED_CSS_TS",
            Self::JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES => {
                "JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES"
            }
            Self::JSDOC_NO_COVERAGE => "JSDOC_NO_COVERAGE",
            Self::JSDOC_REQUIRE_INLINING => "JSDOC_REQUIRE_INLINING",
            Self::JSDOC_USED_VIA_DOT_CONSTRUCTOR => "JSDOC_USED_VIA_DOT_CONSTRUCTOR",
            Self::JSDOC_ENCOURAGE_INLINING => "JSDOC_ENCOURAGE_INLINING",
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: JsdocTag#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "JSDOC_UNSPECIFIED" => Some(Self::JSDOC_UNSPECIFIED),
            "JSDOC_NO_INLINE" => Some(Self::JSDOC_NO_INLINE),
            "JSDOC_DEFINE" => Some(Self::JSDOC_DEFINE),
            "JSDOC_MODIFIES_THIS" => Some(Self::JSDOC_MODIFIES_THIS),
            "JSDOC_MODIFIES_ARGUMENTS" => Some(Self::JSDOC_MODIFIES_ARGUMENTS),
            "JSDOC_THROWS" => Some(Self::JSDOC_THROWS),
            "JSDOC_NO_SIDE_EFFECTS" => Some(Self::JSDOC_NO_SIDE_EFFECTS),
            "JSDOC_CONSTRUCTOR" => Some(Self::JSDOC_CONSTRUCTOR),
            "JSDOC_INTERFACE" => Some(Self::JSDOC_INTERFACE),
            "JSDOC_ENUM" => Some(Self::JSDOC_ENUM),
            "JSDOC_NO_COLLAPSE" => Some(Self::JSDOC_NO_COLLAPSE),
            "JSDOC_THIS" => Some(Self::JSDOC_THIS),
            "JSDOC_CONST" => Some(Self::JSDOC_CONST),
            "JSDOC_ID_GENERATOR_CONSISTENT" => Some(Self::JSDOC_ID_GENERATOR_CONSISTENT),
            "JSDOC_ID_GENERATOR_INCONSISTENT" => Some(Self::JSDOC_ID_GENERATOR_INCONSISTENT),
            "JSDOC_ID_GENERATOR_STABLE" => Some(Self::JSDOC_ID_GENERATOR_STABLE),
            "JSDOC_ID_GENERATOR_MAPPED" => Some(Self::JSDOC_ID_GENERATOR_MAPPED),
            "JSDOC_ID_GENERATOR_XID" => Some(Self::JSDOC_ID_GENERATOR_XID),
            "JSDOC_ABSTRACT" => Some(Self::JSDOC_ABSTRACT),
            "JSDOC_SUPPRESS_MESSAGE_CONVENTION" => Some(Self::JSDOC_SUPPRESS_MESSAGE_CONVENTION),
            "JSDOC_SUPPRESS_PARTIAL_ALIAS" => Some(Self::JSDOC_SUPPRESS_PARTIAL_ALIAS),
            "JSDOC_PURE_OR_BREAK_MY_CODE" => Some(Self::JSDOC_PURE_OR_BREAK_MY_CODE),
            "JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE" => Some(Self::JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE),
            "JSDOC_FILEOVERVIEW" => Some(Self::JSDOC_FILEOVERVIEW),
            "JSDOC_PROVIDE_GOOG" => Some(Self::JSDOC_PROVIDE_GOOG),
            "JSDOC_TYPE_SUMMARY_FILE" => Some(Self::JSDOC_TYPE_SUMMARY_FILE),
            "JSDOC_PROVIDE_ALREADY_PROVIDED" => Some(Self::JSDOC_PROVIDE_ALREADY_PROVIDED),
            "JSDOC_SASS_GENERATED_CSS_TS" => Some(Self::JSDOC_SASS_GENERATED_CSS_TS),
            "JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES" => {
                Some(Self::JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES)
            }
            "JSDOC_NO_COVERAGE" => Some(Self::JSDOC_NO_COVERAGE),
            "JSDOC_REQUIRE_INLINING" => Some(Self::JSDOC_REQUIRE_INLINING),
            "JSDOC_USED_VIA_DOT_CONSTRUCTOR" => Some(Self::JSDOC_USED_VIA_DOT_CONSTRUCTOR),
            "JSDOC_ENCOURAGE_INLINING" => Some(Self::JSDOC_ENCOURAGE_INLINING),
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: JsdocTag#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::JSDOC_UNSPECIFIED),
            1 => Some(Self::JSDOC_NO_INLINE),
            2 => Some(Self::JSDOC_DEFINE),
            3 => Some(Self::JSDOC_MODIFIES_THIS),
            4 => Some(Self::JSDOC_MODIFIES_ARGUMENTS),
            6 => Some(Self::JSDOC_THROWS),
            7 => Some(Self::JSDOC_NO_SIDE_EFFECTS),
            8 => Some(Self::JSDOC_CONSTRUCTOR),
            9 => Some(Self::JSDOC_INTERFACE),
            10 => Some(Self::JSDOC_ENUM),
            11 => Some(Self::JSDOC_NO_COLLAPSE),
            12 => Some(Self::JSDOC_THIS),
            13 => Some(Self::JSDOC_CONST),
            14 => Some(Self::JSDOC_ID_GENERATOR_CONSISTENT),
            15 => Some(Self::JSDOC_ID_GENERATOR_INCONSISTENT),
            16 => Some(Self::JSDOC_ID_GENERATOR_STABLE),
            17 => Some(Self::JSDOC_ID_GENERATOR_MAPPED),
            18 => Some(Self::JSDOC_ID_GENERATOR_XID),
            19 => Some(Self::JSDOC_ABSTRACT),
            22 => Some(Self::JSDOC_SUPPRESS_MESSAGE_CONVENTION),
            23 => Some(Self::JSDOC_SUPPRESS_PARTIAL_ALIAS),
            24 => Some(Self::JSDOC_PURE_OR_BREAK_MY_CODE),
            25 => Some(Self::JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE),
            26 => Some(Self::JSDOC_FILEOVERVIEW),
            31 => Some(Self::JSDOC_PROVIDE_GOOG),
            32 => Some(Self::JSDOC_TYPE_SUMMARY_FILE),
            33 => Some(Self::JSDOC_PROVIDE_ALREADY_PROVIDED),
            34 => Some(Self::JSDOC_SASS_GENERATED_CSS_TS),
            36 => Some(Self::JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES),
            37 => Some(Self::JSDOC_NO_COVERAGE),
            38 => Some(Self::JSDOC_REQUIRE_INLINING),
            39 => Some(Self::JSDOC_USED_VIA_DOT_CONSTRUCTOR),
            40 => Some(Self::JSDOC_ENCOURAGE_INLINING),
            _ => None,
        }
    }
}
impl ProtoEnum for JsdocTag {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for JsdocTag {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: OptimizationJsdoc (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct OptimizationJsdoc {
    pub kind: Vec<JsdocTag>,
    pub license_text_pointer: i32,
    pub description_pointer: i32,
    pub meaning_pointer: i32,
    pub alternate_message_id_pointer: i32,
    pub closure_unaware_mode_pointer: i32,
}
static OPTIMIZATIONJSDOC_DEFAULT_INSTANCE: LazyLock<OptimizationJsdoc> =
    LazyLock::new(OptimizationJsdoc::default);
impl OptimizationJsdoc {
    pub const KIND_FIELD_NUMBER: i32 = 1;
    pub const LICENSE_TEXT_POINTER_FIELD_NUMBER: i32 = 2;
    pub const DESCRIPTION_POINTER_FIELD_NUMBER: i32 = 3;
    pub const MEANING_POINTER_FIELD_NUMBER: i32 = 4;
    pub const ALTERNATE_MESSAGE_ID_POINTER_FIELD_NUMBER: i32 = 5;
    pub const CLOSURE_UNAWARE_MODE_POINTER_FIELD_NUMBER: i32 = 6;
    // port: OptimizationJsdoc#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: OptimizationJsdoc#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &OPTIMIZATIONJSDOC_DEFAULT_INSTANCE
    }
    // port: OptimizationJsdoc#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: OptimizationJsdoc.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: OptimizationJsdoc.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: OptimizationJsdoc#getKindList
    pub fn get_kind_list(&self) -> &[JsdocTag] {
        &self.kind
    }
    // port: OptimizationJsdoc#getKindCount
    pub fn get_kind_count(&self) -> i32 {
        self.kind.len() as i32
    }
    // port: OptimizationJsdoc#getKind(int)
    pub fn get_kind(&self, index: i32) -> JsdocTag {
        self.kind[index as usize]
    }
    // port: OptimizationJsdoc.Builder#addKind
    pub fn add_kind(mut self, value: JsdocTag) -> Self {
        self.kind.push(value);
        self
    }
    // port: OptimizationJsdoc.Builder#addAllKind
    pub fn add_all_kind(mut self, values: impl IntoIterator<Item = JsdocTag>) -> Self {
        self.kind.extend(values);
        self
    }
    // port: OptimizationJsdoc.Builder#clearKind
    pub fn clear_kind(mut self) -> Self {
        self.kind.clear();
        self
    }
    // port: OptimizationJsdoc#getLicenseTextPointer
    pub fn get_license_text_pointer(&self) -> i32 {
        self.license_text_pointer
    }
    // port: OptimizationJsdoc.Builder#setLicenseTextPointer
    pub fn set_license_text_pointer(mut self, value: i32) -> Self {
        self.license_text_pointer = value;
        self
    }
    // port: OptimizationJsdoc#getDescriptionPointer
    pub fn get_description_pointer(&self) -> i32 {
        self.description_pointer
    }
    // port: OptimizationJsdoc.Builder#setDescriptionPointer
    pub fn set_description_pointer(mut self, value: i32) -> Self {
        self.description_pointer = value;
        self
    }
    // port: OptimizationJsdoc#getMeaningPointer
    pub fn get_meaning_pointer(&self) -> i32 {
        self.meaning_pointer
    }
    // port: OptimizationJsdoc.Builder#setMeaningPointer
    pub fn set_meaning_pointer(mut self, value: i32) -> Self {
        self.meaning_pointer = value;
        self
    }
    // port: OptimizationJsdoc#getAlternateMessageIdPointer
    pub fn get_alternate_message_id_pointer(&self) -> i32 {
        self.alternate_message_id_pointer
    }
    // port: OptimizationJsdoc.Builder#setAlternateMessageIdPointer
    pub fn set_alternate_message_id_pointer(mut self, value: i32) -> Self {
        self.alternate_message_id_pointer = value;
        self
    }
    // port: OptimizationJsdoc#getClosureUnawareModePointer
    pub fn get_closure_unaware_mode_pointer(&self) -> i32 {
        self.closure_unaware_mode_pointer
    }
    // port: OptimizationJsdoc.Builder#setClosureUnawareModePointer
    pub fn set_closure_unaware_mode_pointer(mut self, value: i32) -> Self {
        self.closure_unaware_mode_pointer = value;
        self
    }
}
impl Message for OptimizationJsdoc {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 | 10 => {
                    input.read_repeated(tag, &mut self.kind, |input| {
                        Ok(JsdocTag::for_number_or_unrecognized(input.read_enum()?))
                    })?;
                }
                16 => {
                    self.license_text_pointer = input.read_int32()?;
                }
                24 => {
                    self.description_pointer = input.read_int32()?;
                }
                32 => {
                    self.meaning_pointer = input.read_int32()?;
                }
                40 => {
                    self.alternate_message_id_pointer = input.read_int32()?;
                }
                48 => {
                    self.closure_unaware_mode_pointer = input.read_int32()?;
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
        if !self.kind.is_empty() {
            let data_size: usize = self
                .kind
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(v.get_number()))
                .sum();
            output.write_tag(1, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.kind {
                output.write_int32_no_tag(v.get_number());
            }
        }
        if self.license_text_pointer != 0 {
            let v = &self.license_text_pointer;
            output.write_tag(2, 0);
            output.write_int32_no_tag(*v);
        }
        if self.description_pointer != 0 {
            let v = &self.description_pointer;
            output.write_tag(3, 0);
            output.write_int32_no_tag(*v);
        }
        if self.meaning_pointer != 0 {
            let v = &self.meaning_pointer;
            output.write_tag(4, 0);
            output.write_int32_no_tag(*v);
        }
        if self.alternate_message_id_pointer != 0 {
            let v = &self.alternate_message_id_pointer;
            output.write_tag(5, 0);
            output.write_int32_no_tag(*v);
        }
        if self.closure_unaware_mode_pointer != 0 {
            let v = &self.closure_unaware_mode_pointer;
            output.write_tag(6, 0);
            output.write_int32_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.kind.is_empty() {
            let data_size: usize = self
                .kind
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(v.get_number()))
                .sum();
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        if self.license_text_pointer != 0 {
            let v = &self.license_text_pointer;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.description_pointer != 0 {
            let v = &self.description_pointer;
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.meaning_pointer != 0 {
            let v = &self.meaning_pointer;
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.alternate_message_id_pointer != 0 {
            let v = &self.alternate_message_id_pointer;
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.closure_unaware_mode_pointer != 0 {
            let v = &self.closure_unaware_mode_pointer;
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        size
    }
}
// port: Descriptors.FileDescriptor of optimization_jsdoc.proto (the message descriptors protoc embeds)
pub static MESSAGE_DESCRIPTORS: &[Descriptor] = &[Descriptor {
    full_name: "jscomp.OptimizationJsdoc",
    fields: &[
        FieldDescriptor {
            name: "kind",
            number: 1,
            field_type: FieldType::ENUM,
            repeated: true,
            containing_oneof: None,
            type_name: "jscomp.JsdocTag",
        },
        FieldDescriptor {
            name: "license_text_pointer",
            number: 2,
            field_type: FieldType::INT32,
            repeated: false,
            containing_oneof: None,
            type_name: "",
        },
        FieldDescriptor {
            name: "description_pointer",
            number: 3,
            field_type: FieldType::INT32,
            repeated: false,
            containing_oneof: None,
            type_name: "",
        },
        FieldDescriptor {
            name: "meaning_pointer",
            number: 4,
            field_type: FieldType::INT32,
            repeated: false,
            containing_oneof: None,
            type_name: "",
        },
        FieldDescriptor {
            name: "alternate_message_id_pointer",
            number: 5,
            field_type: FieldType::INT32,
            repeated: false,
            containing_oneof: None,
            type_name: "",
        },
        FieldDescriptor {
            name: "closure_unaware_mode_pointer",
            number: 6,
            field_type: FieldType::INT32,
            repeated: false,
            containing_oneof: None,
            type_name: "",
        },
    ],
}];
// port: Descriptors.FileDescriptor of optimization_jsdoc.proto (the enum descriptors protoc embeds)
pub static ENUM_DESCRIPTORS: &[EnumDescriptor] = &[EnumDescriptor {
    full_name: "jscomp.JsdocTag",
    name: "JsdocTag",
    values: &[
        ("JSDOC_UNSPECIFIED", 0),
        ("JSDOC_NO_INLINE", 1),
        ("JSDOC_DEFINE", 2),
        ("JSDOC_MODIFIES_THIS", 3),
        ("JSDOC_MODIFIES_ARGUMENTS", 4),
        ("JSDOC_THROWS", 6),
        ("JSDOC_NO_SIDE_EFFECTS", 7),
        ("JSDOC_CONSTRUCTOR", 8),
        ("JSDOC_INTERFACE", 9),
        ("JSDOC_ENUM", 10),
        ("JSDOC_NO_COLLAPSE", 11),
        ("JSDOC_THIS", 12),
        ("JSDOC_CONST", 13),
        ("JSDOC_ID_GENERATOR_CONSISTENT", 14),
        ("JSDOC_ID_GENERATOR_INCONSISTENT", 15),
        ("JSDOC_ID_GENERATOR_STABLE", 16),
        ("JSDOC_ID_GENERATOR_MAPPED", 17),
        ("JSDOC_ID_GENERATOR_XID", 18),
        ("JSDOC_ABSTRACT", 19),
        ("JSDOC_SUPPRESS_MESSAGE_CONVENTION", 22),
        ("JSDOC_SUPPRESS_PARTIAL_ALIAS", 23),
        ("JSDOC_PURE_OR_BREAK_MY_CODE", 24),
        ("JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE", 25),
        ("JSDOC_FILEOVERVIEW", 26),
        ("JSDOC_PROVIDE_GOOG", 31),
        ("JSDOC_TYPE_SUMMARY_FILE", 32),
        ("JSDOC_PROVIDE_ALREADY_PROVIDED", 33),
        ("JSDOC_SASS_GENERATED_CSS_TS", 34),
        ("JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES", 36),
        ("JSDOC_NO_COVERAGE", 37),
        ("JSDOC_REQUIRE_INLINING", 38),
        ("JSDOC_USED_VIA_DOT_CONSTRUCTOR", 39),
        ("JSDOC_ENCOURAGE_INLINING", 40),
    ],
}];
