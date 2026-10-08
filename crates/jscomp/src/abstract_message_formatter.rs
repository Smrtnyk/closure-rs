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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AbstractMessageFormatter.java.

use crate::{check_level::CheckLevel, source_excerpt_provider::SourceExcerptProvider};
use std::sync::Arc;
pub struct AbstractMessageFormatter {
    source: Option<Arc<dyn SourceExcerptProvider>>,
    colorize: bool,
}
impl AbstractMessageFormatter {
    // port: AbstractMessageFormatter#AbstractMessageFormatter
    pub fn new(source: Option<Arc<dyn SourceExcerptProvider>>) -> Self {
        Self {
            source,
            colorize: false,
        }
    }
    // port: AbstractMessageFormatter#setColorize
    pub fn set_colorize(&mut self, colorize: bool) {
        self.colorize = colorize;
    }
    // port: AbstractMessageFormatter#getSource
    pub fn get_source(&self) -> Option<&Arc<dyn SourceExcerptProvider>> {
        self.source.as_ref()
    }
    // port: AbstractMessageFormatter#termSupportsColor
    pub fn term_supports_color(term: &str) -> bool {
        ["xterm", "xterm-color", "xterm-256color", "screen-bce"].contains(&term)
    }
    // port: AbstractMessageFormatter#getLevelName
    pub fn get_level_name(&self, level: CheckLevel) -> String {
        match level {
            CheckLevel::ERROR => self.maybe_colorize("ERROR", Color::ERROR),
            CheckLevel::WARNING => self.maybe_colorize("WARNING", Color::WARNING),
            _ => level.to_string(),
        }
    }
    // port: AbstractMessageFormatter#maybeEmbolden
    pub fn maybe_embolden(&self, text: &str) -> String {
        if !self.colorize {
            text.into()
        } else {
            format!(
                "{}{text}{}",
                Color::BOLD.get_control_character(),
                Color::UNBOLD.get_control_character()
            )
        }
    }
    // port: AbstractMessageFormatter#maybeColorize
    fn maybe_colorize(&self, text: &str, color: Color) -> String {
        if !self.colorize {
            text.into()
        } else {
            format!(
                "{}{text}{}",
                color.get_control_character(),
                Color::NO_COLOR.get_control_character()
            )
        }
    }
}
// port: AbstractMessageFormatter.Color#Color
#[derive(Debug, Clone, Copy)]
pub enum Color {
    ERROR,
    WARNING,
    NO_COLOR,
    BOLD,
    UNBOLD,
}
impl Color {
    // port: AbstractMessageFormatter.Color#getControlCharacter
    pub fn get_control_character(self) -> &'static str {
        match self {
            Self::ERROR => "\u{1b}[31m",
            Self::WARNING => "\u{1b}[35m",
            Self::NO_COLOR => "\u{1b}[39m",
            Self::BOLD => "\u{1b}[1m",
            Self::UNBOLD => "\u{1b}[0m",
        }
    }
}
