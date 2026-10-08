/*
 * Copyright 2026 The closure-rs Authors.
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

// The Level.parse subset used by the command-line runner under LANG=C.UTF-8.
use closure_jscomp::logger_error_manager::Level;
// port: Level#parse
pub fn parse(name: &str) -> Result<Level, crate::abstract_command_line_runner::RunnerException> {
    let value = match name {
        "OFF" => Some(Level::OFF),
        "SEVERE" => Some(Level::SEVERE),
        "WARNING" => Some(Level::WARNING),
        "INFO" => Some(Level::INFO),
        "CONFIG" => Some(Level::CONFIG),
        "FINE" => Some(Level::FINE),
        "FINER" => Some(Level::FINER),
        "FINEST" => Some(Level::FINEST),
        "ALL" => Some(Level::ALL),
        _ => closure_rhino::java_lang::parse_int(&name.encode_utf16().collect::<Vec<_>>(), 10)
            .ok()
            .map(Level::new),
    };
    value.ok_or_else(|| crate::abstract_command_line_runner::RunnerException::java_exception(
        "java.lang.IllegalArgumentException", format!("Bad level \"{name}\""),
        vec!["java.logging/java.util.logging.Level.parse(Level.java:527)".into(),
            "com.google.javascript.jscomp.AbstractCommandLineRunner.doRun(AbstractCommandLineRunner.java:1157)".into()]))
}
