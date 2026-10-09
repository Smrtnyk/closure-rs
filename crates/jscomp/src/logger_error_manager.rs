/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/LoggerErrorManager.java.

use crate::{
    basic_error_manager::BasicErrorManager, check_level::CheckLevel, error_manager::ErrorManager,
    js_error::JSError, lightweight_message_formatter::LightweightMessageFormatter,
    message_formatter::MessageFormatter, sorting_error_manager::SortingErrorManager,
};
use closure_rhino::{java_lang::formatter::format_one_decimal, node::Ast};
/// java.util.logging.Level, identified by its integer value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    value: i32,
}
impl Level {
    pub const OFF: Level = Level::new(i32::MAX);
    pub const SEVERE: Level = Level::new(1000);
    pub const WARNING: Level = Level::new(900);
    pub const INFO: Level = Level::new(800);
    pub const CONFIG: Level = Level::new(700);
    pub const FINE: Level = Level::new(500);
    pub const FINER: Level = Level::new(400);
    pub const FINEST: Level = Level::new(300);
    pub const ALL: Level = Level::new(i32::MIN);
    // port: Level#Level(String,int)
    pub const fn new(value: i32) -> Self {
        Self { value }
    }
    // port: Level#intValue
    pub const fn int_value(self) -> i32 {
        self.value
    }
}
pub trait Logger: Send {
    // port: Logger#severe
    fn severe(&mut self, message: &str);
    // port: Logger#warning
    fn warning(&mut self, message: &str);
    // port: Logger#log
    fn log(&mut self, level: Level, message: &str);
}
pub struct LoggerErrorManager {
    manager: SortingErrorManager,
    formatter: Box<dyn MessageFormatter>,
    logger: Box<dyn Logger>,
}
impl LoggerErrorManager {
    // port: LoggerErrorManager#LoggerErrorManager(MessageFormatter,Logger)
    pub fn new(formatter: Box<dyn MessageFormatter>, logger: Box<dyn Logger>) -> Self {
        Self {
            manager: Self::new_base(),
            formatter,
            logger,
        }
    }
    // port: LoggerErrorManager#LoggerErrorManager(Logger)
    pub fn new_without_source(logger: Box<dyn Logger>) -> Self {
        Self::new(
            Box::new(LightweightMessageFormatter::without_source()),
            logger,
        )
    }
}
impl BasicErrorManager for LoggerErrorManager {
    fn sorting_manager(&self) -> &SortingErrorManager {
        &self.manager
    }
    fn sorting_manager_mut(&mut self) -> &mut SortingErrorManager {
        &mut self.manager
    }
    // port: LoggerErrorManager#println
    fn println(&mut self, ast: &Ast, level: CheckLevel, error: &JSError) {
        match level {
            CheckLevel::ERROR => self
                .logger
                .severe(&error.format(ast, level, self.formatter.as_ref()).unwrap()),
            CheckLevel::WARNING => self
                .logger
                .warning(&error.format(ast, level, self.formatter.as_ref()).unwrap()),
            CheckLevel::OFF => {}
        }
    }
    // port: LoggerErrorManager#printSummary
    fn print_summary(&mut self) {
        let level = if self.get_error_count() + self.get_warning_count() == 0 {
            Level::INFO
        } else {
            Level::WARNING
        };
        if self.get_typed_percent() > 0.0 {
            self.logger.log(
                level,
                &format!(
                    "{} error(s), {} warning(s), {}% typed",
                    self.get_error_count(),
                    self.get_warning_count(),
                    format_one_decimal(self.get_typed_percent())
                ),
            );
        } else if self.get_error_count() + self.get_warning_count() > 0 {
            self.logger.log(
                level,
                &format!(
                    "{} error(s), {} warning(s)",
                    self.get_error_count(),
                    self.get_warning_count()
                ),
            );
        }
    }
}
