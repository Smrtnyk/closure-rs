/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/diagnostic/WritingLogFile.java.

use super::{
    log_file::{LogFile, StreamedJsonProducer},
    logs_gson::{LogsGson, LogsGsonObject},
};
use closure_sourcemap::gson::stream::json_writer::JsonWriter;
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
};

/// An implementation that adapts a BufferedWriter and writes to a real file.
pub(crate) struct WritingLogFile {
    // None once the BufferedWriter is closed (Java then throws "Stream closed").
    writer: Option<BufWriter<File>>,
}
impl WritingLogFile {
    // port: WritingLogFile#create
    pub(crate) fn create(file: &Path) -> Box<dyn LogFile> {
        let dir = file.parent().unwrap_or_else(|| Path::new(""));
        if let Err(e) = std::fs::create_dir_all(dir) {
            panic!("java.lang.RuntimeException: {e}");
        }
        // Allow the logged string to contain invalid Unicode (replace invalid character sequences
        // but don't throw): the writes below go through Java's replacing UTF-8 encoder.
        let output = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .unwrap_or_else(|e| panic!("java.lang.RuntimeException: {e}"));
        Box::new(WritingLogFile::new(BufWriter::new(output)))
    }
    // port: WritingLogFile#WritingLogFile
    fn new(writer: BufWriter<File>) -> Self {
        Self {
            writer: Some(writer),
        }
    }
    fn writer(&mut self) -> std::io::Result<&mut BufWriter<File>> {
        self.writer
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Stream closed"))
    }
    // port: WritingLogFile#logInternal
    fn log_internal(&mut self, value: &str) -> &mut dyn LogFile {
        // It's fine to pass a fully rendered string because we know we're going to use it by the
        // time this method is called.
        let result = self.writer().and_then(|writer| {
            writer.write_all(value.as_bytes())?;
            writer.write_all(b"\n")
        });
        if let Err(e) = result {
            panic!("java.lang.RuntimeException: {e}");
        }
        self
    }
}
impl LogFile for WritingLogFile {
    // port: WritingLogFile#log(Object)
    fn log_object(&mut self, value: &dyn fmt::Display) -> &mut dyn LogFile {
        self.log_internal(&value.to_string())
    }
    // port: WritingLogFile#log(String)
    fn log_string(&mut self, value: &str) -> &mut dyn LogFile {
        self.log_internal(value)
    }
    // port: WritingLogFile#log(Supplier)
    fn log(&mut self, value: &mut dyn FnMut() -> String) -> &mut dyn LogFile {
        self.log_internal(&value())
    }
    // port: WritingLogFile#log(String,Object...)
    fn log_format(&mut self, template_and_values: fmt::Arguments<'_>) -> &mut dyn LogFile {
        self.log_internal(&fmt::format(template_and_values))
    }
    // port: WritingLogFile#logJson(Object)
    fn log_json_object(&mut self, value: Option<&dyn LogsGsonObject>) -> &mut dyn LogFile {
        self.log_internal(&LogsGson::to_json(value))
    }
    // port: WritingLogFile#logJson(Supplier)
    fn log_json_supplier(
        &mut self,
        value: &mut dyn FnMut() -> Option<Box<dyn LogsGsonObject>>,
    ) -> &mut dyn LogFile {
        let value = value();
        self.log_internal(&LogsGson::to_json(value.as_deref()))
    }
    // port: WritingLogFile#logJson(StreamedJsonProducer)
    fn log_json(&mut self, producer: &dyn StreamedJsonProducer) -> &mut dyn LogFile {
        // try (JsonWriter writer = new JsonWriter(this.writer)): closing the JsonWriter closes
        // this log's BufferedWriter.
        let mut writer = JsonWriter::new(Vec::new());
        let produced = producer.write_json(&mut writer);
        // JsonWriter#close closes `out` first, then checks for an incomplete document.
        let close = writer.close();
        let bytes = closure_rhino::java_lang::string::get_bytes_utf8(&writer.into_string());
        let closed_out = match self.writer.take() {
            Some(mut out) => out.write_all(&bytes).and_then(|()| out.flush()),
            None => Err(std::io::Error::other("Stream closed")),
        };
        let result = produced.and(closed_out).and(close);
        if let Err(ex) = result {
            panic!("java.lang.AssertionError: java.io.IOException: {ex}");
        }
        self
    }
    // port: WritingLogFile#close
    fn close(&mut self) {
        if let Some(mut writer) = self.writer.take()
            && let Err(e) = writer.flush()
        {
            panic!("java.lang.RuntimeException: {e}");
        }
    }
    // port: WritingLogFile#isLogging
    fn is_logging(&self) -> bool {
        true
    }
}
impl Drop for WritingLogFile {
    // try-with-resources: callers' log scopes close the file when they end.
    fn drop(&mut self) {
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.flush();
        }
    }
}
