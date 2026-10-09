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
//   org/kohsuke/args4j/CmdLineParser.java.

use super::{
    cmd_line_exception::CmdLineException,
    messages,
    named_option_def::NamedOptionDef,
    parser_properties::ParserProperties,
    spi::{
        self,
        option_handler::{HandlerKind, ParsedValue},
        parameters::Parameters,
        setter::Setter,
    },
};
use closure_rhino::fx_hash::IndexSet;
use std::fmt::Write;

pub struct CmdLineParser {
    pub options: Vec<&'static NamedOptionDef>,
    pub parser_properties: ParserProperties,
    parsing_options: bool,
}
impl CmdLineParser {
    // port: CmdLineParser#CmdLineParser
    pub fn new(options: &'static [NamedOptionDef]) -> Self {
        let mut parser = Self {
            options: options.iter().collect(),
            parser_properties: ParserProperties::default(),
            parsing_options: true,
        };
        if parser.parser_properties.sort_options {
            parser.options.sort_by_key(|o| o.to_string());
        }
        parser
    }
    // port: CmdLineParser#findOptionByName
    pub fn find_option_by_name(&self, name: &str) -> Option<&'static NamedOptionDef> {
        self.options
            .iter()
            .copied()
            .find(|o| o.name == name || o.aliases.contains(&name))
    }
    // port: CmdLineParser#findOptionHandler
    fn find_option_handler(&self, name: &str) -> Option<&'static NamedOptionDef> {
        let pos = name
            .find(&self.parser_properties.option_value_delimiter)
            .or_else(|| name.find('='));
        self.find_option_by_name(match pos {
            Some(pos) if pos > 0 => &name[..pos],
            _ => name,
        })
    }
    // port: CmdLineParser#stopOptionParsing
    pub fn stop_option_parsing(&mut self) {
        self.parsing_options = false;
    }
    // port: CmdLineParser#expandAtFiles
    fn expand_at_files(&self, args: &[String]) -> Result<Vec<String>, CmdLineException> {
        let mut result = Vec::new();
        for arg in args {
            if let Some(filename) = arg.strip_prefix('@') {
                let path = std::path::Path::new(filename);
                if !path.exists() {
                    return Err(CmdLineException(format!("No such file: {filename}")));
                }
                let bytes = std::fs::read(path)
                    .map_err(|_| CmdLineException(format!("Failed to parse {filename}")))?;
                let text = String::from_utf8_lossy(&bytes);
                // BufferedReader.readLine accepts CR, LF and CRLF, without adding a final empty line.
                let mut lines = text.split(['\n', '\r']).peekable();
                let mut offset = 0;
                while let Some(line) = lines.next() {
                    if offset == text.len() {
                        break;
                    }
                    result.push(line.into());
                    offset += line.len();
                    if text.as_bytes().get(offset) == Some(&b'\r')
                        && text.as_bytes().get(offset + 1) == Some(&b'\n')
                    {
                        lines.next();
                        offset += 2;
                    } else {
                        offset += 1;
                    }
                }
            } else {
                result.push(arg.clone());
            }
        }
        Ok(result)
    }
    // port: CmdLineParser#parseArgument
    pub fn parse_argument(
        &mut self,
        args: &[String],
        setter: &mut impl Setter,
    ) -> Result<(), CmdLineException> {
        let mut args = if self.parser_properties.at_syntax {
            self.expand_at_files(args)?
        } else {
            args.to_vec()
        };
        let mut present = IndexSet::<_>::default();
        let mut pos = 0;
        while pos < args.len() {
            let arg = args[pos].clone();
            if self.parsing_options && arg.starts_with('-') {
                let key_value = arg.contains(&self.parser_properties.option_value_delimiter)
                    || arg.contains('=');
                let option = if key_value {
                    self.find_option_handler(&arg)
                } else {
                    self.find_option_by_name(&arg)
                }
                .ok_or_else(|| CmdLineException(messages::undefined_option(&arg)))?;
                if key_value {
                    if let Some(eq) = args[pos].find('=')
                        && eq > 0
                    {
                        args[pos] = args[pos][eq + 1..].into();
                    }
                } else {
                    pos += 1;
                }
                let params = Parameters {
                    args: &args,
                    pos,
                    option_name: option.to_string(),
                };
                let (value, consumed) = match option.handler {
                    HandlerKind::String => spi::string_option_handler::parse_arguments(&params)?,
                    HandlerKind::Int => spi::int_option_handler::parse_arguments(&params)?,
                    HandlerKind::Enum(values) => {
                        spi::enum_option_handler::parse_arguments(&params, values)?
                    }
                    HandlerKind::Map => spi::map_option_handler::parse_arguments(&params)?,
                    HandlerKind::Boolean => spi::boolean_option_handler::parse_arguments(),
                    HandlerKind::ClosureBoolean => {
                        crate::command_line_runner::BooleanOptionHandler::new(self, option)
                            .parse_arguments(&params)
                    }
                };
                setter.add_value(option.index, value);
                pos += consumed;
                present.insert(option.index);
            } else {
                setter.add_argument(arg);
                pos += 1;
            }
        }
        if !self
            .options
            .iter()
            .any(|o| o.help && present.contains(&o.index))
        {
            self.check_required_options_and_arguments(&present)?;
        }
        Ok(())
    }
    // port: CmdLineParser#checkRequiredOptionsAndArguments
    fn check_required_options_and_arguments(
        &self,
        present: &IndexSet<usize>,
    ) -> Result<(), CmdLineException> {
        for option in &self.options {
            if option.required && !present.contains(&option.index) {
                return Err(CmdLineException(format!("Option \"{option}\" is required")));
            }
        }
        for index in present {
            let option = self.options.iter().find(|o| o.index == *index).unwrap();
            if option.depends.iter().any(|name| {
                !self
                    .find_option_handler(name)
                    .is_some_and(|o| present.contains(&o.index))
            }) {
                return Err(CmdLineException(format!(
                    "option \"{option}\" requires the option(s) [{}]",
                    option.depends.join(", ")
                )));
            }
        }
        for index in present {
            let option = self.options.iter().find(|o| o.index == *index).unwrap();
            if option.forbids.iter().any(|name| {
                self.find_option_handler(name)
                    .is_some_and(|o| present.contains(&o.index))
            }) {
                return Err(CmdLineException(format!(
                    "option \"{option}\" cannot be used with the option(s) [{}]",
                    option.forbids.join(", ")
                )));
            }
        }
        Ok(())
    }
    // port: CmdLineParser#wrapLines
    fn wrap_lines(line: &str, max_length: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut lines: Vec<&str> = line.split('\n').collect();
        if !line.is_empty() {
            while lines.last() == Some(&"") {
                lines.pop();
            }
        }
        for line in lines {
            let mut rest: Vec<u16> = line.encode_utf16().collect();
            while rest.len() > max_length {
                assert!(max_length > 0);
                let space = rest[..max_length].iter().rposition(|c| *c == 32);
                let length = space
                    .filter(|s| *s > max_length * 3 / 5)
                    .unwrap_or(max_length);
                result.push(String::from_utf16_lossy(&rest[..length]));
                let next = &rest[length..];
                let start = next.iter().position(|c| *c > 32).unwrap_or(next.len());
                let end = next.iter().rposition(|c| *c > 32).map_or(start, |i| i + 1);
                rest = next[start..end].to_vec();
            }
            result.push(String::from_utf16_lossy(&rest));
        }
        result
    }
    // port: CmdLineParser#createDefaultValuePart
    fn create_default_value_part(&self, handler: &NamedOptionDef, getter: &impl Setter) -> String {
        if self.parser_properties.show_defaults
            && !handler.required
            && let Some(value) = getter.print_default_value(handler.index)
        {
            return format!(" (default: {value})");
        }
        String::new()
    }
    // port: CmdLineParser#printUsage
    pub fn print_usage(
        &self,
        getter: &impl Setter,
        filter: impl Fn(&NamedOptionDef) -> bool,
    ) -> String {
        let len = self
            .options
            .iter()
            .filter(|o| !o.usage.is_empty())
            .map(|o| {
                o.get_name_and_meta(&self.parser_properties)
                    .encode_utf16()
                    .count()
            })
            .max()
            .unwrap_or(0);
        let mut result = String::new();
        for handler in &self.options {
            if !handler.usage.is_empty() && filter(handler) {
                self.print_option(&mut result, handler, len, getter);
            }
        }
        result
    }
    // port: CmdLineParser#printOption
    fn print_option(
        &self,
        out: &mut String,
        handler: &NamedOptionDef,
        len: usize,
        getter: &impl Setter,
    ) {
        let total = self.parser_properties.usage_width;
        let metadata = len.min((total - 4) / 2);
        let usage_width = total - 4 - metadata;
        let names = Self::wrap_lines(
            &handler.get_name_and_meta(&self.parser_properties),
            metadata,
        );
        let usages = Self::wrap_lines(
            &(handler.usage.to_string() + &self.create_default_value_part(handler, getter)),
            usage_width,
        );
        for i in 0..names.len().max(usages.len()) {
            let name = names.get(i).map(String::as_str).unwrap_or("");
            let usage = usages.get(i).map(String::as_str).unwrap_or("");
            let delimiter = if !name.is_empty() && i == 0 {
                " : "
            } else {
                "   "
            };
            writeln!(
                out,
                " {name}{}{delimiter}{usage}",
                " ".repeat(metadata.saturating_sub(name.encode_utf16().count()))
            )
            .unwrap();
        }
    }
    // port: CmdLineParser#printSingleLineUsage
    pub fn print_single_line_usage(&self) -> String {
        let mut out = " [VAL ...]".to_string();
        for h in &self.options {
            out.push(' ');
            if !h.required {
                out.push('[');
            }
            out.push_str(&h.get_name_and_meta(&self.parser_properties));
            // NamedOptionDef's super constructor uses false for isMultiValued.
            if !h.required {
                out.push(']');
            }
        }
        out
    }
}
// Preserve the public value type used by setters and handler adapters.
const _: Option<ParsedValue> = None;
