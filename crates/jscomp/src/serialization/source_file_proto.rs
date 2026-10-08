/*
 * Copyright The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/GlobalNamespace.java,
//   src/com/google/javascript/rhino/typed_ast/source_file.proto.

// Generated from src/com/google/javascript/rhino/typed_ast/source_file.proto (protoc's Java API shape; D-003, no
// protoc on the host). Wire format and defaults follow proto3; see protobuf.rs.
#![allow(clippy::all, unused_imports, unused_variables, dead_code)]
use super::protobuf::{
    CodedInputStream, CodedOutputStream, Descriptor, EnumDescriptor, FieldDescriptor, FieldType,
    Message, ProtoEnum, ProtoResult, WireFormat,
};
use closure_rhino::js_string::JsString;
use std::fmt;
use std::sync::LazyLock;
// port: SourceFileProto.SourceKind (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum SourceKind {
    #[default]
    NOT_SPECIFIED = 0,
    EXTERN = 1,
    CODE = 2,
    UNRECOGNIZED = -1,
}
impl SourceKind {
    pub const VALUES: [Self; 4] = [
        Self::NOT_SPECIFIED,
        Self::EXTERN,
        Self::CODE,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: SourceKind#getNumber
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
            Self::NOT_SPECIFIED => "NOT_SPECIFIED",
            Self::EXTERN => "EXTERN",
            Self::CODE => "CODE",
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: SourceKind#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "NOT_SPECIFIED" => Some(Self::NOT_SPECIFIED),
            "EXTERN" => Some(Self::EXTERN),
            "CODE" => Some(Self::CODE),
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: SourceKind#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::NOT_SPECIFIED),
            1 => Some(Self::EXTERN),
            2 => Some(Self::CODE),
            _ => None,
        }
    }
}
impl ProtoEnum for SourceKind {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for SourceKind {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: SourceFilePool (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SourceFilePool {
    pub source_file: Vec<SourceFileProto>,
}
static SOURCEFILEPOOL_DEFAULT_INSTANCE: LazyLock<SourceFilePool> =
    LazyLock::new(SourceFilePool::default);
impl SourceFilePool {
    pub const SOURCE_FILE_FIELD_NUMBER: i32 = 1;
    // port: SourceFilePool#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: SourceFilePool#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &SOURCEFILEPOOL_DEFAULT_INSTANCE
    }
    // port: SourceFilePool#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: SourceFilePool.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: SourceFilePool.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: SourceFilePool#getSourceFileList
    pub fn get_source_file_list(&self) -> &[SourceFileProto] {
        &self.source_file
    }
    // port: SourceFilePool#getSourceFileCount
    pub fn get_source_file_count(&self) -> i32 {
        self.source_file.len() as i32
    }
    // port: SourceFilePool#getSourceFile(int)
    pub fn get_source_file(&self, index: i32) -> &SourceFileProto {
        &self.source_file[index as usize]
    }
    // port: SourceFilePool.Builder#addSourceFile
    pub fn add_source_file(mut self, value: SourceFileProto) -> Self {
        self.source_file.push(value);
        self
    }
    // port: SourceFilePool.Builder#addAllSourceFile
    pub fn add_all_source_file(
        mut self,
        values: impl IntoIterator<Item = SourceFileProto>,
    ) -> Self {
        self.source_file.extend(values);
        self
    }
    // port: SourceFilePool.Builder#clearSourceFile
    pub fn clear_source_file(mut self) -> Self {
        self.source_file.clear();
        self
    }
}
impl Message for SourceFilePool {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = SourceFileProto::default();
                    input.read_message(&mut v)?;
                    self.source_file.push(v);
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
        for v in &self.source_file {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.source_file {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: SourceFileProto#loader (oneof)
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Loader {
    PRELOADED_CONTENTS(JsString),
    FILE_ON_DISK(FileOnDisk),
    ZIP_ENTRY(ZipEntryOnDisk),
    STUB_FILE(bool),
    #[default]
    LOADER_NOT_SET,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoaderCase {
    PRELOADED_CONTENTS,
    FILE_ON_DISK,
    ZIP_ENTRY,
    STUB_FILE,
    LOADER_NOT_SET,
}
// port: SourceFileProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SourceFileProto {
    pub filename: String,
    pub source_kind: SourceKind,
    pub is_closure_unaware_code: bool,
    pub num_bytes_plus_one: i32,
    pub num_lines_plus_one: i32,
    pub loader: Loader,
}
static SOURCEFILEPROTO_DEFAULT_INSTANCE: LazyLock<SourceFileProto> =
    LazyLock::new(SourceFileProto::default);
impl SourceFileProto {
    pub const FILENAME_FIELD_NUMBER: i32 = 1;
    pub const PRELOADED_CONTENTS_FIELD_NUMBER: i32 = 2;
    pub const FILE_ON_DISK_FIELD_NUMBER: i32 = 3;
    pub const ZIP_ENTRY_FIELD_NUMBER: i32 = 4;
    pub const STUB_FILE_FIELD_NUMBER: i32 = 9;
    pub const SOURCE_KIND_FIELD_NUMBER: i32 = 5;
    pub const IS_CLOSURE_UNAWARE_CODE_FIELD_NUMBER: i32 = 8;
    pub const NUM_BYTES_PLUS_ONE_FIELD_NUMBER: i32 = 6;
    pub const NUM_LINES_PLUS_ONE_FIELD_NUMBER: i32 = 7;
    // port: SourceFileProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: SourceFileProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &SOURCEFILEPROTO_DEFAULT_INSTANCE
    }
    // port: SourceFileProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: SourceFileProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: SourceFileProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: SourceFileProto#getFilename
    pub fn get_filename(&self) -> &str {
        &self.filename
    }
    // port: SourceFileProto.Builder#setFilename
    pub fn set_filename(mut self, value: impl Into<String>) -> Self {
        self.filename = value.into();
        self
    }
    // port: SourceFileProto#hasPreloadedContents
    pub fn has_preloaded_contents(&self) -> bool {
        matches!(self.loader, Loader::PRELOADED_CONTENTS(_))
    }
    // port: SourceFileProto#getPreloadedContents
    pub fn get_preloaded_contents(&self) -> JsString {
        if let Loader::PRELOADED_CONTENTS(v) = &self.loader {
            v.clone()
        } else {
            JsString::default()
        }
    }
    // port: SourceFileProto.Builder#setPreloadedContents
    pub fn set_preloaded_contents(mut self, value: impl Into<JsString>) -> Self {
        self.loader = Loader::PRELOADED_CONTENTS(value.into());
        self
    }
    // port: SourceFileProto#hasFileOnDisk
    pub fn has_file_on_disk(&self) -> bool {
        matches!(self.loader, Loader::FILE_ON_DISK(_))
    }
    // port: SourceFileProto#getFileOnDisk
    pub fn get_file_on_disk(&self) -> &FileOnDisk {
        if let Loader::FILE_ON_DISK(v) = &self.loader {
            v
        } else {
            FileOnDisk::default_instance_ref()
        }
    }
    // port: SourceFileProto.Builder#setFileOnDisk
    pub fn set_file_on_disk(mut self, value: FileOnDisk) -> Self {
        self.loader = Loader::FILE_ON_DISK(value);
        self
    }
    // port: SourceFileProto#hasZipEntry
    pub fn has_zip_entry(&self) -> bool {
        matches!(self.loader, Loader::ZIP_ENTRY(_))
    }
    // port: SourceFileProto#getZipEntry
    pub fn get_zip_entry(&self) -> &ZipEntryOnDisk {
        if let Loader::ZIP_ENTRY(v) = &self.loader {
            v
        } else {
            ZipEntryOnDisk::default_instance_ref()
        }
    }
    // port: SourceFileProto.Builder#setZipEntry
    pub fn set_zip_entry(mut self, value: ZipEntryOnDisk) -> Self {
        self.loader = Loader::ZIP_ENTRY(value);
        self
    }
    // port: SourceFileProto#hasStubFile
    pub fn has_stub_file(&self) -> bool {
        matches!(self.loader, Loader::STUB_FILE(_))
    }
    // port: SourceFileProto#getStubFile
    pub fn get_stub_file(&self) -> bool {
        if let Loader::STUB_FILE(v) = &self.loader {
            *v
        } else {
            bool::default()
        }
    }
    // port: SourceFileProto.Builder#setStubFile
    pub fn set_stub_file(mut self, value: bool) -> Self {
        self.loader = Loader::STUB_FILE(value);
        self
    }
    // port: SourceFileProto#getSourceKind
    pub fn get_source_kind(&self) -> SourceKind {
        self.source_kind
    }
    // port: SourceFileProto.Builder#setSourceKind
    pub fn set_source_kind(mut self, value: SourceKind) -> Self {
        self.source_kind = value;
        self
    }
    // port: SourceFileProto#getSourceKindValue
    pub fn get_source_kind_value(&self) -> i32 {
        self.source_kind.get_number()
    }
    // port: SourceFileProto#getIsClosureUnawareCode
    pub fn get_is_closure_unaware_code(&self) -> bool {
        self.is_closure_unaware_code
    }
    // port: SourceFileProto.Builder#setIsClosureUnawareCode
    pub fn set_is_closure_unaware_code(mut self, value: bool) -> Self {
        self.is_closure_unaware_code = value;
        self
    }
    // port: SourceFileProto#getNumBytesPlusOne
    pub fn get_num_bytes_plus_one(&self) -> i32 {
        self.num_bytes_plus_one
    }
    // port: SourceFileProto.Builder#setNumBytesPlusOne
    pub fn set_num_bytes_plus_one(mut self, value: i32) -> Self {
        self.num_bytes_plus_one = value;
        self
    }
    // port: SourceFileProto#getNumLinesPlusOne
    pub fn get_num_lines_plus_one(&self) -> i32 {
        self.num_lines_plus_one
    }
    // port: SourceFileProto.Builder#setNumLinesPlusOne
    pub fn set_num_lines_plus_one(mut self, value: i32) -> Self {
        self.num_lines_plus_one = value;
        self
    }
    // port: SourceFileProto#getLoaderCase
    pub fn get_loader_case(&self) -> LoaderCase {
        match self.loader {
            Loader::PRELOADED_CONTENTS(_) => LoaderCase::PRELOADED_CONTENTS,
            Loader::FILE_ON_DISK(_) => LoaderCase::FILE_ON_DISK,
            Loader::ZIP_ENTRY(_) => LoaderCase::ZIP_ENTRY,
            Loader::STUB_FILE(_) => LoaderCase::STUB_FILE,
            Loader::LOADER_NOT_SET => LoaderCase::LOADER_NOT_SET,
        }
    }
}
impl Message for SourceFileProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.filename = input.read_string_require_utf8()?;
                }
                18 => {
                    self.loader = Loader::PRELOADED_CONTENTS(JsString::from(
                        input.read_string_require_utf8()?,
                    ));
                }
                26 => {
                    let mut v = if let Loader::FILE_ON_DISK(v) = std::mem::take(&mut self.loader) {
                        v
                    } else {
                        FileOnDisk::default()
                    };
                    input.read_message(&mut v)?;
                    self.loader = Loader::FILE_ON_DISK(v);
                }
                34 => {
                    let mut v = if let Loader::ZIP_ENTRY(v) = std::mem::take(&mut self.loader) {
                        v
                    } else {
                        ZipEntryOnDisk::default()
                    };
                    input.read_message(&mut v)?;
                    self.loader = Loader::ZIP_ENTRY(v);
                }
                72 => {
                    self.loader = Loader::STUB_FILE(input.read_bool()?);
                }
                40 => {
                    self.source_kind = SourceKind::for_number_or_unrecognized(input.read_enum()?);
                }
                64 => {
                    self.is_closure_unaware_code = input.read_bool()?;
                }
                48 => {
                    self.num_bytes_plus_one = input.read_uint32()?;
                }
                56 => {
                    self.num_lines_plus_one = input.read_uint32()?;
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
        if !self.filename.is_empty() {
            let v = &self.filename;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if let Loader::PRELOADED_CONTENTS(v) = &self.loader {
            output.write_tag(2, 2);
            output.write_byte_array_no_tag(&super::protobuf::encode_utf8_java(v));
        }
        if let Loader::FILE_ON_DISK(v) = &self.loader {
            output.write_tag(3, 2);
            output.write_message_no_tag(v);
        }
        if let Loader::ZIP_ENTRY(v) = &self.loader {
            output.write_tag(4, 2);
            output.write_message_no_tag(v);
        }
        if self.source_kind.get_number() != 0 {
            let v = &self.source_kind;
            output.write_tag(5, 0);
            output.write_int32_no_tag(v.get_number());
        }
        if self.num_bytes_plus_one != 0 {
            let v = &self.num_bytes_plus_one;
            output.write_tag(6, 0);
            output.write_uint32_no_tag(*v);
        }
        if self.num_lines_plus_one != 0 {
            let v = &self.num_lines_plus_one;
            output.write_tag(7, 0);
            output.write_uint32_no_tag(*v);
        }
        if self.is_closure_unaware_code {
            let v = &self.is_closure_unaware_code;
            output.write_tag(8, 0);
            output.write_bool_no_tag(*v);
        }
        if let Loader::STUB_FILE(v) = &self.loader {
            output.write_tag(9, 0);
            output.write_bool_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.filename.is_empty() {
            let v = &self.filename;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if let Loader::PRELOADED_CONTENTS(v) = &self.loader {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_byte_array_size_no_tag(
                    &super::protobuf::encode_utf8_java(v),
                );
        }
        if let Loader::FILE_ON_DISK(v) = &self.loader {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Loader::ZIP_ENTRY(v) = &self.loader {
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if self.source_kind.get_number() != 0 {
            let v = &self.source_kind;
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_int32_size_no_tag(v.get_number());
        }
        if self.num_bytes_plus_one != 0 {
            let v = &self.num_bytes_plus_one;
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if self.num_lines_plus_one != 0 {
            let v = &self.num_lines_plus_one;
            size += CodedOutputStream::compute_tag_size(7)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if self.is_closure_unaware_code {
            let v = &self.is_closure_unaware_code;
            size += CodedOutputStream::compute_tag_size(8) + 1;
        }
        if let Loader::STUB_FILE(v) = &self.loader {
            size += CodedOutputStream::compute_tag_size(9) + 1;
        }
        size
    }
}
// port: SourceFileProto.FileOnDisk (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FileOnDisk {
    pub actual_path: String,
    pub charset: String,
}
static FILEONDISK_DEFAULT_INSTANCE: LazyLock<FileOnDisk> = LazyLock::new(FileOnDisk::default);
impl FileOnDisk {
    pub const ACTUAL_PATH_FIELD_NUMBER: i32 = 1;
    pub const CHARSET_FIELD_NUMBER: i32 = 2;
    // port: FileOnDisk#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: FileOnDisk#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &FILEONDISK_DEFAULT_INSTANCE
    }
    // port: FileOnDisk#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: FileOnDisk.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: FileOnDisk.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: FileOnDisk#getActualPath
    pub fn get_actual_path(&self) -> &str {
        &self.actual_path
    }
    // port: FileOnDisk.Builder#setActualPath
    pub fn set_actual_path(mut self, value: impl Into<String>) -> Self {
        self.actual_path = value.into();
        self
    }
    // port: FileOnDisk#getCharset
    pub fn get_charset(&self) -> &str {
        &self.charset
    }
    // port: FileOnDisk.Builder#setCharset
    pub fn set_charset(mut self, value: impl Into<String>) -> Self {
        self.charset = value.into();
        self
    }
}
impl Message for FileOnDisk {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.actual_path = input.read_string_require_utf8()?;
                }
                18 => {
                    self.charset = input.read_string_require_utf8()?;
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
        if !self.actual_path.is_empty() {
            let v = &self.actual_path;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if !self.charset.is_empty() {
            let v = &self.charset;
            output.write_tag(2, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.actual_path.is_empty() {
            let v = &self.actual_path;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if !self.charset.is_empty() {
            let v = &self.charset;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: SourceFileProto.ZipEntryOnDisk (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ZipEntryOnDisk {
    pub zip_path: String,
    pub entry_name: String,
    pub charset: String,
}
static ZIPENTRYONDISK_DEFAULT_INSTANCE: LazyLock<ZipEntryOnDisk> =
    LazyLock::new(ZipEntryOnDisk::default);
impl ZipEntryOnDisk {
    pub const ZIP_PATH_FIELD_NUMBER: i32 = 1;
    pub const ENTRY_NAME_FIELD_NUMBER: i32 = 2;
    pub const CHARSET_FIELD_NUMBER: i32 = 3;
    // port: ZipEntryOnDisk#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ZipEntryOnDisk#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &ZIPENTRYONDISK_DEFAULT_INSTANCE
    }
    // port: ZipEntryOnDisk#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ZipEntryOnDisk.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ZipEntryOnDisk.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ZipEntryOnDisk#getZipPath
    pub fn get_zip_path(&self) -> &str {
        &self.zip_path
    }
    // port: ZipEntryOnDisk.Builder#setZipPath
    pub fn set_zip_path(mut self, value: impl Into<String>) -> Self {
        self.zip_path = value.into();
        self
    }
    // port: ZipEntryOnDisk#getEntryName
    pub fn get_entry_name(&self) -> &str {
        &self.entry_name
    }
    // port: ZipEntryOnDisk.Builder#setEntryName
    pub fn set_entry_name(mut self, value: impl Into<String>) -> Self {
        self.entry_name = value.into();
        self
    }
    // port: ZipEntryOnDisk#getCharset
    pub fn get_charset(&self) -> &str {
        &self.charset
    }
    // port: ZipEntryOnDisk.Builder#setCharset
    pub fn set_charset(mut self, value: impl Into<String>) -> Self {
        self.charset = value.into();
        self
    }
}
impl Message for ZipEntryOnDisk {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.zip_path = input.read_string_require_utf8()?;
                }
                18 => {
                    self.entry_name = input.read_string_require_utf8()?;
                }
                26 => {
                    self.charset = input.read_string_require_utf8()?;
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
        if !self.zip_path.is_empty() {
            let v = &self.zip_path;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if !self.entry_name.is_empty() {
            let v = &self.entry_name;
            output.write_tag(2, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if !self.charset.is_empty() {
            let v = &self.charset;
            output.write_tag(3, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.zip_path.is_empty() {
            let v = &self.zip_path;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if !self.entry_name.is_empty() {
            let v = &self.entry_name;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if !self.charset.is_empty() {
            let v = &self.charset;
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: Descriptors.FileDescriptor of source_file.proto (the message descriptors protoc embeds)
pub static MESSAGE_DESCRIPTORS: &[Descriptor] = &[
    Descriptor {
        full_name: "jscomp.SourceFilePool",
        fields: &[FieldDescriptor {
            name: "source_file",
            number: 1,
            field_type: FieldType::MESSAGE,
            repeated: true,
            containing_oneof: None,
            type_name: "jscomp.SourceFileProto",
        }],
    },
    Descriptor {
        full_name: "jscomp.SourceFileProto",
        fields: &[
            FieldDescriptor {
                name: "filename",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "preloaded_contents",
                number: 2,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: Some("loader"),
                type_name: "",
            },
            FieldDescriptor {
                name: "file_on_disk",
                number: 3,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: Some("loader"),
                type_name: "jscomp.SourceFileProto.FileOnDisk",
            },
            FieldDescriptor {
                name: "zip_entry",
                number: 4,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: Some("loader"),
                type_name: "jscomp.SourceFileProto.ZipEntryOnDisk",
            },
            FieldDescriptor {
                name: "stub_file",
                number: 9,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: Some("loader"),
                type_name: "",
            },
            FieldDescriptor {
                name: "source_kind",
                number: 5,
                field_type: FieldType::ENUM,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.SourceFileProto.SourceKind",
            },
            FieldDescriptor {
                name: "is_closure_unaware_code",
                number: 8,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "num_bytes_plus_one",
                number: 6,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "num_lines_plus_one",
                number: 7,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.SourceFileProto.FileOnDisk",
        fields: &[
            FieldDescriptor {
                name: "actual_path",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "charset",
                number: 2,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.SourceFileProto.ZipEntryOnDisk",
        fields: &[
            FieldDescriptor {
                name: "zip_path",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "entry_name",
                number: 2,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "charset",
                number: 3,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
];
// port: Descriptors.FileDescriptor of source_file.proto (the enum descriptors protoc embeds)
pub static ENUM_DESCRIPTORS: &[EnumDescriptor] = &[EnumDescriptor {
    full_name: "jscomp.SourceFileProto.SourceKind",
    name: "SourceKind",
    values: &[("NOT_SPECIFIED", 0), ("EXTERN", 1), ("CODE", 2)],
}];
