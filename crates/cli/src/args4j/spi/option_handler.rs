/*
 * Copyright (c) 2013 Kohsuke Kawaguchi and other contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the "Software"), to deal in
 * the Software without restriction, including without limitation the rights to
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is furnished to do
 * so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
// Ported from args4j 2.33 (https://github.com/kohsuke/args4j):
//   org/kohsuke/args4j/spi/OptionHandler.java.

use crate::args4j::{named_option_def::NamedOptionDef, parser_properties::ParserProperties};
#[derive(Clone, Copy, Debug)]
pub enum HandlerKind {
    String,
    Int,
    Boolean,
    ClosureBoolean,
    Enum(&'static [&'static str]),
    Map,
}
#[derive(Clone, Debug)]
pub enum ParsedValue {
    String(String),
    Int(i32),
    Boolean(bool),
    Map(String, Option<String>),
}
impl ParsedValue {
    pub fn into_string(self) -> String {
        if let Self::String(value) = self {
            value
        } else {
            unreachable!()
        }
    }
    pub fn into_int(self) -> i32 {
        if let Self::Int(value) = self {
            value
        } else {
            unreachable!()
        }
    }
    pub fn into_bool(self) -> bool {
        if let Self::Boolean(value) = self {
            value
        } else {
            unreachable!()
        }
    }
    pub fn into_map(self) -> (String, Option<String>) {
        if let Self::Map(key, value) = self {
            (key, value)
        } else {
            unreachable!()
        }
    }
}
impl NamedOptionDef {
    // port: OptionHandler#getMetaVariable
    pub fn get_meta_variable(&self) -> Option<String> {
        if let HandlerKind::Enum(values) = self.handler {
            return Some(format!("[{}]", values.join(" | ")));
        }
        if !self.meta_var.is_empty() {
            return Some(self.meta_var.into());
        }
        match self.handler {
            HandlerKind::String => Some("VAL".into()),
            HandlerKind::Int => Some("N".into()),
            _ => None,
        }
    }
    // port: OptionHandler#getNameAndMeta
    pub fn get_name_and_meta(&self, properties: &ParserProperties) -> String {
        let mut value = self.to_string();
        if let Some(meta) = self.get_meta_variable() {
            value.push_str(&properties.option_value_delimiter);
            value.push_str(&meta);
        }
        value
    }
}
