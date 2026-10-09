/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Platform.java.

use closure_rhino::java_lang::message_format::MessageFormat;
pub struct Platform;
impl Platform {
    // port: Platform#getFileSeperator
    pub fn get_file_seperator() -> &'static str {
        std::path::MAIN_SEPARATOR_STR
    }
    // Rust has no JVM interrupt flag or managed heap. The platform hooks retain
    // their signatures, with zero heap accounting and no pending interruption.
    // port: Platform#isThreadInterrupted
    pub fn is_thread_interrupted() -> bool {
        false
    }
    // port: Platform#freeMemory
    pub fn free_memory() -> i64 {
        0
    }
    // port: Platform#totalMemory
    pub fn total_memory() -> i64 {
        0
    }
    // port: Platform#formatMessage
    pub fn format_message(message: &str, arguments: &[&str]) -> String {
        MessageFormat::format(message, arguments)
    }
}
