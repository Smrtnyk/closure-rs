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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/diagnostic/NoOpLogFile.java.

use super::{
    log_file::{LogFile, StreamedJsonProducer},
    logs_gson::LogsGsonObject,
};
use std::fmt;

/// An implementation that does nothing to allow logging calls to be cheap when disabled.
pub(crate) struct NoOpLogFile;
impl LogFile for NoOpLogFile {
    // port: NoOpLogFile#log(Object)
    fn log_object(&mut self, _value: &dyn fmt::Display) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#log(String)
    fn log_string(&mut self, _value: &str) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#log(Supplier)
    fn log(&mut self, _value: &mut dyn FnMut() -> String) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#log(String,Object...)
    fn log_format(&mut self, _template_and_values: fmt::Arguments<'_>) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#logJson(Object)
    fn log_json_object(&mut self, _value: Option<&dyn LogsGsonObject>) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#logJson(Supplier)
    fn log_json_supplier(
        &mut self,
        _value: &mut dyn FnMut() -> Option<Box<dyn LogsGsonObject>>,
    ) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#logJson(StreamedJsonProducer)
    fn log_json(&mut self, _producer: &dyn StreamedJsonProducer) -> &mut dyn LogFile {
        self
    }
    // port: NoOpLogFile#close
    fn close(&mut self) {}
    // port: NoOpLogFile#isLogging
    fn is_logging(&self) -> bool {
        false
    }
}
