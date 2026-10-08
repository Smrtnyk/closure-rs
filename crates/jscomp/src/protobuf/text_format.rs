// Protocol Buffers - Google's data interchange format
// Copyright 2008 Google Inc.  All rights reserved.
//
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file or at
// https://developers.google.com/open-source/licenses/bsd

// Ported from Protocol Buffers for Java 4.30.2 (https://github.com/protocolbuffers/protobuf):
//   java/core/src/main/java/com/google/protobuf/AbstractMessage.java,
//   java/core/src/main/java/com/google/protobuf/Descriptors.java,
//   java/core/src/main/java/com/google/protobuf/TextFormat.java,
//   java/core/src/main/java/com/google/protobuf/TextFormatEscaper.java.

//! Port of the parts of `com.google.protobuf.TextFormat` (protobuf-java 4.30.2, the version the
//! pinned Closure Compiler builds with) that Closure uses for `conformance.proto`:
//! `TextFormat.merge(CharSequence, Message.Builder)` (the default `Parser`: no extensions,
//! `ALLOW_SINGULAR_OVERWRITES`, recursion limit 100) and `TextFormat.printer().printToString`.
//!
//! The Java code works on descriptors and reflection; here each message implements [`Message`],
//! which exposes its `FieldDescriptor`s (in field-number order, like `getAllFields`) and typed
//! setters. Only the field types used by `conformance.proto` (string, bool, enum, message) exist.

use closure_rhino::java_lang::{self, NumberFormatException};
use closure_rhino::js_string::JsString;

/// The Java type of a field (`FieldDescriptor#getType`), restricted to the types the conformance
/// protos use.
#[derive(Clone, Copy, Debug)]
pub enum FieldKind {
    String,
    Bool,
    /// An enum field: the enum's full name and its values (name, number) in declaration order.
    Enum(&'static str, &'static [(&'static str, i32)]),
    Message,
}

/// Port of `Descriptors.FieldDescriptor` for the fields this port needs.
#[derive(Debug)]
pub struct FieldDescriptor {
    name: &'static str,
    number: i32,
    kind: FieldKind,
    repeated: bool,
}

impl FieldDescriptor {
    pub const fn new(name: &'static str, number: i32, kind: FieldKind, repeated: bool) -> Self {
        Self {
            name,
            number,
            kind,
            repeated,
        }
    }
    // port: FieldDescriptor#getName
    pub fn get_name(&self) -> &'static str {
        self.name
    }
    // port: FieldDescriptor#getNumber
    pub fn get_number(&self) -> i32 {
        self.number
    }
    // port: FieldDescriptor#isRepeated
    pub fn is_repeated(&self) -> bool {
        self.repeated
    }
    pub fn kind(&self) -> FieldKind {
        self.kind
    }
    fn is_message(&self) -> bool {
        matches!(self.kind, FieldKind::Message)
    }
}

impl std::fmt::Display for FieldDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

/// One value of a field, as `Message#getField` returns it.
pub enum FieldValue<'a> {
    String(&'a str),
    Bool(bool),
    Enum(i32),
    Message(&'a dyn Message),
}

/// The reflection surface of a generated message (`Message` + `Message.Builder` +
/// `MessageReflection.MergeTarget`).
pub trait Message {
    /// `Descriptor#getFullName`.
    fn full_name(&self) -> &'static str;
    /// `Descriptor#getFields`, in field-number order.
    fn fields(&self) -> &'static [FieldDescriptor];
    /// The value(s) of a field: empty when a singular field is unset or a repeated field is empty.
    fn get_field(&self, number: i32) -> Vec<FieldValue<'_>>;
    /// `setField` (singular) / `addRepeatedField` (repeated) of a string field.
    fn set_string(&mut self, number: i32, _value: String) {
        unreachable!("no string field {number}")
    }
    /// `setField` / `addRepeatedField` of a bool field.
    fn set_bool(&mut self, number: i32, _value: bool) {
        unreachable!("no bool field {number}")
    }
    /// `setField` / `addRepeatedField` of an enum field (by number; always a declared value).
    fn set_enum(&mut self, number: i32, _value: i32) {
        unreachable!("no enum field {number}")
    }
    /// `newMergeTargetForField` + `finish` + `addRepeatedField` of a repeated message field: a
    /// fresh sub-message is filled by `fill` and added only when `fill` succeeds.
    fn merge_message_field(
        &mut self,
        number: i32,
        _fill: &mut dyn FnMut(&mut dyn Message) -> Result<(), ParseException>,
    ) -> Result<(), ParseException> {
        unreachable!("no message field {number}")
    }
    // port: Descriptor#findFieldByName
    fn find_field_by_name(&self, name: &str) -> Option<&'static FieldDescriptor> {
        self.fields().iter().find(|f| f.name == name)
    }
}

/// Port of `Message#getAllFields().keySet()`: the set fields, in field-number order.
// port: AbstractMessage#getAllFields
pub fn get_all_fields(message: &dyn Message) -> Vec<&'static FieldDescriptor> {
    message
        .fields()
        .iter()
        .filter(|f| !message.get_field(f.number).is_empty())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Printer

/// Port of `TextFormat.Printer` with the default settings of `TextFormat.printer()`
/// (`escapeNonAscii = true`, no short repeated primitives, no redaction).
pub struct Printer {
    escape_non_ascii: bool,
}

// port: TextFormat#printer
pub fn printer() -> Printer {
    Printer {
        escape_non_ascii: true,
    }
}

impl Printer {
    // port: TextFormat.Printer#printToString(MessageOrBuilder)
    pub fn print_to_string(&self, message: &dyn Message) -> String {
        let mut text = String::new();
        let mut generator = TextGenerator::new(&mut text, false);
        self.print(message, &mut generator);
        text
    }

    // port: TextFormat.Printer#print(MessageOrBuilder,TextGenerator)
    fn print(&self, message: &dyn Message, generator: &mut TextGenerator<'_>) {
        self.print_message(message, generator);
    }

    // port: TextFormat.Printer#printMessage
    fn print_message(&self, message: &dyn Message, generator: &mut TextGenerator<'_>) {
        for field in get_all_fields(message) {
            self.print_field(field, message.get_field(field.number), generator);
        }
        // printUnknownFields: these messages never carry unknown fields (TextFormat.merge rejects
        // them and nothing here parses the wire format).
    }

    // port: TextFormat.Printer#printField
    fn print_field(
        &self,
        field: &FieldDescriptor,
        value: Vec<FieldValue<'_>>,
        generator: &mut TextGenerator<'_>,
    ) {
        // Map fields and short repeated primitives do not occur (useShortRepeatedPrimitives is
        // false); repeated and singular fields both print each value as a single field.
        for element in value {
            self.print_single_field(field, element, generator);
        }
    }

    // port: TextFormat.Printer#printSingleField
    fn print_single_field(
        &self,
        field: &FieldDescriptor,
        value: FieldValue<'_>,
        generator: &mut TextGenerator<'_>,
    ) {
        generator.print(field.get_name());
        if field.is_message() {
            generator.print(" {");
            generator.eol();
            generator.indent();
        } else {
            generator.print(": ");
        }
        self.print_field_value(field, value, generator);
        if field.is_message() {
            generator.outdent();
            generator.print("}");
        }
        generator.eol();
    }

    // port: TextFormat.Printer#printFieldValue
    fn print_field_value(
        &self,
        field: &FieldDescriptor,
        value: FieldValue<'_>,
        generator: &mut TextGenerator<'_>,
    ) {
        match value {
            FieldValue::Bool(b) => generator.print(if b { "true" } else { "false" }),
            FieldValue::String(s) => {
                generator.print("\"");
                if self.escape_non_ascii {
                    generator.print(&escape_text(s));
                } else {
                    generator.print(&escape_double_quotes_and_backslashes(s).replace('\n', "\\n"));
                }
                generator.print("\"");
            }
            FieldValue::Enum(number) => {
                let FieldKind::Enum(_, values) = field.kind else {
                    unreachable!()
                };
                match values.iter().find(|(_, n)| *n == number) {
                    Some((name, _)) => generator.print(name),
                    None => generator.print(&number.to_string()),
                }
            }
            FieldValue::Message(m) => self.print(m, generator),
        }
    }
}

/// Port of `TextFormat.TextGenerator`.
struct TextGenerator<'a> {
    output: &'a mut String,
    indent: String,
    single_line_mode: bool,
    at_start_of_line: bool,
}

impl<'a> TextGenerator<'a> {
    fn new(output: &'a mut String, single_line_mode: bool) -> Self {
        Self {
            output,
            indent: String::new(),
            single_line_mode,
            at_start_of_line: false,
        }
    }
    // port: TextFormat.TextGenerator#indent
    fn indent(&mut self) {
        self.indent.push_str("  ");
    }
    // port: TextFormat.TextGenerator#outdent
    fn outdent(&mut self) {
        let length = self.indent.len();
        if length == 0 {
            panic!(" Outdent() without matching Indent().");
        }
        self.indent.truncate(length - 2);
    }
    // port: TextFormat.TextGenerator#print
    fn print(&mut self, text: &str) {
        if self.at_start_of_line {
            self.at_start_of_line = false;
            if self.single_line_mode {
                self.output.push(' ');
            } else {
                self.output.push_str(&self.indent);
            }
        }
        self.output.push_str(text);
    }
    // port: TextFormat.TextGenerator#eol
    fn eol(&mut self) {
        if !self.single_line_mode {
            self.output.push('\n');
        }
        self.at_start_of_line = true;
    }
}

// port: TextFormatEscaper#escapeText
pub fn escape_text(input: &str) -> String {
    escape_bytes(input.as_bytes())
}

// port: TextFormatEscaper#escapeBytes(ByteSequence)
pub fn escape_bytes(input: &[u8]) -> String {
    let mut builder = String::with_capacity(input.len());
    for &b in input {
        match b {
            0x07 => builder.push_str("\\a"),
            0x08 => builder.push_str("\\b"),
            0x0c => builder.push_str("\\f"),
            b'\n' => builder.push_str("\\n"),
            b'\r' => builder.push_str("\\r"),
            b'\t' => builder.push_str("\\t"),
            0x0b => builder.push_str("\\v"),
            b'\\' => builder.push_str("\\\\"),
            b'\'' => builder.push_str("\\'"),
            b'"' => builder.push_str("\\\""),
            _ => {
                // Java's byte is signed: only 0x20..=0x7e print as themselves.
                if (0x20..=0x7e).contains(&b) {
                    builder.push(b as char);
                } else {
                    builder.push('\\');
                    builder.push((b'0' + ((b >> 6) & 3)) as char);
                    builder.push((b'0' + ((b >> 3) & 7)) as char);
                    builder.push((b'0' + (b & 7)) as char);
                }
            }
        }
    }
    builder
}

// port: TextFormatEscaper#escapeDoubleQuotesAndBackslashes
fn escape_double_quotes_and_backslashes(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------------------------
// Parser

/// Port of `TextFormat.ParseException`: `getMessage()` is `line:column: message`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseException {
    line: i32,
    column: i32,
    message: String,
}

impl ParseException {
    // port: TextFormat.ParseException#ParseException(int,int,String)
    pub fn new(line: i32, column: i32, message: impl AsRef<str>) -> Self {
        Self {
            line,
            column,
            message: format!("{}:{}: {}", line, column, message.as_ref()),
        }
    }
    // port: TextFormat.ParseException#getLine
    pub fn get_line(&self) -> i32 {
        self.line
    }
    // port: TextFormat.ParseException#getColumn
    pub fn get_column(&self) -> i32 {
        self.column
    }
    // port: Throwable#getMessage
    pub fn get_message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ParseException {
    /// `Throwable#toString`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "com.google.protobuf.TextFormat$ParseException: {}",
            self.message
        )
    }
}

impl std::error::Error for ParseException {}

/// Port of `TextFormat.InvalidEscapeSequenceException` (its message).
#[derive(Debug)]
struct InvalidEscapeSequenceException(String);

/// Port of `TextFormat.Tokenizer` (protobuf-java 4.30.2: the hand-written tokenizer). The text is
/// indexed in UTF-16 code units like Java's `CharSequence`.
struct Tokenizer {
    text: Vec<u16>,
    current_token: Vec<u16>,
    pos: usize,
    line: i32,
    column: i32,
    line_info_tracking_pos: usize,
    previous_line: i32,
    previous_column: i32,
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn lossy(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

impl Tokenizer {
    // port: TextFormat.Tokenizer#Tokenizer
    fn new(text: &str) -> Self {
        let mut t = Self {
            text: units(text),
            current_token: Vec::new(),
            pos: 0,
            line: 0,
            column: 0,
            line_info_tracking_pos: 0,
            previous_line: 0,
            previous_column: 0,
        };
        t.skip_whitespace();
        t.next_token();
        t
    }

    // port: TextFormat.Tokenizer#getPreviousLine
    fn get_previous_line(&self) -> i32 {
        self.previous_line
    }
    // port: TextFormat.Tokenizer#getPreviousColumn
    fn get_previous_column(&self) -> i32 {
        self.previous_column
    }

    // port: TextFormat.Tokenizer#atEnd
    fn at_end(&self) -> bool {
        self.current_token.is_empty()
    }

    // port: TextFormat.Tokenizer#nextToken
    fn next_token(&mut self) {
        self.previous_line = self.line;
        self.previous_column = self.column;

        // Advance the line counter to the current position.
        while self.line_info_tracking_pos < self.pos {
            if self.text[self.line_info_tracking_pos] == b'\n' as u16 {
                self.line += 1;
                self.column = 0;
            } else {
                self.column += 1;
            }
            self.line_info_tracking_pos += 1;
        }

        // Match the next token.
        if self.pos == self.text.len() {
            self.current_token = Vec::new(); // EOF
        } else {
            self.current_token = self.next_token_internal();
            self.skip_whitespace();
        }
    }

    // port: TextFormat.Tokenizer#nextTokenInternal
    fn next_token_internal(&mut self) -> Vec<u16> {
        let text_length = self.text.len();
        let start_pos = self.pos;
        let start_char = self.text[start_pos];

        let mut end_pos = self.pos;
        if is_alpha_under(start_char) {
            // Identifier
            loop {
                end_pos += 1;
                if end_pos == text_length {
                    break;
                }
                let c = self.text[end_pos];
                if !(is_alpha_under(c) || is_digit_plus_minus(c)) {
                    break;
                }
            }
        } else if is_digit_plus_minus(start_char) || start_char == b'.' as u16 {
            // Number
            if start_char == b'.' as u16 {
                // Optional leading dot
                end_pos += 1;
                if end_pos == text_length {
                    return self.next_token_single_char();
                }
                if !is_digit_plus_minus(self.text[end_pos]) {
                    // Mandatory first digit
                    return self.next_token_single_char();
                }
            }
            loop {
                end_pos += 1;
                if end_pos == text_length {
                    break;
                }
                let c = self.text[end_pos];
                if !(is_digit_plus_minus(c) || is_alpha_under(c) || c == b'.' as u16) {
                    break;
                }
            }
        } else if start_char == b'"' as u16 || start_char == b'\'' as u16 {
            // String
            loop {
                end_pos += 1;
                if end_pos == text_length {
                    break;
                }
                let c = self.text[end_pos];
                if c == start_char {
                    end_pos += 1;
                    break; // Quote terminates
                } else if c == b'\n' as u16 {
                    break; // Newline terminates (error during parsing) (not consumed)
                } else if c == b'\\' as u16 {
                    end_pos += 1;
                    if end_pos == text_length {
                        break; // Escape into end-of-text terminates (error during parsing)
                    } else if self.text[end_pos] == b'\n' as u16 {
                        break; // Escape into newline terminates (error during parsing) (not consumed)
                    }
                    // Otherwise the escaped character is consumed.
                }
            }
        } else {
            return self.next_token_single_char(); // Unrecognized start character
        }

        self.pos = end_pos;
        self.text[start_pos..end_pos].to_vec()
    }

    // port: TextFormat.Tokenizer#nextTokenSingleChar
    fn next_token_single_char(&mut self) -> Vec<u16> {
        let c = self.text[self.pos];
        self.pos += 1;
        vec![c]
    }

    // port: TextFormat.Tokenizer#skipWhitespace
    fn skip_whitespace(&mut self) {
        let text_length = self.text.len();
        let mut end_pos = self.pos;
        while end_pos != text_length {
            let c = self.text[end_pos];
            if c == b'#' as u16 {
                loop {
                    end_pos += 1;
                    if end_pos == text_length {
                        break;
                    }
                    if self.text[end_pos] == b'\n' as u16 {
                        break; // Consume the newline as whitespace.
                    }
                }
                if end_pos == text_length {
                    break;
                }
            } else if is_whitespace(c) {
                // OK
            } else {
                break;
            }
            end_pos += 1;
        }
        self.pos = end_pos;
    }

    fn token_is(&self, token: &str) -> bool {
        self.current_token.iter().copied().eq(token.encode_utf16())
    }

    // port: TextFormat.Tokenizer#tryConsume
    fn try_consume(&mut self, token: &str) -> bool {
        if self.token_is(token) {
            self.next_token();
            true
        } else {
            false
        }
    }

    // port: TextFormat.Tokenizer#consume
    fn consume(&mut self, token: &str) -> Result<(), ParseException> {
        if !self.try_consume(token) {
            return Err(self.parse_exception(&format!("Expected \"{token}\".")));
        }
        Ok(())
    }

    // port: TextFormat.Tokenizer#lookingAtInteger
    fn looking_at_integer(&self) -> bool {
        if self.current_token.is_empty() {
            return false;
        }
        is_digit_plus_minus(self.current_token[0])
    }

    // port: TextFormat.Tokenizer#lookingAt
    fn looking_at(&self, text: &str) -> bool {
        self.token_is(text)
    }

    // port: TextFormat.Tokenizer#consumeIdentifier
    fn consume_identifier(&mut self) -> Result<String, ParseException> {
        for &c in &self.current_token {
            if is_alpha_under(c) || (b'0' as u16..=b'9' as u16).contains(&c) || c == b'.' as u16 {
                // OK
            } else {
                return Err(self.parse_exception(&format!(
                    "Expected identifier. Found '{}'",
                    lossy(&self.current_token)
                )));
            }
        }
        let result = lossy(&self.current_token);
        self.next_token();
        Ok(result)
    }

    // port: TextFormat.Tokenizer#tryConsumeIdentifier
    fn try_consume_identifier(&mut self) -> bool {
        self.consume_identifier().is_ok()
    }

    // port: TextFormat.Tokenizer#consumeInt32
    fn consume_int32(&mut self) -> Result<i32, ParseException> {
        match parse_int32(&self.current_token) {
            Ok(result) => {
                self.next_token();
                Ok(result)
            }
            Err(e) => Err(self.integer_parse_exception(&e)),
        }
    }

    // port: TextFormat.Tokenizer#consumeInt64
    fn consume_int64(&mut self) -> Result<i64, ParseException> {
        match parse_int64(&self.current_token) {
            Ok(result) => {
                self.next_token();
                Ok(result)
            }
            Err(e) => Err(self.integer_parse_exception(&e)),
        }
    }

    // port: TextFormat.Tokenizer#tryConsumeInt64
    fn try_consume_int64(&mut self) -> bool {
        self.consume_int64().is_ok()
    }

    // port: TextFormat.Tokenizer#consumeUInt64
    fn consume_uint64(&mut self) -> Result<i64, ParseException> {
        match parse_uint64(&self.current_token) {
            Ok(result) => {
                self.next_token();
                Ok(result)
            }
            Err(e) => Err(self.integer_parse_exception(&e)),
        }
    }

    // port: TextFormat.Tokenizer#tryConsumeUInt64
    fn try_consume_uint64(&mut self) -> bool {
        self.consume_uint64().is_ok()
    }

    // port: TextFormat.Tokenizer#consumeDouble
    fn consume_double(&mut self) -> Result<f64, ParseException> {
        match lossy(&self.current_token).to_lowercase().as_str() {
            "-inf" | "-infinity" => {
                self.next_token();
                return Ok(f64::NEG_INFINITY);
            }
            "inf" | "infinity" => {
                self.next_token();
                return Ok(f64::INFINITY);
            }
            "nan" => {
                self.next_token();
                return Ok(f64::NAN);
            }
            _ => {}
        }
        match java_lang::parse_double(&JsString::from_units(self.current_token.clone())) {
            Ok(result) => {
                self.next_token();
                Ok(result)
            }
            Err(e) => Err(self.float_parse_exception(&e)),
        }
    }

    // port: TextFormat.Tokenizer#tryConsumeDouble
    fn try_consume_double(&mut self) -> bool {
        self.consume_double().is_ok()
    }

    // port: TextFormat.Tokenizer#consumeFloat
    fn consume_float(&mut self) -> Result<f32, ParseException> {
        match lossy(&self.current_token).to_lowercase().as_str() {
            "-inf" | "-inff" | "-infinity" | "-infinityf" => {
                self.next_token();
                return Ok(f32::NEG_INFINITY);
            }
            "inf" | "inff" | "infinity" | "infinityf" => {
                self.next_token();
                return Ok(f32::INFINITY);
            }
            "nan" | "nanf" => {
                self.next_token();
                return Ok(f32::NAN);
            }
            _ => {}
        }
        // Float.parseFloat accepts exactly the strings Double.parseDouble accepts; the value is
        // only used to skip unknown fields.
        match java_lang::parse_double(&JsString::from_units(self.current_token.clone())) {
            Ok(result) => {
                self.next_token();
                Ok(result as f32)
            }
            Err(e) => Err(self.float_parse_exception(&e)),
        }
    }

    // port: TextFormat.Tokenizer#tryConsumeFloat
    fn try_consume_float(&mut self) -> bool {
        self.consume_float().is_ok()
    }

    // port: TextFormat.Tokenizer#consumeBoolean
    fn consume_boolean(&mut self) -> Result<bool, ParseException> {
        if self.token_is("true")
            || self.token_is("True")
            || self.token_is("t")
            || self.token_is("1")
        {
            self.next_token();
            Ok(true)
        } else if self.token_is("false")
            || self.token_is("False")
            || self.token_is("f")
            || self.token_is("0")
        {
            self.next_token();
            Ok(false)
        } else {
            Err(self.parse_exception(&format!(
                "Expected \"true\" or \"false\". Found \"{}\".",
                lossy(&self.current_token)
            )))
        }
    }

    // port: TextFormat.Tokenizer#consumeString
    fn consume_string(&mut self) -> Result<String, ParseException> {
        // ByteString#toStringUtf8 decodes like `new String(bytes, UTF_8)`.
        let bytes = self.consume_byte_string()?;
        Ok(java_lang::utf_8::decode(&bytes).to_string_lossy())
    }

    // port: TextFormat.Tokenizer#consumeByteString()
    fn consume_byte_string(&mut self) -> Result<Vec<u8>, ParseException> {
        let mut list: Vec<u8> = Vec::new();
        self.consume_byte_string_into(&mut list)?;
        while self.current_token.first() == Some(&(b'\'' as u16))
            || self.current_token.first() == Some(&(b'"' as u16))
        {
            self.consume_byte_string_into(&mut list)?;
        }
        Ok(list)
    }

    // port: TextFormat.Tokenizer#tryConsumeByteString
    fn try_consume_byte_string(&mut self) -> bool {
        self.consume_byte_string().is_ok()
    }

    // port: TextFormat.Tokenizer#consumeByteString(List)
    fn consume_byte_string_into(&mut self, list: &mut Vec<u8>) -> Result<(), ParseException> {
        let quote = self.current_token.first().copied().unwrap_or(0);
        if quote != b'"' as u16 && quote != b'\'' as u16 {
            return Err(self.parse_exception("Expected string."));
        }

        if self.current_token.len() < 2 || *self.current_token.last().unwrap() != quote {
            return Err(self.parse_exception("String missing ending quote."));
        }

        let escaped = &self.current_token[1..self.current_token.len() - 1];
        match unescape_bytes(escaped) {
            Ok(result) => {
                self.next_token();
                list.extend(result);
                Ok(())
            }
            Err(InvalidEscapeSequenceException(message)) => Err(self.parse_exception(&message)),
        }
    }

    // port: TextFormat.Tokenizer#parseException
    fn parse_exception(&self, description: &str) -> ParseException {
        // Java's line and column are 0-based; ParseException's are 1-based.
        ParseException::new(self.line + 1, self.column + 1, description)
    }

    // port: TextFormat.Tokenizer#parseExceptionPreviousToken
    fn parse_exception_previous_token(&self, description: &str) -> ParseException {
        ParseException::new(
            self.previous_line + 1,
            self.previous_column + 1,
            description,
        )
    }

    // port: TextFormat.Tokenizer#integerParseException
    fn integer_parse_exception(&self, e: &NumberFormatException) -> ParseException {
        self.parse_exception(&format!(
            "Couldn't parse integer: {}",
            String::from_utf16_lossy(&e.message)
        ))
    }

    // port: TextFormat.Tokenizer#floatParseException
    fn float_parse_exception(&self, e: &NumberFormatException) -> ParseException {
        self.parse_exception(&format!(
            "Couldn't parse number: {}",
            String::from_utf16_lossy(&e.message)
        ))
    }
}

// port: TextFormat.Tokenizer#isAlphaUnder
fn is_alpha_under(c: u16) -> bool {
    (b'a' as u16..=b'z' as u16).contains(&c)
        || (b'A' as u16..=b'Z' as u16).contains(&c)
        || c == b'_' as u16
}

// port: TextFormat.Tokenizer#isDigitPlusMinus
fn is_digit_plus_minus(c: u16) -> bool {
    (b'0' as u16..=b'9' as u16).contains(&c) || c == b'+' as u16 || c == b'-' as u16
}

// port: TextFormat.Tokenizer#isWhitespace
fn is_whitespace(c: u16) -> bool {
    c == b' ' as u16 || c == 0x0c || c == b'\n' as u16 || c == b'\r' as u16 || c == b'\t' as u16
}

/// Recursion limit of the default parser (`TextFormat.Parser.Builder#recursionLimit`).
const RECURSION_LIMIT: i32 = 100;

/// One entry of the parser's unknown-field list (`TextFormat.Parser.UnknownField`).
struct UnknownField {
    message: String,
}

// port: TextFormat#merge(CharSequence,Message.Builder)
pub fn merge(input: &str, builder: &mut dyn Message) -> Result<(), ParseException> {
    let mut tokenizer = Tokenizer::new(input);
    let mut unknown_fields: Vec<UnknownField> = Vec::new();

    while !tokenizer.at_end() {
        merge_field(
            &mut tokenizer,
            builder,
            &mut unknown_fields,
            RECURSION_LIMIT,
        )?;
    }
    check_unknown_fields(&unknown_fields)
}

// port: TextFormat.Parser#checkUnknownFields
fn check_unknown_fields(unknown_fields: &[UnknownField]) -> Result<(), ParseException> {
    if unknown_fields.is_empty() {
        return Ok(());
    }

    let mut msg = String::from("Input contains unknown fields and/or extensions:");
    for field in unknown_fields {
        msg.push('\n');
        msg.push_str(&field.message);
    }

    // allowUnknownFields and allowUnknownExtensions are false in the default parser.
    let first_error_index = 0;
    let line_column: Vec<&str> = unknown_fields[first_error_index]
        .message
        .split(':')
        .collect();
    Err(ParseException::new(
        line_column[0].parse::<i32>().unwrap(),
        line_column[1].parse::<i32>().unwrap(),
        msg,
    ))
}

// port: TextFormat.Parser#mergeField
fn merge_field(
    tokenizer: &mut Tokenizer,
    target: &mut dyn Message,
    unknown_fields: &mut Vec<UnknownField>,
    recursion_limit: i32,
) -> Result<(), ParseException> {
    let mut field: Option<&'static FieldDescriptor> = None;

    // The type is never google.protobuf.Any.
    if tokenizer.try_consume("[") {
        // An extension.
        let mut name_builder = tokenizer.consume_identifier()?;
        while tokenizer.try_consume(".") {
            name_builder.push('.');
            name_builder.push_str(&tokenizer.consume_identifier()?);
        }
        let name = name_builder;

        // The empty extension registry knows no extensions.
        let message = format!(
            "{}:{}:\t{}.[{}]",
            tokenizer.get_previous_line() + 1,
            tokenizer.get_previous_column() + 1,
            target.full_name(),
            name
        );
        unknown_fields.push(UnknownField { message });

        tokenizer.consume("]")?;
    } else {
        let name = tokenizer.consume_identifier()?;
        field = target.find_field_by_name(&name);

        // Group names are expected to be capitalized as they appear in the .proto file; these
        // messages have no groups, so the lower-cased lookup never yields a field.

        if field.is_none() {
            let message = format!(
                "{}:{}:\t{}.{}",
                tokenizer.get_previous_line() + 1,
                tokenizer.get_previous_column() + 1,
                target.full_name(),
                name
            );
            unknown_fields.push(UnknownField { message });
        }
    }

    // Skips unknown fields.
    let Some(field) = field else {
        guess_field_type_and_skip(tokenizer, recursion_limit)?;
        return Ok(());
    };

    // Handle potential ':'.
    if field.is_message() {
        tokenizer.try_consume(":"); // optional
    } else {
        tokenizer.consume(":")?; // required
    }
    consume_field_values(tokenizer, target, field, unknown_fields, recursion_limit)?;

    // For historical reasons, fields may optionally be separated by commas or semicolons.
    if !tokenizer.try_consume(";") {
        tokenizer.try_consume(",");
    }
    Ok(())
}

// port: TextFormat.Parser#consumeFullTypeName
fn consume_full_type_name(tokenizer: &mut Tokenizer) -> Result<(), ParseException> {
    // If there is not a leading `[`, this is just a type name.
    if !tokenizer.try_consume("[") {
        tokenizer.consume_identifier()?;
        return Ok(());
    }

    // Otherwise, this is an extension or google.protobuf.Any type URL: we first consume the
    // domain or extension name, then the type name if any.
    tokenizer.consume_identifier()?;
    while tokenizer.try_consume(".") {
        tokenizer.consume_identifier()?;
    }
    if tokenizer.try_consume("/") {
        tokenizer.consume_identifier()?;
        while tokenizer.try_consume(".") {
            tokenizer.consume_identifier()?;
        }
    }
    tokenizer.consume("]")
}

// port: TextFormat.Parser#consumeFieldValues
fn consume_field_values(
    tokenizer: &mut Tokenizer,
    target: &mut dyn Message,
    field: &'static FieldDescriptor,
    unknown_fields: &mut Vec<UnknownField>,
    recursion_limit: i32,
) -> Result<(), ParseException> {
    // Support specifying repeated field values as a comma-separated list.
    // Ex."foo: [1, 2, 3]"
    if field.is_repeated() && tokenizer.try_consume("[") {
        if !tokenizer.try_consume("]") {
            // Allow "foo: []" to be treated as empty.
            loop {
                consume_field_value(tokenizer, target, field, unknown_fields, recursion_limit)?;
                if tokenizer.try_consume("]") {
                    // End of list.
                    break;
                }
                tokenizer.consume(",")?;
            }
        }
    } else {
        consume_field_value(tokenizer, target, field, unknown_fields, recursion_limit)?;
    }
    Ok(())
}

// port: TextFormat.Parser#consumeFieldValue
fn consume_field_value(
    tokenizer: &mut Tokenizer,
    target: &mut dyn Message,
    field: &'static FieldDescriptor,
    unknown_fields: &mut Vec<UnknownField>,
    recursion_limit: i32,
) -> Result<(), ParseException> {
    // singularOverwritePolicy is ALLOW_SINGULAR_OVERWRITES in the default parser.
    match field.kind {
        FieldKind::Message => {
            if recursion_limit < 1 {
                return Err(tokenizer.parse_exception("Message is nested too deep"));
            }
            let end_token = if tokenizer.try_consume("<") {
                ">"
            } else {
                tokenizer.consume("{")?;
                "}"
            };
            target.merge_message_field(field.number, &mut |sub_field: &mut dyn Message| {
                while !tokenizer.try_consume(end_token) {
                    if tokenizer.at_end() {
                        return Err(
                            tokenizer.parse_exception(&format!("Expected \"{end_token}\"."))
                        );
                    }
                    merge_field(tokenizer, sub_field, unknown_fields, recursion_limit - 1)?;
                }
                Ok(())
            })?;
        }
        FieldKind::Bool => {
            let value = tokenizer.consume_boolean()?;
            target.set_bool(field.number, value);
        }
        FieldKind::String => {
            let value = tokenizer.consume_string()?;
            target.set_string(field.number, value);
        }
        FieldKind::Enum(full_name, values) => {
            // proto2 enums are closed: findValueByNumber.
            let value;
            if tokenizer.looking_at_integer() {
                let number = tokenizer.consume_int32()?;
                if !values.iter().any(|(_, n)| *n == number) {
                    // allowUnknownEnumValues is false in the default parser.
                    return Err(tokenizer.parse_exception_previous_token(&format!(
                        "Enum type \"{full_name}\" has no value with number {number}."
                    )));
                }
                value = number;
            } else {
                let id = tokenizer.consume_identifier()?;
                match values.iter().find(|(n, _)| *n == id) {
                    Some((_, number)) => value = *number,
                    None => {
                        return Err(tokenizer.parse_exception_previous_token(&format!(
                            "Enum type \"{full_name}\" has no value named \"{id}\"."
                        )));
                    }
                }
            }
            target.set_enum(field.number, value);
        }
    }
    Ok(())
}

// port: TextFormat.Parser#skipField
fn skip_field(tokenizer: &mut Tokenizer, recursion_limit: i32) -> Result<(), ParseException> {
    consume_full_type_name(tokenizer)?;
    guess_field_type_and_skip(tokenizer, recursion_limit)?;

    // For historical reasons, fields may optionally be separated by commas or semicolons.
    if !tokenizer.try_consume(";") {
        tokenizer.try_consume(",");
    }
    Ok(())
}

// port: TextFormat.Parser#skipFieldMessage
fn skip_field_message(
    tokenizer: &mut Tokenizer,
    recursion_limit: i32,
) -> Result<(), ParseException> {
    let delimiter = if tokenizer.try_consume("<") {
        ">"
    } else {
        tokenizer.consume("{")?;
        "}"
    };
    while !tokenizer.looking_at(">") && !tokenizer.looking_at("}") {
        skip_field(tokenizer, recursion_limit)?;
    }
    tokenizer.consume(delimiter)
}

// port: TextFormat.Parser#skipFieldValue
fn skip_field_value(tokenizer: &mut Tokenizer) -> Result<(), ParseException> {
    if !tokenizer.try_consume_byte_string()
        && !tokenizer.try_consume_identifier() // includes enum & boolean
        && !tokenizer.try_consume_int64() // includes int32
        && !tokenizer.try_consume_uint64() // includes uint32
        && !tokenizer.try_consume_double()
        && !tokenizer.try_consume_float()
    {
        return Err(tokenizer.parse_exception(&format!(
            "Invalid field value: {}",
            lossy(&tokenizer.current_token)
        )));
    }
    Ok(())
}

// port: TextFormat.Parser#guessFieldTypeAndSkip
fn guess_field_type_and_skip(
    tokenizer: &mut Tokenizer,
    recursion_limit: i32,
) -> Result<(), ParseException> {
    let semicolon_consumed = tokenizer.try_consume(":");
    if tokenizer.looking_at("[") {
        // Short repeated field form. If a semicolon was consumed, it could be repeated scalars
        // or repeated messages. If not, it can only be repeated messages.
        skip_field_short_formed_repeated(tokenizer, semicolon_consumed, recursion_limit)?;
    } else if semicolon_consumed && !tokenizer.looking_at("{") && !tokenizer.looking_at("<") {
        skip_field_value(tokenizer)?;
    } else {
        if recursion_limit < 1 {
            return Err(tokenizer.parse_exception("Message is nested too deep"));
        }
        skip_field_message(tokenizer, recursion_limit - 1)?;
    }
    Ok(())
}

// port: TextFormat.Parser#skipFieldShortFormedRepeated
fn skip_field_short_formed_repeated(
    tokenizer: &mut Tokenizer,
    scalar_allowed: bool,
    recursion_limit: i32,
) -> Result<(), ParseException> {
    if !tokenizer.try_consume("[") || tokenizer.try_consume("]") {
        // Try skipping "[]".
        return Ok(());
    }

    loop {
        if tokenizer.looking_at("{") || tokenizer.looking_at("<") {
            // Try skipping message field inside a repeated message field.
            if recursion_limit < 1 {
                return Err(tokenizer.parse_exception("Message is nested too deep"));
            }
            skip_field_message(tokenizer, recursion_limit - 1)?;
        } else if scalar_allowed {
            // Try skipping scalar field inside a repeated field.
            skip_field_value(tokenizer)?;
        } else {
            return Err(tokenizer
                .parse_exception("Invalid repeated scalar field: missing \":\" before \"[\"."));
        }
        if tokenizer.try_consume("]") {
            break;
        }
        tokenizer.consume(",")?;
    }
    Ok(())
}

// port: TextFormat#unescapeBytes
fn unescape_bytes(char_string: &[u16]) -> Result<Vec<u8>, InvalidEscapeSequenceException> {
    // First convert the Java character sequence to UTF-8 bytes (ByteString#copyFromUtf8: lone
    // surrogates become '?').
    let input: Vec<u8> = java_lang::utf_8::encode(&JsString::from_units(char_string.to_vec()));
    let size = input.len();
    let mut result: Vec<u8> = Vec::with_capacity(size);
    let mut i = 0;
    while i < size {
        let mut c = input[i];
        if c == b'\\' {
            if i + 1 < size {
                i += 1;
                c = input[i];
                if is_octal(c) {
                    let mut code = digit_value(c);
                    if i + 1 < size && is_octal(input[i + 1]) {
                        i += 1;
                        code = code * 8 + digit_value(input[i]);
                    }
                    if i + 1 < size && is_octal(input[i + 1]) {
                        i += 1;
                        code = code * 8 + digit_value(input[i]);
                    }
                    result.push(code as u8);
                } else {
                    match c {
                        b'a' => result.push(0x07),
                        b'b' => result.push(0x08),
                        b'f' => result.push(0x0c),
                        b'n' => result.push(b'\n'),
                        b'r' => result.push(b'\r'),
                        b't' => result.push(b'\t'),
                        b'v' => result.push(0x0b),
                        b'\\' => result.push(b'\\'),
                        b'\'' => result.push(b'\''),
                        b'"' => result.push(b'"'),
                        b'?' => result.push(b'?'),
                        b'x' => {
                            // hex escape
                            let mut code;
                            if i + 1 < size && is_hex(input[i + 1]) {
                                i += 1;
                                code = digit_value(input[i]);
                            } else {
                                return Err(InvalidEscapeSequenceException(
                                    "Invalid escape sequence: '\\x' with no digits".into(),
                                ));
                            }
                            if i + 1 < size && is_hex(input[i + 1]) {
                                i += 1;
                                code = code * 16 + digit_value(input[i]);
                            }
                            result.push(code as u8);
                        }
                        b'u' => {
                            // Unicode escape
                            i += 1;
                            if i + 3 < size
                                && is_hex(input[i])
                                && is_hex(input[i + 1])
                                && is_hex(input[i + 2])
                                && is_hex(input[i + 3])
                            {
                                let ch = (digit_value(input[i]) << 12
                                    | digit_value(input[i + 1]) << 8
                                    | digit_value(input[i + 2]) << 4
                                    | digit_value(input[i + 3]))
                                    as u16;
                                if (0xd800..=0xdfff).contains(&ch) {
                                    return Err(InvalidEscapeSequenceException(
                                        "Invalid escape sequence: '\\u' refers to a surrogate"
                                            .into(),
                                    ));
                                }
                                let ch_utf8 =
                                    java_lang::utf_8::encode(&JsString::from_units(vec![ch]));
                                result.extend(ch_utf8);
                                i += 3;
                            } else {
                                return Err(InvalidEscapeSequenceException(
                                    "Invalid escape sequence: '\\u' with too few hex chars".into(),
                                ));
                            }
                        }
                        b'U' => {
                            // Unicode escape
                            i += 1;
                            if i + 7 >= size {
                                return Err(InvalidEscapeSequenceException(
                                    "Invalid escape sequence: '\\U' with too few hex chars".into(),
                                ));
                            }
                            let mut codepoint: i32 = 0;
                            for &b in &input[i..i + 8] {
                                if !is_hex(b) {
                                    return Err(InvalidEscapeSequenceException(
                                        "Invalid escape sequence: '\\U' with too few hex chars"
                                            .into(),
                                    ));
                                }
                                codepoint = (codepoint << 4) | digit_value(b);
                            }
                            let digits = String::from_utf8_lossy(&input[i..i + 8]).into_owned();
                            // Character#isValidCodePoint
                            if !(0..=0x10ffff).contains(&codepoint) {
                                return Err(InvalidEscapeSequenceException(format!(
                                    "Invalid escape sequence: '\\U{digits}' is not a valid code point value"
                                )));
                            }
                            // Character.UnicodeBlock LOW_SURROGATES, HIGH_SURROGATES and
                            // HIGH_PRIVATE_USE_SURROGATES together cover U+D800..U+DFFF.
                            if (0xd800..=0xdfff).contains(&codepoint) {
                                return Err(InvalidEscapeSequenceException(format!(
                                    "Invalid escape sequence: '\\U{digits}' refers to a surrogate code unit"
                                )));
                            }
                            let ch = char::from_u32(codepoint as u32).unwrap();
                            let mut buf = [0u8; 4];
                            result.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            i += 7;
                        }
                        _ => {
                            // Java appends `(char) c` of the signed byte.
                            let ch = (c as i8 as i16 as u16) as u32;
                            let ch = char::from_u32(ch).unwrap_or(char::REPLACEMENT_CHARACTER);
                            return Err(InvalidEscapeSequenceException(format!(
                                "Invalid escape sequence: '\\{ch}'"
                            )));
                        }
                    }
                }
            } else {
                return Err(InvalidEscapeSequenceException(
                    "Invalid escape sequence: '\\' at end of string.".into(),
                ));
            }
        } else {
            result.push(c);
        }
        i += 1;
    }
    Ok(result)
}

// port: TextFormat#isOctal
fn is_octal(c: u8) -> bool {
    (b'0'..=b'7').contains(&c)
}

// port: TextFormat#isHex
fn is_hex(c: u8) -> bool {
    c.is_ascii_digit() || (b'a'..=b'f').contains(&c) || (b'A'..=b'F').contains(&c)
}

// port: TextFormat#digitValue
fn digit_value(c: u8) -> i32 {
    if c.is_ascii_digit() {
        (c - b'0') as i32
    } else if c.is_ascii_lowercase() {
        (c - b'a') as i32 + 10
    } else {
        (c as i32) - (b'A' as i32) + 10
    }
}

// port: TextFormat#parseInt32
fn parse_int32(text: &[u16]) -> Result<i32, NumberFormatException> {
    Ok(parse_integer(text, true, false)? as i32)
}

// port: TextFormat#parseInt64
fn parse_int64(text: &[u16]) -> Result<i64, NumberFormatException> {
    parse_integer(text, true, true)
}

// port: TextFormat#parseUInt64
fn parse_uint64(text: &[u16]) -> Result<i64, NumberFormatException> {
    parse_integer(text, false, true)
}

fn nfe(message: String) -> NumberFormatException {
    NumberFormatException {
        message: message.encode_utf16().collect(),
    }
}

// port: TextFormat#parseInteger
fn parse_integer(
    text: &[u16],
    is_signed: bool,
    is_long: bool,
) -> Result<i64, NumberFormatException> {
    let text_str = lossy(text);
    let mut pos = 0;

    let mut negative = false;
    if text.first() == Some(&(b'-' as u16)) {
        if !is_signed {
            return Err(nfe(format!("Number must be positive: {text_str}")));
        }
        pos += 1;
        negative = true;
    }

    let mut radix = 10;
    if text[pos..].starts_with(&[b'0' as u16, b'x' as u16]) {
        pos += 2;
        radix = 16;
    } else if text[pos..].starts_with(&[b'0' as u16]) {
        radix = 8;
    }

    let number_text = &text[pos..];

    let mut result: i64;
    if number_text.len() < 16 {
        // Can safely assume no overflow.
        result = parse_long(number_text, radix)?;
        if negative {
            result = result.wrapping_neg();
        }

        // Check bounds.
        // No need to check for 64-bit numbers since they'd have to be 16 chars or longer to
        // overflow.
        if !is_long {
            if is_signed {
                if result > i32::MAX as i64 || result < i32::MIN as i64 {
                    return Err(nfe(format!(
                        "Number out of range for 32-bit signed integer: {text_str}"
                    )));
                }
            } else if !(0..(1i64 << 32)).contains(&result) {
                return Err(nfe(format!(
                    "Number out of range for 32-bit unsigned integer: {text_str}"
                )));
            }
        }
    } else {
        let mut big_value =
            java_lang::parse_big_integer(&JsString::from_units(number_text.to_vec()), radix as i32)
                .map_err(|e| NumberFormatException {
                    message: e.message().as_units().to_vec(),
                })?;
        if negative {
            big_value = -big_value;
        }

        // Check bounds (BigInteger#bitLength).
        let bit_length = big_integer_bit_length(&big_value);
        if !is_long {
            if is_signed {
                if bit_length > 31 {
                    return Err(nfe(format!(
                        "Number out of range for 32-bit signed integer: {text_str}"
                    )));
                }
            } else if bit_length > 32 {
                return Err(nfe(format!(
                    "Number out of range for 32-bit unsigned integer: {text_str}"
                )));
            }
        } else if is_signed {
            if bit_length > 63 {
                return Err(nfe(format!(
                    "Number out of range for 64-bit signed integer: {text_str}"
                )));
            }
        } else if bit_length > 64 {
            return Err(nfe(format!(
                "Number out of range for 64-bit unsigned integer: {text_str}"
            )));
        }

        // BigInteger#longValue: the low 64 bits in two's complement.
        let (sign, digits) = big_value.to_u64_digits();
        let low = digits.first().copied().unwrap_or(0);
        result = if sign == num_bigint::Sign::Minus {
            (low as i64).wrapping_neg()
        } else {
            low as i64
        };
    }

    Ok(result)
}

// port: BigInteger#bitLength
fn big_integer_bit_length(value: &num_bigint::BigInt) -> u64 {
    if value.sign() == num_bigint::Sign::Minus {
        // For negative numbers bitLength is that of (-value - 1).
        let magnitude: num_bigint::BigInt = -value - 1i32;
        magnitude.bits()
    } else {
        value.bits()
    }
}

// Long#parseLong, ported from OpenJDK (GPL-2.0 with the Classpath exception), is in its own file.
#[path = "text_format_jdk.rs"]
mod jdk;
use jdk::parse_long;
