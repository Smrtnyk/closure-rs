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
//   src/com/google/javascript/jscomp/diagnostic/LogFile.java.

use super::{
    logs_gson::LogsGsonObject, no_op_log_file::NoOpLogFile, writing_log_file::WritingLogFile,
};
use closure_sourcemap::gson::stream::json_writer::JsonWriter;
use std::{fmt, path::Path};

/// An interface allowing streaming JSON to a LogFile via a JsonWriter.
///
/// Use with LogFile#logJson(StreamedJsonProducer).
pub trait StreamedJsonProducer {
    // port: LogFile.StreamedJsonProducer#writeJson
    fn write_json(&self, json_writer: &mut JsonWriter) -> std::io::Result<()>;
}

/// A simple interface for writing to a human readable log file.
///
/// The Java class is abstract with package-private subclasses; it closes like an AutoCloseable
/// (`close`, and on drop for try-with-resources scopes).
pub trait LogFile: Send {
    // port: LogFile#log(Object)
    fn log_object(&mut self, value: &dyn fmt::Display) -> &mut dyn LogFile;
    // port: LogFile#log(String)
    fn log_string(&mut self, value: &str) -> &mut dyn LogFile;
    // port: LogFile#log(Supplier)
    fn log(&mut self, value: &mut dyn FnMut() -> String) -> &mut dyn LogFile;
    // port: LogFile#log(String,Object...)
    fn log_format(&mut self, template_and_values: fmt::Arguments<'_>) -> &mut dyn LogFile;
    // port: LogFile#logJson(Object)
    fn log_json_object(&mut self, value: Option<&dyn LogsGsonObject>) -> &mut dyn LogFile;
    // port: LogFile#logJson(Supplier)
    fn log_json_supplier(
        &mut self,
        value: &mut dyn FnMut() -> Option<Box<dyn LogsGsonObject>>,
    ) -> &mut dyn LogFile;
    // port: LogFile#logJson(StreamedJsonProducer)
    fn log_json(&mut self, producer: &dyn StreamedJsonProducer) -> &mut dyn LogFile;
    // Whether or not the results are being recorded.
    // port: LogFile#isLogging
    fn is_logging(&self) -> bool;
    // port: LogFile#close
    fn close(&mut self);
}

// port: LogFile#createOrReopen
pub fn create_or_reopen(file: &Path) -> Box<dyn LogFile> {
    WritingLogFile::create(file)
}

// port: LogFile#createNoOp
pub fn create_no_op() -> Box<dyn LogFile> {
    Box::new(NoOpLogFile)
}
