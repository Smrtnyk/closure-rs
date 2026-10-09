/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SourceFile.java.

// Keep Java nested conditionals and nested loader class names.
#![allow(clippy::collapsible_if, clippy::enum_variant_names)]
use crate::{
    js_comp_zip_file_cache::JSCompZipFileCache,
    serialization::source_file_proto::{FileOnDisk, Loader, SourceFileProto, ZipEntryOnDisk},
    simple_region::SimpleRegion,
};
use closure_rhino::{
    check_argument, check_state,
    java_lang::{
        charset::{Charset, utf8_encoded_text},
        io_exception::IOException,
    },
    js_string::JsString,
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::{
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[derive(Debug)]
struct State {
    code: Option<JsString>,
    line_offsets: Option<Arc<[i32]>>,
    num_lines: i32,
    num_bytes: i32,
    kind: SourceKind,
    is_closure_unaware_code: bool,
    is_stub_source_file_for_already_provided_input: bool,
}
#[derive(Debug)]
pub struct SourceFile {
    file_name: String,
    loader: CodeLoader,
    state: Mutex<State>,
    load_lock: Mutex<()>,
}
impl SourceFile {
    // port: SourceFile#SourceFile
    fn new(loader: CodeLoader, file_name: String, kind: SourceKind) -> Self {
        check_argument!(!file_name.is_empty(), "a source must have a name");
        let file_name = if std::path::MAIN_SEPARATOR != '/' {
            file_name.replace(std::path::MAIN_SEPARATOR, "/")
        } else {
            file_name
        };
        Self {
            file_name,
            loader,
            state: Mutex::new(State {
                code: None,
                line_offsets: None,
                num_lines: -1,
                num_bytes: -1,
                kind,
                is_closure_unaware_code: false,
                is_stub_source_file_for_already_provided_input: false,
            }),
            load_lock: Mutex::new(()),
        }
    }
    // port: SourceFile#isStubSourceFileForAlreadyProvidedInput
    pub fn is_stub_source_file_for_already_provided_input(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .is_stub_source_file_for_already_provided_input
    }
    // port: SourceFile#setIsStubSourceFileForAlreadyProvidedInput
    pub fn set_is_stub_source_file_for_already_provided_input(&self, value: bool) {
        self.state
            .lock()
            .unwrap()
            .is_stub_source_file_for_already_provided_input = value;
    }
    // port: SourceFile#getNumLines
    pub fn get_num_lines(&self) -> i32 {
        let unknown = self.state.lock().unwrap().num_lines < 0;
        if unknown && self.get_code().is_err() {
            return 0;
        }
        self.state.lock().unwrap().num_lines
    }
    // port: SourceFile#getNumBytes
    pub fn get_num_bytes(&self) -> i32 {
        let unknown = self.state.lock().unwrap().num_bytes < 0;
        if unknown && self.get_code().is_err() {
            return 0;
        }
        self.state.lock().unwrap().num_bytes
    }
    // port: SourceFile#findLineOffsets
    fn find_line_offsets(&self) -> Arc<[i32]> {
        if let Some(offsets) = &self.state.lock().unwrap().line_offsets {
            return offsets.clone();
        }
        let local_code = match self.get_code() {
            Ok(code) => code,
            Err(_) => {
                let offsets: Arc<[i32]> = Arc::new([0]);
                self.state.lock().unwrap().line_offsets = Some(offsets.clone());
                return offsets;
            }
        };
        let mut offsets = vec![0];
        for (index, &c) in local_code.as_units().iter().enumerate() {
            if c == 10 {
                offsets.push((index + 1) as i32);
            }
        }
        check_state!(offsets.len() as i32 == self.state.lock().unwrap().num_lines);
        let offsets: Arc<[i32]> = offsets.into();
        self.state.lock().unwrap().line_offsets = Some(offsets.clone());
        offsets
    }
    // port: SourceFile#getCode
    pub fn get_code(&self) -> Result<JsString, IOException> {
        if self.state.lock().unwrap().code.is_none() {
            let _lock = self
                .load_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.state.lock().unwrap().code.is_none() {
                self.set_code_and_do_bookkeeping(Some(self.loader.load_uncached_code()?));
            }
        }
        Ok(self.state.lock().unwrap().code.clone().unwrap())
    }
    /// Rust-only: the code `get_code` returns, without caching it in this SourceFile (for
    /// `parallel_parse`, which must not change the file's state).
    pub fn load_code_uncached(&self) -> Result<JsString, IOException> {
        if let Some(code) = self.state.lock().unwrap().code.clone() {
            return Ok(code);
        }
        let mut code = self.loader.load_uncached_code()?;
        // set_code_and_do_bookkeeping
        if code.as_units().first() == Some(&0xfeff) {
            code = code.substring_from(1);
        }
        Ok(code)
    }
    // port: SourceFile#setCodeDeprecated
    pub fn set_code_deprecated(&self, code: impl Into<JsString>) {
        self.set_code_and_do_bookkeeping(Some(code.into()));
    }
    // port: SourceFile#getCodeReader
    pub fn get_code_reader(&self) -> Result<JsString, IOException> {
        if self.state.lock().unwrap().code.is_none() {
            let _lock = self
                .load_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.state.lock().unwrap().code.is_none() {
                if let Some(reader) = self.loader.open_uncached_reader()? {
                    return Ok(reader);
                }
            }
        }
        self.get_code()
    }
    // port: SourceFile#setCodeAndDoBookkeeping
    fn set_code_and_do_bookkeeping(&self, source_code: Option<JsString>) {
        let mut state = self.state.lock().unwrap();
        state.code = None;
        state.line_offsets = None;
        if let Some(mut source_code) = source_code {
            if source_code.as_units().first() == Some(&0xfeff) {
                source_code = source_code.substring_from(1);
            }
            state.num_bytes = source_code.length() as i32;
            state.num_lines =
                1 + source_code.as_units().iter().filter(|&&c| c == 10).count() as i32;
            state.code = Some(source_code);
        }
    }
    // port: SourceFile#getOriginalPath
    pub fn get_original_path(&self) -> &str {
        self.get_name()
    }
    // port: SourceFile#clearCachedSource
    pub fn clear_cached_source(&self) {
        self.set_code_and_do_bookkeeping(None);
    }
    // port: SourceFile#hasSourceInMemory
    pub fn has_source_in_memory(&self) -> bool {
        self.state.lock().unwrap().code.is_some()
    }
    // port: SourceFile#setKind
    pub fn set_kind(&self, kind: SourceKind) {
        self.state.lock().unwrap().kind = kind;
    }
    // port: SourceFile#markAsClosureUnawareCode
    pub fn mark_as_closure_unaware_code(&self) {
        self.state.lock().unwrap().is_closure_unaware_code = true;
    }
    // port: SourceFile#getLine
    pub fn get_line(&self, mut line_number: i32) -> Option<String> {
        let js = self.get_code().ok()?;
        let offsets = self.find_line_offsets();
        if line_number > offsets.len() as i32 {
            return None;
        }
        if line_number < 1 {
            line_number = 1;
        }
        let pos = offsets[(line_number - 1) as usize] as usize;
        match js.as_units()[pos..].iter().position(|&c| c == 10) {
            None if pos >= js.length() => None,
            None => Some(utf8_encoded_text(js.substring_from(pos).as_units())),
            Some(end) => Some(utf8_encoded_text(js.substring(pos, pos + end).as_units())),
        }
    }
    // port: SourceFile#getLines
    pub fn get_lines(&self, mut line_number: i32, mut length: i32) -> Option<SimpleRegion> {
        let js = self.get_code().ok()?;
        let offsets = self.find_line_offsets();
        if line_number > offsets.len() as i32 {
            return None;
        }
        if line_number < 1 {
            line_number = 1;
        }
        if length <= 0 {
            length = 1;
        }
        let pos = offsets[(line_number - 1) as usize];
        if pos as usize == js.length() {
            return Some(SimpleRegion::new(line_number, line_number, ""));
        }
        let mut end_char = pos;
        let mut end_line = line_number;
        while end_char < pos.wrapping_add(length) && end_line <= offsets.len() as i32 {
            end_char = if end_line < offsets.len() as i32 {
                offsets[end_line as usize]
            } else {
                js.length() as i32
            };
            end_line += 1;
        }
        if js.char_at((end_char - 1) as usize) == 10 {
            end_char -= 1;
        }
        Some(SimpleRegion::new(
            line_number,
            end_line,
            utf8_encoded_text(js.substring(pos as usize, end_char as usize).as_units()),
        ))
    }
    // port: SourceFile#getRegion
    pub fn get_region(&self, line_number: i32) -> Option<SimpleRegion> {
        let js = self.get_code().ok()?;
        let mut pos = 0usize;
        let start_line = 1.max(line_number.wrapping_sub(3).wrapping_add(1));
        for _ in 1..start_line {
            let Some(next) = js.as_units()[pos..].iter().position(|&c| c == 10) else {
                break;
            };
            pos += next + 1;
        }
        let mut end = pos as i32;
        let mut end_line = start_line;
        for _ in 0..5 {
            end = js.as_units()[end as usize..]
                .iter()
                .position(|&c| c == 10)
                .map_or(-1, |p| end + p as i32);
            if end == -1 {
                break;
            }
            end += 1;
            end_line += 1;
        }
        if line_number >= end_line {
            return None;
        }
        let source = if end == -1 {
            let last = js.length() - 1;
            if js.char_at(last) == 10 {
                js.substring(pos, last)
            } else {
                js.substring_from(pos)
            }
        } else {
            js.substring(pos, end as usize)
        };
        Some(SimpleRegion::new(
            start_line,
            end_line,
            utf8_encoded_text(source.as_units()),
        ))
    }
    // port: SourceFile#fromZipFile
    pub fn from_zip_file(zip_name: &str, input_charset: Charset) -> Result<Vec<Self>, IOException> {
        let input = fs::File::open(zip_name).map_err(|e| {
            IOException::new(format!(
                "{zip_name} ({})",
                if e.kind() == std::io::ErrorKind::NotFound {
                    "No such file or directory".into()
                } else {
                    e.to_string()
                }
            ))
        })?;
        Self::from_zip_input(zip_name, input, input_charset)
    }
    // port: SourceFile#fromZipInput
    pub fn from_zip_input(
        zip_name: &str,
        input: impl Read,
        input_charset: Charset,
    ) -> Result<Vec<Self>, IOException> {
        let absolute_zip_path = std::path::absolute(zip_name)
            .map_err(|e| IOException::new(e.to_string()))?
            .to_string_lossy()
            .into_owned();
        let mut input =
            closure_rhino::java_lang::zip_input_stream::ZipInputStream::new(input, input_charset);
        let mut source_files = Vec::new();
        while let Some(entry_name) = input.get_next_entry()? {
            if !entry_name.ends_with(".js") {
                continue;
            }
            source_files.push(
                Self::builder()
                    .with_charset(input_charset)
                    .with_original_path(format!("{zip_name}!/{entry_name}"))
                    .with_zip_entry_path(&absolute_zip_path, &entry_name)
                    .build(),
            );
        }
        Ok(source_files)
    }
    // port: SourceFile#fromFile(String)
    pub fn from_file(file_name: &str) -> Self {
        Self::builder().with_path(file_name).build()
    }
    // port: SourceFile#fromFile(String,Charset)
    pub fn from_file_with_charset(file_name: &str, charset: Charset) -> Self {
        Self::builder()
            .with_path(file_name)
            .with_charset(charset)
            .build()
    }
    // port: SourceFile#fromPath
    pub fn from_path(path: impl AsRef<Path>, charset: Charset) -> Self {
        Self::builder()
            .with_path_buf(path.as_ref().to_path_buf())
            .with_charset(charset)
            .build()
    }
    // port: SourceFile#fromCode(String,String)
    pub fn from_code(file_name: &str, code: impl Into<JsString>) -> Self {
        Self::builder()
            .with_path(file_name)
            .with_content(code)
            .build()
    }
    // port: SourceFile#fromCode(String,String,SourceKind)
    pub fn from_code_with_kind(
        file_name: &str,
        code: impl Into<JsString>,
        kind: SourceKind,
    ) -> Self {
        Self::builder()
            .with_path(file_name)
            .with_kind(kind)
            .with_content(code)
            .build()
    }
    // port: SourceFile#stubSourceFile
    pub fn stub_source_file(file_name: &str, kind: SourceKind) -> Self {
        Self::builder()
            .with_path(file_name)
            .with_kind(kind)
            .set_is_stub_source_file_for_already_provided_input()
            .build()
    }
    // port: SourceFile#stripConfigSegment
    pub fn strip_config_segment(path: &str) -> String {
        for prefix in ["bin/", "genfiles/", "testlogs/"] {
            if let Some(rest) = path.strip_prefix(prefix) {
                return rest.into();
            }
        }
        if let Some(index) = path.find("blaze-out/").or_else(|| path.find("bazel-out/")) {
            if let Some(next) = path[index + 10..].find('/') {
                let rest = &path[index + 10 + next + 1..];
                for prefix in ["bin/", "genfiles/", "testlogs/"] {
                    if let Some(rest) = rest.strip_prefix(prefix) {
                        return rest.into();
                    }
                }
            }
        }
        path.into()
    }
    // port: SourceFile#canonicalizePathForMatching
    pub fn canonicalize_path_for_matching(path: &str) -> String {
        Self::strip_config_segment(path)
    }
    // port: SourceFile#restoreCachedStateFrom
    pub fn restore_cached_state_from(&self, proto: &SourceFileProto) {
        check_state!(
            proto.filename == self.get_name()
                || Self::canonicalize_path_for_matching(&proto.filename)
                    == Self::canonicalize_path_for_matching(self.get_name()),
            "Cannot restore state for %s from %s",
            self.get_name(),
            proto.filename
        );
        if proto.source_kind == crate::serialization::source_file_proto::SourceKind::EXTERN {
            check_state!(
                self.get_kind() == SourceKind::EXTERN,
                "TypedAST compilations must pass all extern files as externs, not js, but found %s",
                self.get_name()
            );
        }
        let mut state = self.state.lock().unwrap();
        let lines = proto.num_lines_plus_one.wrapping_sub(1);
        let bytes = proto.num_bytes_plus_one.wrapping_sub(1);
        if lines != -1 {
            state.num_lines = lines;
        }
        if bytes != -1 {
            state.num_bytes = bytes;
        }
        state.is_closure_unaware_code = proto.is_closure_unaware_code;
    }
    // port: SourceFile#fromProto(SourceFileProto)
    pub fn from_proto(proto: &SourceFileProto) -> Self {
        let kind = Self::get_source_kind_from_proto(proto);
        let file = Self::from_proto_with_kind(proto, kind);
        {
            let mut state = file.state.lock().unwrap();
            state.num_lines = proto.num_lines_plus_one.wrapping_sub(1);
            state.num_bytes = proto.num_bytes_plus_one.wrapping_sub(1);
        }
        if proto.is_closure_unaware_code {
            file.mark_as_closure_unaware_code();
        }
        file
    }
    // port: SourceFile#fromProto(SourceFileProto,SourceKind)
    fn from_proto_with_kind(proto: &SourceFileProto, kind: SourceKind) -> Self {
        match &proto.loader {
            Loader::PRELOADED_CONTENTS(code) => {
                Self::from_code_with_kind(&proto.filename, code.clone(), kind)
            }
            Loader::FILE_ON_DISK(file) => Self::builder()
                .with_charset(Self::to_charset(&file.charset))
                .with_original_path(&proto.filename)
                .with_kind(kind)
                .with_path(if file.actual_path.is_empty() {
                    &proto.filename
                } else {
                    &file.actual_path
                })
                .build(),
            Loader::ZIP_ENTRY(zip) => Self::builder()
                .with_kind(kind)
                .with_original_path(&proto.filename)
                .with_charset(Self::to_charset(&zip.charset))
                .with_zip_entry_path(&zip.zip_path, &zip.entry_name)
                .build(),
            Loader::STUB_FILE(_) => Self::builder()
                .with_kind(kind)
                .with_original_path(&proto.filename)
                .set_is_stub_source_file_for_already_provided_input()
                .build(),
            Loader::LOADER_NOT_SET => panic!(""),
        }
    }
    // port: SourceFile#getSourceKindFromProto
    fn get_source_kind_from_proto(proto: &SourceFileProto) -> SourceKind {
        use crate::serialization::source_file_proto::SourceKind as P;
        match proto.source_kind {
            P::EXTERN => SourceKind::EXTERN,
            P::CODE => SourceKind::STRONG,
            other => panic!("{other:?}"),
        }
    }
    // port: SourceFile#toCharset
    fn to_charset(charset: &str) -> Charset {
        if charset.is_empty() {
            Charset::UTF_8
        } else {
            Charset::for_name(charset)
        }
    }
    // port: SourceFile#builder
    pub fn builder() -> Builder {
        Builder::new()
    }
    // port: SourceFile#getProto
    pub fn get_proto(&self) -> SourceFileProto {
        let state = self.state.lock().unwrap();
        self.loader
            .to_proto_location_builder(self.get_name())
            .set_filename(self.get_name())
            .set_source_kind(Self::source_kind_to_proto(state.kind))
            .set_is_closure_unaware_code(state.is_closure_unaware_code)
            .set_num_lines_plus_one(state.num_lines.wrapping_add(1))
            .set_num_bytes_plus_one(state.num_bytes.wrapping_add(1))
            .build()
    }
    // port: SourceFile#sourceKindToProto
    fn source_kind_to_proto(
        kind: SourceKind,
    ) -> crate::serialization::source_file_proto::SourceKind {
        use crate::serialization::source_file_proto::SourceKind as P;
        match kind {
            SourceKind::EXTERN => P::EXTERN,
            SourceKind::STRONG | SourceKind::WEAK => P::CODE,
            SourceKind::NON_CODE => panic!("NON_CODE"),
        }
    }
}
impl StaticSourceFile for SourceFile {
    // port: SourceFile#getName
    fn get_name(&self) -> &str {
        &self.file_name
    }
    // port: SourceFile#getKind
    fn get_kind(&self) -> SourceKind {
        self.state.lock().unwrap().kind
    }
    // port: SourceFile#isClosureUnawareCode
    fn is_closure_unaware_code(&self) -> bool {
        self.state.lock().unwrap().is_closure_unaware_code
    }
    // port: SourceFile#getLineOffset
    fn get_line_offset(&self, lineno: i32) -> i32 {
        let offsets = self.find_line_offsets();
        check_argument!(
            lineno >= 1 && lineno <= offsets.len() as i32,
            "Expected line number between 1 and %s\nActual: %s",
            offsets.len(),
            lineno
        );
        offsets[(lineno - 1) as usize]
    }
    // port: SourceFile#getLineOfOffset
    fn get_line_of_offset(&self, offset: i32) -> i32 {
        let offsets = self.find_line_offsets();
        match offsets.binary_search(&offset) {
            Ok(index) => index as i32 + 1,
            Err(insertion) => ((insertion as i32 - 1).min(offsets.len() as i32 - 1)) + 1,
        }
    }
    // port: SourceFile#getColumnOfOffset
    fn get_column_of_offset(&self, offset: i32) -> i32 {
        let line = self.get_line_of_offset(offset);
        offset.wrapping_sub(self.find_line_offsets()[(line - 1) as usize])
    }
}
impl fmt::Display for SourceFile {
    // port: SourceFile#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.file_name)
    }
}
pub struct Builder {
    kind: SourceKind,
    charset: Charset,
    original_path: Option<String>,
    is_stub_source_file_for_already_provided_input: bool,
    path: Option<String>,
    path_with_filesystem: Option<PathBuf>,
    zip_entry_path: Option<String>,
    lazy_content: Option<Content>,
}
enum Content {
    Text(JsString),
    Stream(Box<dyn Read + Send>),
}
impl Builder {
    // port: SourceFile.Builder#Builder
    fn new() -> Self {
        Self {
            kind: SourceKind::STRONG,
            charset: Charset::UTF_8,
            original_path: None,
            is_stub_source_file_for_already_provided_input: false,
            path: None,
            path_with_filesystem: None,
            zip_entry_path: None,
            lazy_content: None,
        }
    }
    // port: SourceFile.Builder#withKind
    pub fn with_kind(mut self, kind: SourceKind) -> Self {
        self.kind = kind;
        self
    }
    // port: SourceFile.Builder#withCharset
    pub fn with_charset(mut self, charset: Charset) -> Self {
        self.charset = charset;
        self
    }
    // port: SourceFile.Builder#withPath(String)
    pub fn with_path(self, path: impl Into<String>) -> Self {
        self.with_path_internal(path.into(), None)
    }
    // port: SourceFile.Builder#withPath(Path)
    pub fn with_path_buf(self, path: PathBuf) -> Self {
        self.with_path_internal(path.to_string_lossy().into_owned(), Some(path))
    }
    // port: SourceFile.Builder#withContent(String)
    pub fn with_content(mut self, code: impl Into<JsString>) -> Self {
        self.lazy_content = Some(Content::Text(code.into()));
        self
    }
    // port: SourceFile.Builder#withContent(InputStream)
    pub fn with_content_stream(mut self, input: impl Read + Send + 'static) -> Self {
        self.lazy_content = Some(Content::Stream(Box::new(input)));
        self
    }
    // port: SourceFile.Builder#withZipEntryPath
    pub fn with_zip_entry_path(
        mut self,
        zip_path: impl Into<String>,
        entry_path: impl Into<String>,
    ) -> Self {
        self.path = Some(zip_path.into());
        self.zip_entry_path = Some(entry_path.into());
        self
    }
    // port: SourceFile.Builder#withOriginalPath
    pub fn with_original_path(mut self, original_path: impl Into<String>) -> Self {
        self.original_path = Some(original_path.into());
        self
    }
    // port: SourceFile.Builder#setIsStubSourceFileForAlreadyProvidedInput
    pub fn set_is_stub_source_file_for_already_provided_input(mut self) -> Self {
        self.is_stub_source_file_for_already_provided_input = true;
        self
    }
    // port: SourceFile.Builder#build
    pub fn build(self) -> SourceFile {
        let display_path = self
            .original_path
            .unwrap_or_else(|| match &self.zip_entry_path {
                None => self.path.clone().unwrap_or_default(),
                Some(entry) => format!("{}!/{entry}", self.path.as_deref().unwrap_or("null")),
            });
        if self.is_stub_source_file_for_already_provided_input {
            let file = SourceFile::new(
                CodeLoader::StubSourceFileCodeLoader,
                display_path,
                self.kind,
            );
            file.set_is_stub_source_file_for_already_provided_input(true);
            return file;
        }
        if let Some(content) = self.lazy_content {
            let code = match content {
                Content::Text(code) => code,
                Content::Stream(mut input) => {
                    let mut bytes = Vec::new();
                    input
                        .read_to_end(&mut bytes)
                        .unwrap_or_else(|e| panic!("{e}"));
                    self.charset.decode(&bytes, true).unwrap()
                }
            };
            return SourceFile::new(
                CodeLoader::Preloaded {
                    preloaded_code: code,
                },
                display_path,
                self.kind,
            );
        }
        if let Some(entry_name) = self.zip_entry_path {
            return SourceFile::new(
                CodeLoader::AtZip {
                    zip_name: self.path.unwrap(),
                    entry_name,
                    serializable_charset: self.charset.name().into(),
                },
                display_path,
                self.kind,
            );
        }
        SourceFile::new(
            CodeLoader::OnDisk {
                relative_path: self
                    .path_with_filesystem
                    .unwrap_or_else(|| PathBuf::from(self.path.unwrap())),
                serializable_charset: self.charset.name().into(),
            },
            display_path,
            self.kind,
        )
    }
    // port: SourceFile.Builder#withPathInternal
    fn with_path_internal(mut self, path: String, path_with_filesystem: Option<PathBuf>) -> Self {
        if let Some(bang) = path.find("!/") {
            let zip_path = &path[..bang];
            let entry_path = &path[bang + 2..];
            if zip_path.ends_with(".zip")
                && (entry_path.ends_with(".js") || entry_path.ends_with(".js.map"))
            {
                return self.with_zip_entry_path(zip_path, entry_path);
            }
        }
        self.path = Some(path);
        self.path_with_filesystem = path_with_filesystem;
        self
    }
}
#[derive(Debug)]
enum CodeLoader {
    // port: SourceFile.CodeLoader.Preloaded#Preloaded
    Preloaded {
        preloaded_code: JsString,
    },
    // port: SourceFile.CodeLoader.StubSourceFileCodeLoader#StubSourceFileCodeLoader
    StubSourceFileCodeLoader,
    // port: SourceFile.CodeLoader.OnDisk#OnDisk
    OnDisk {
        relative_path: PathBuf,
        serializable_charset: String,
    },
    // port: SourceFile.CodeLoader.AtZip#AtZip
    AtZip {
        zip_name: String,
        entry_name: String,
        serializable_charset: String,
    },
}
impl CodeLoader {
    // port: SourceFile.CodeLoader.Preloaded#loadUncachedCode
    // port: SourceFile.CodeLoader.StubSourceFileCodeLoader#loadUncachedCode
    // port: SourceFile.CodeLoader.OnDisk#loadUncachedCode
    // port: SourceFile.CodeLoader.AtZip#loadUncachedCode
    fn load_uncached_code(&self) -> Result<JsString, IOException> {
        match self {
            Self::Preloaded { preloaded_code } => Ok(preloaded_code.clone()),
            Self::StubSourceFileCodeLoader => {
                panic!("Attempting to load code from a stub SourceFile.")
            }
            Self::OnDisk { relative_path, .. } => {
                let path = relative_path.to_string_lossy();
                let bytes = fs::read(relative_path).map_err(|e| IOException::from_io(e, &path))?;
                self.get_charset().decode(&bytes, false).map_err(|_| {
                    IOException::new(format!(
                        "Failed to read: {path}, is this input UTF-8 encoded?"
                    ))
                })
            }
            Self::AtZip { .. } => Ok(self.open_uncached_reader()?.unwrap()),
        }
    }
    // port: SourceFile.CodeLoader#openUncachedReader
    // port: SourceFile.CodeLoader.OnDisk#openUncachedReader
    // port: SourceFile.CodeLoader.AtZip#openUncachedReader
    fn open_uncached_reader(&self) -> Result<Option<JsString>, IOException> {
        match self {
            Self::OnDisk { relative_path, .. } => {
                let bytes = fs::read(relative_path)
                    .map_err(|e| IOException::from_io(e, &relative_path.to_string_lossy()))?;
                self.get_charset().decode(&bytes, false).map(Some)
            }
            Self::AtZip {
                zip_name,
                entry_name,
                ..
            } => self
                .get_charset()
                .decode(
                    &JSCompZipFileCache::get_entry_stream(zip_name, entry_name)?,
                    true,
                )
                .map(Some),
            _ => Ok(None),
        }
    }
    // port: SourceFile.CodeLoader.OnDisk#getCharset
    // port: SourceFile.CodeLoader.AtZip#getCharset
    fn get_charset(&self) -> Charset {
        match self {
            Self::OnDisk {
                serializable_charset,
                ..
            }
            | Self::AtZip {
                serializable_charset,
                ..
            } => Charset::for_name(serializable_charset),
            _ => panic!(""),
        }
    }
    // port: SourceFile.CodeLoader.Preloaded#toProtoLocationBuilder
    // port: SourceFile.CodeLoader.StubSourceFileCodeLoader#toProtoLocationBuilder
    // port: SourceFile.CodeLoader.OnDisk#toProtoLocationBuilder
    // port: SourceFile.CodeLoader.AtZip#toProtoLocationBuilder
    fn to_proto_location_builder(&self, file_name: &str) -> SourceFileProto {
        match self {
            Self::Preloaded { preloaded_code } => {
                SourceFileProto::new_builder().set_preloaded_contents(preloaded_code.clone())
            }
            Self::StubSourceFileCodeLoader => SourceFileProto::new_builder()
                .set_filename(file_name)
                .set_stub_file(true),
            Self::OnDisk {
                relative_path,
                serializable_charset,
            } => {
                let actual_path = relative_path.to_string_lossy();
                SourceFileProto::new_builder().set_file_on_disk(
                    FileOnDisk::new_builder()
                        .set_actual_path(if file_name == actual_path {
                            ""
                        } else {
                            &actual_path
                        })
                        .set_charset(if self.get_charset() == Charset::UTF_8 {
                            ""
                        } else {
                            serializable_charset
                        })
                        .build(),
                )
            }
            Self::AtZip {
                zip_name,
                entry_name,
                serializable_charset,
            } => SourceFileProto::new_builder()
                .set_filename(file_name)
                .set_zip_entry(
                    ZipEntryOnDisk::new_builder()
                        .set_entry_name(entry_name)
                        .set_zip_path(zip_name)
                        .set_charset(if self.get_charset() == Charset::UTF_8 {
                            ""
                        } else {
                            serializable_charset
                        })
                        .build(),
                ),
        }
    }
}
