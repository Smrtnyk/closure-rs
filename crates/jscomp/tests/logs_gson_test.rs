/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/diagnostic/LogsGsonTest.java.

//! Port of diagnostic/LogsGsonTest.java.
use closure_jscomp::diagnostic::logs_gson::{Able, LogsGson, LogsGsonObject, Multimap};

// port: LogsGsonTest#assertLogsGson
fn assert_logs_gson(value: &dyn LogsGsonObject) -> String {
    LogsGson::to_json(Some(value))
}

struct TestLogsGsonable;
impl Able for TestLogsGsonable {
    // port: LogsGsonTest.TestLogsGsonable#toLogsGson
    fn to_logs_gson(&self) -> Box<dyn LogsGsonObject> {
        Box::new(vec![0])
    }
}

#[test]
// port: LogsGsonTest#adapter_write_logsGsonable
fn adapter_write_logs_gsonable() {
    assert_eq!(assert_logs_gson(&vec![TestLogsGsonable]), "[[0]]");
}

#[test]
// port: LogsGsonTest#adapter_write_multimap
fn adapter_write_multimap() {
    // Given
    let mut value: Multimap<&str, i32> = Multimap::new();
    value.put("foo", 0);
    value.put("foo", 1);
    value.put("bar", 2);

    // Then
    assert_eq!(assert_logs_gson(&value), "{\"foo\":[0,1],\"bar\":[2]}");
}
