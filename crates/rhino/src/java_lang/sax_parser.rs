/*
 * Copyright (c) 2000, 2019, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
/*
 * Copyright (c) 2003, 2021, Oracle and/or its affiliates. All rights reserved.
 */
/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements.  See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License.  You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * Copyright (c) 2003, 2022, Oracle and/or its affiliates. All rights reserved.
 */
/*
 * Copyright (c) 2003, 2020, Oracle and/or its affiliates. All rights reserved.
 */
/*
 * Copyright (c) 2003, 2018, Oracle and/or its affiliates. All rights reserved.
 */
/*
 * Copyright (c) 2000, 2020, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
/*
 * reserved comment block
 * DO NOT REMOVE OR ALTER!
 */
/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements.  See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License.  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * Copyright (c) 2017, 2018, Oracle and/or its affiliates. All rights reserved.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1):
//   java.xml/com/sun/org/apache/xerces/internal/impl/XMLDTDScannerImpl.java,
//   java.xml/com/sun/org/apache/xerces/internal/impl/XMLDocumentFragmentScannerImpl.java,
//   java.xml/com/sun/org/apache/xerces/internal/impl/XMLDocumentScannerImpl.java,
//   java.xml/com/sun/org/apache/xerces/internal/impl/XMLEntityScanner.java,
//   java.xml/com/sun/org/apache/xerces/internal/impl/XMLScanner.java,
//   java.xml/com/sun/org/apache/xerces/internal/impl/dtd/XMLDTDValidator.java,
//   java.xml/com/sun/org/apache/xerces/internal/impl/io/UTF8Reader.java,
//   java.xml/org/xml/sax/Attributes.java, java.xml/org/xml/sax/ContentHandler.java,
//   java.xml/org/xml/sax/SAXParseException.java, java.xml/org/xml/sax/XMLReader.java.

//! Port of the behaviour of the JDK 21 SAX parser (`javax.xml.parsers.SAXParser`, the internal
//! Xerces `XMLDocumentScannerImpl` / `XMLDocumentFragmentScannerImpl` / `XMLEntityScanner` /
//! `XMLDTDScannerImpl` / `UTF8Reader` / `AbstractSAXParser`) in the configuration
//! `XtbMessageBundle#createSAXParser` sets up: not validating, not namespace aware, external
//! general entities, external parameter entities and the external DTD not loaded.
//!
//! What a `ContentHandler` sees is reproduced: the `startElement` / `endElement` /
//! `characters` / `processingInstruction` / `skippedEntity` calls, including how Xerces splits
//! character data into `characters` calls (`SCANNER_STATE_CHARACTER_DATA`, `scanContent`,
//! `normalizeNewlines`, one call per built-in entity or character reference, one per CDATA
//! section) and the fatal errors with their `XMLMessages.properties` texts.
//!
//! Each entity is scanned as one buffer: the refills of the JDK entity scanner's 8K character
//! buffer are not emulated, so a run of character data crossing a refill boundary may be split
//! into different `characters` calls than in Java (the concatenated text is the same).
//! Of the internal DTD subset, `<!ENTITY` declarations (internal general entities) and
//! `<!ATTLIST` declarations (default values, and the value normalization of non-CDATA types)
//! are applied; other declarations are skipped. Only the UTF-8, UTF-16 (with byte order mark),
//! US-ASCII and ISO-8859-1 encodings are read.

use std::fmt;

use crate::fx_hash::{IndexMap, IndexSet};

use super::xml_char;
use crate::js_string::JsString;

/// The attributes of an element, in document order.
#[derive(Clone, Debug, Default)]
pub struct Attributes {
    names: Vec<String>,
    values: Vec<JsString>,
}

impl Attributes {
    /// Look up an attribute's value by XML qualified (prefixed) name.
    // port: Attributes#getValue(String)
    pub fn get_value(&self, q_name: &str) -> Option<&JsString> {
        self.names
            .iter()
            .position(|n| n == q_name)
            .map(|i| &self.values[i])
    }

    /// Return the number of attributes in the list.
    // port: Attributes#getLength
    pub fn get_length(&self) -> usize {
        self.names.len()
    }

    /// Look up an attribute's XML qualified (prefixed) name by index.
    // port: Attributes#getQName
    pub fn get_q_name(&self, index: usize) -> &str {
        &self.names[index]
    }
}

/// Receive notification of the logical content of a document (`org.xml.sax.ContentHandler`).
/// The parser is not namespace aware, so `uri` and `local_name` are always empty.
pub trait ContentHandler {
    /// Receive an object for locating the origin of SAX document events. Rust-only: the `Locator`
    /// argument is not modelled (the one implementation ignores it).
    // port: ContentHandler#setDocumentLocator
    fn set_document_locator(&mut self);
    // port: ContentHandler#startDocument
    fn start_document(&mut self);
    // port: ContentHandler#endDocument
    fn end_document(&mut self);
    /// Never called: the parser is not namespace aware.
    // port: ContentHandler#startPrefixMapping
    fn start_prefix_mapping(&mut self, prefix: &str, uri: &str);
    /// Never called: the parser is not namespace aware.
    // port: ContentHandler#endPrefixMapping
    fn end_prefix_mapping(&mut self, prefix: &str);
    // port: ContentHandler#startElement
    fn start_element(&mut self, uri: &str, local_name: &str, q_name: &str, atts: &Attributes);
    // port: ContentHandler#endElement
    fn end_element(&mut self, uri: &str, local_name: &str, q_name: &str);
    // port: ContentHandler#characters
    fn characters(&mut self, ch: &[u16]);
    // port: ContentHandler#ignorableWhitespace
    fn ignorable_whitespace(&mut self, ch: &[u16]);
    // port: ContentHandler#processingInstruction
    fn processing_instruction(&mut self, target: &str, data: &JsString);
    // port: ContentHandler#skippedEntity
    fn skipped_entity(&mut self, name: &str);
}

/// `org.xml.sax.SAXParseException`, as reported for a fatal error of the parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaxParseException {
    pub line_number: i32,
    pub column_number: i32,
    pub message: String,
}

impl fmt::Display for SaxParseException {
    // port: SAXParseException#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("org.xml.sax.SAXParseException")?;
        if self.line_number != -1 {
            write!(f, "; lineNumber: {}", self.line_number)?;
        }
        if self.column_number != -1 {
            write!(f, "; columnNumber: {}", self.column_number)?;
        }
        write!(f, "; {}", self.message)
    }
}

impl std::error::Error for SaxParseException {}

/// Parses `bytes` as an XML document and reports its content to `handler`.
// port: XMLReader#parse(InputSource)
pub fn parse(bytes: &[u8], handler: &mut dyn ContentHandler) -> Result<(), SaxParseException> {
    let result = parse_document(bytes, handler);
    if let Err(e) = &result {
        // The default ErrorHandler of Xerces (DefaultErrorHandler#printError) prints fatal errors
        // to System.err before they are thrown.
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "[Fatal Error] :{}:{}: {}",
            e.line_number,
            e.column_number,
            e.message
        );
    }
    result
}

fn parse_document(bytes: &[u8], handler: &mut dyn ContentHandler) -> Result<(), SaxParseException> {
    let text = decode_document(bytes)?;
    let mut scanner = Scanner {
        entities: vec![Entity {
            ch: text,
            position: 0,
            name: None,
            element_depth: 0,
        }],
        handler,
        general_entities: IndexMap::<_, _>::default(),
        unparsed_entities: IndexSet::<_>::default(),
        attribute_decls: IndexMap::<_, _>::default(),
        has_external_dtd: false,
        standalone: false,
        element_stack: Vec::new(),
        empty_element: false,
        state: ScannerState::Content,
    };
    // XMLDocumentScannerImpl#startEntity of the document entity -> AbstractSAXParser#startDocument
    scanner.handler.set_document_locator();
    scanner.handler.start_document();
    scanner.scan_document()?;
    // XMLDocumentScannerImpl#endEntity of the document entity -> AbstractSAXParser#endDocument
    scanner.handler.end_document();
    Ok(())
}

// --- Decoding (XMLEntityManager#createReader, UTF8Reader) -------------------------------------

fn fatal_at_start(message: String) -> SaxParseException {
    SaxParseException {
        line_number: 1,
        column_number: 1,
        message,
    }
}

/// Decodes the document bytes to UTF-16 and normalizes its newlines (`#xD#xA` and `#xD` become
/// `#xA`, as `XMLEntityScanner#normalizeNewlines` does for the external document entity).
fn decode_document(bytes: &[u8]) -> Result<Vec<u16>, SaxParseException> {
    let units = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        decode_utf8(&bytes[3..])?
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        bytes[2..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
            .collect()
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        bytes[2..]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], *c.get(1).unwrap_or(&0)]))
            .collect()
    } else {
        match declared_encoding(bytes).map(|e| e.to_ascii_uppercase()) {
            None => decode_utf8(bytes)?,
            Some(e) if e == "UTF-8" || e == "UTF8" => decode_utf8(bytes)?,
            Some(e) if e == "ISO-8859-1" || e == "ISO8859_1" || e == "LATIN1" => {
                bytes.iter().map(|&b| b as u16).collect()
            }
            Some(e) if e == "US-ASCII" || e == "ASCII" => {
                let mut out = Vec::with_capacity(bytes.len());
                for &b in bytes {
                    if b >= 0x80 {
                        return Err(fatal_at_start(format!(
                            "Byte \"{}\" is not a member of the (7-bit) ASCII character set.",
                            b as i8
                        )));
                    }
                    out.push(b as u16);
                }
                out
            }
            Some(_) => {
                let e = declared_encoding(bytes).unwrap_or_default();
                return Err(fatal_at_start(format!("Invalid encoding name \"{e}\".")));
            }
        }
    };
    let mut out = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let c = units[i];
        if c == '\r' as u16 {
            out.push('\n' as u16);
            if units.get(i + 1) == Some(&('\n' as u16)) {
                i += 1;
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    Ok(out)
}

/// The value of the encoding pseudo-attribute of an ASCII-compatible XML declaration.
fn declared_encoding(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"<?xml") {
        return None;
    }
    let end = bytes.windows(2).position(|w| w == b"?>")?;
    let decl = std::str::from_utf8(&bytes[..end]).ok()?;
    let at = decl.find("encoding")?;
    let rest = decl[at + "encoding".len()..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value = &rest[1..];
    Some(value[..value.find(quote)?].to_string())
}

/// A malformed byte sequence, located at the characters decoded before it (the JDK reports
/// the position its scanner had reached when the reader failed; this approximates it).
fn decode_error(decoded: &[u16], message: String) -> SaxParseException {
    let mut line = 1;
    let mut column = 0;
    let mut i = 0;
    while i < decoded.len() {
        let c = decoded[i];
        if c == '\n' as u16 || c == '\r' as u16 {
            if c == '\r' as u16 && decoded.get(i + 1) == Some(&('\n' as u16)) {
                i += 1;
            }
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
        i += 1;
    }
    SaxParseException {
        line_number: line,
        column_number: column.max(1),
        message,
    }
}

/// `UTF8Reader#read`: decodes UTF-8, reporting the reader's malformed byte sequence errors.
// port: UTF8Reader#read
fn decode_utf8(bytes: &[u8]) -> Result<Vec<u16>, SaxParseException> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as u32;
        if b0 < 0x80 {
            out.push(b0 as u16);
            i += 1;
        } else if (b0 & 0xE0) == 0xC0 && (b0 & 0x1E) != 0 {
            // UTF-8:   [110y yyyy] [10xx xxxx]
            // Unicode: [0000 0yyy] [yyxx xxxx]
            let Some(&b1) = bytes.get(i + 1) else {
                return Err(decode_error(
                    &out,
                    "Expected byte 2 of 2-byte UTF-8 sequence.".to_string(),
                ));
            };
            let b1 = b1 as u32;
            if (b1 & 0xC0) != 0x80 {
                return Err(decode_error(
                    &out,
                    "Invalid byte 2 of 2-byte UTF-8 sequence.".to_string(),
                ));
            }
            out.push((((b0 << 6) & 0x07C0) | (b1 & 0x003F)) as u16);
            i += 2;
        } else if (b0 & 0xF0) == 0xE0 {
            // UTF-8:   [1110 zzzz] [10yy yyyy] [10xx xxxx]
            // Unicode: [zzzz yyyy] [yyxx xxxx]
            let Some(&b1) = bytes.get(i + 1) else {
                return Err(decode_error(
                    &out,
                    "Expected byte 2 of 3-byte UTF-8 sequence.".to_string(),
                ));
            };
            let b1 = b1 as u32;
            if (b1 & 0xC0) != 0x80
                || (b0 == 0xED && b1 >= 0xA0)
                || ((b0 & 0x0F) == 0 && (b1 & 0x20) == 0)
            {
                return Err(decode_error(
                    &out,
                    "Invalid byte 2 of 3-byte UTF-8 sequence.".to_string(),
                ));
            }
            let Some(&b2) = bytes.get(i + 2) else {
                return Err(decode_error(
                    &out,
                    "Expected byte 3 of 3-byte UTF-8 sequence.".to_string(),
                ));
            };
            let b2 = b2 as u32;
            if (b2 & 0xC0) != 0x80 {
                return Err(decode_error(
                    &out,
                    "Invalid byte 3 of 3-byte UTF-8 sequence.".to_string(),
                ));
            }
            out.push((((b0 << 12) & 0xF000) | ((b1 << 6) & 0x0FC0) | (b2 & 0x003F)) as u16);
            i += 3;
        } else if (b0 & 0xF8) == 0xF0 {
            // UTF-8:   [1111 0uuu] [10uu zzzz] [10yy yyyy] [10xx xxxx]*
            // Unicode: [1101 10ww] [wwzz zzyy] (high surrogate)
            //          [1101 11yy] [yyxx xxxx] (low surrogate)
            //          * uuuuu = wwww + 1
            let Some(&b1) = bytes.get(i + 1) else {
                return Err(decode_error(
                    &out,
                    "Expected byte 2 of 4-byte UTF-8 sequence.".to_string(),
                ));
            };
            let b1 = b1 as u32;
            if (b1 & 0xC0) != 0x80 || ((b1 & 0x30) == 0 && (b0 & 0x07) == 0) {
                return Err(decode_error(
                    &out,
                    "Invalid byte 2 of 4-byte UTF-8 sequence.".to_string(),
                ));
            }
            let Some(&b2) = bytes.get(i + 2) else {
                return Err(decode_error(
                    &out,
                    "Expected byte 3 of 4-byte UTF-8 sequence.".to_string(),
                ));
            };
            let b2 = b2 as u32;
            if (b2 & 0xC0) != 0x80 {
                return Err(decode_error(
                    &out,
                    "Invalid byte 3 of 4-byte UTF-8 sequence.".to_string(),
                ));
            }
            let Some(&b3) = bytes.get(i + 3) else {
                return Err(decode_error(
                    &out,
                    "Expected byte 4 of 4-byte UTF-8 sequence.".to_string(),
                ));
            };
            let b3 = b3 as u32;
            if (b3 & 0xC0) != 0x80 {
                return Err(decode_error(
                    &out,
                    "Invalid byte 4 of 4-byte UTF-8 sequence.".to_string(),
                ));
            }
            let uuuuu = ((b0 << 2) & 0x001C) | ((b1 >> 4) & 0x0003);
            if uuuuu > 0x10 {
                return Err(decode_error(
                    &out,
                    format!(
                        "High surrogate bits in UTF-8 sequence must not exceed 0x10 but found \
                         0x{uuuuu:x}."
                    ),
                ));
            }
            let wwww = uuuuu - 1;
            let hs = 0xD800 | ((wwww << 6) & 0x03C0) | ((b1 << 2) & 0x003C) | ((b2 >> 4) & 0x0003);
            let ls = 0xDC00 | ((b2 << 6) & 0x03C0) | (b3 & 0x003F);
            out.push(hs as u16);
            out.push(ls as u16);
            i += 4;
        } else {
            return Err(decode_error(
                &out,
                "Invalid byte 1 of 1-byte UTF-8 sequence.".to_string(),
            ));
        }
    }
    Ok(out)
}

// --- Scanning --------------------------------------------------------------------------------

struct Entity {
    ch: Vec<u16>,
    position: usize,
    /// `None` for the document entity.
    name: Option<String>,
    /// Depth of the element stack when the entity was started.
    element_depth: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScannerState {
    Content,
    StartOfMarkup,
    StartElementTag,
    EndElementTag,
    CharacterData,
    Reference,
    Cdata,
    Comment,
    Pi,
}

const LT: i32 = '<' as i32;
const AMP: i32 = '&' as i32;
const RBRACKET: i32 = ']' as i32;
const CR: i32 = '\r' as i32;
const LF: i32 = '\n' as i32;

struct Scanner<'h> {
    entities: Vec<Entity>,
    handler: &'h mut dyn ContentHandler,
    /// Declared general entities: `Some(replacement text)` for internal ones, `None` for
    /// external ones (never read: external general entities are disabled).
    general_entities: IndexMap<String, Option<Vec<u16>>>,
    unparsed_entities: IndexSet<String>,
    /// `<!ATTLIST` declarations by element and attribute name; the first declaration of an
    /// attribute is binding.
    attribute_decls: IndexMap<String, IndexMap<String, AttributeDecl>>,
    has_external_dtd: bool,
    standalone: bool,
    element_stack: Vec<String>,
    empty_element: bool,
    state: ScannerState,
}

type ScanResult<T> = Result<T, SaxParseException>;

impl Scanner<'_> {
    // -- XMLErrorReporter / Locator

    fn report_fatal_error(&self, message: String) -> SaxParseException {
        let document = &self.entities[0];
        let mut line = 1;
        let mut column = 1;
        for &c in &document.ch[..document.position] {
            if c == '\n' as u16 {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        SaxParseException {
            line_number: line,
            column_number: column,
            message,
        }
    }

    // -- XMLEntityScanner

    fn current(&self) -> &Entity {
        self.entities.last().unwrap()
    }

    fn current_mut(&mut self) -> &mut Entity {
        self.entities.last_mut().unwrap()
    }

    /// The end of an internal entity's replacement text: `XMLEntityManager#endEntity`.
    fn end_entity(&mut self) -> ScanResult<()> {
        let entity = self.entities.pop().unwrap();
        if self.element_stack.len() > entity.element_depth {
            let name = self.element_stack.last().unwrap().clone();
            return Err(self.report_fatal_error(format!(
                "The element \"{name}\" must start and end within the same entity."
            )));
        }
        Ok(())
    }

    /// Ends every finished internal entity; true when the document entity is at its end.
    fn load(&mut self) -> ScanResult<bool> {
        while self.current().position == self.current().ch.len() {
            if self.entities.len() == 1 {
                return Ok(true);
            }
            self.end_entity()?;
        }
        Ok(false)
    }

    fn char_at(&self, i: usize) -> i32 {
        self.current().ch[i] as i32
    }

    // port: XMLEntityScanner#peekChar
    fn peek_char(&mut self) -> ScanResult<i32> {
        if self.load()? {
            return Ok(-1);
        }
        Ok(self.char_at(self.current().position))
    }

    // port: XMLEntityScanner#scanChar
    fn scan_char(&mut self) -> ScanResult<i32> {
        if self.load()? {
            return Ok(-1);
        }
        let c = self.char_at(self.current().position);
        self.current_mut().position += 1;
        Ok(c)
    }

    // port: XMLEntityScanner#skipChar
    fn skip_char(&mut self, c: char) -> ScanResult<bool> {
        if self.load()? {
            return Ok(false);
        }
        if self.char_at(self.current().position) == c as i32 {
            self.current_mut().position += 1;
            return Ok(true);
        }
        Ok(false)
    }

    // port: XMLEntityScanner#skipString
    fn skip_string(&mut self, s: &str) -> ScanResult<bool> {
        if self.load()? {
            return Ok(false);
        }
        let entity = self.current();
        let units: Vec<u16> = s.encode_utf16().collect();
        if entity.ch[entity.position..].starts_with(&units) {
            self.current_mut().position += units.len();
            return Ok(true);
        }
        Ok(false)
    }

    // port: XMLEntityScanner#skipSpaces
    fn skip_spaces(&mut self) -> ScanResult<bool> {
        let mut skipped = false;
        loop {
            if self.load()? {
                return Ok(skipped);
            }
            let c = self.char_at(self.current().position);
            if !xml_char::is_space(c) {
                return Ok(skipped);
            }
            self.current_mut().position += 1;
            skipped = true;
        }
    }

    // port: XMLEntityScanner#scanName
    fn scan_name(&mut self) -> ScanResult<Option<String>> {
        if self.load()? {
            return Ok(None);
        }
        let start = self.current().position;
        let c = self.char_at(start);
        if !xml_char::is_name_start(c) {
            return Ok(None);
        }
        let mut end = start + 1;
        while end < self.current().ch.len() && xml_char::is_name(self.char_at(end)) {
            end += 1;
        }
        let name = String::from_utf16_lossy(&self.current().ch[start..end]);
        self.current_mut().position = end;
        Ok(Some(name))
    }

    /// `XMLEntityScanner#normalizeNewlines` on an already normalized buffer: consumes a run of
    /// leading newlines; returns true (with the run in `buffer`) when the run reaches the last
    /// character of the buffer, as the JDK scanner does before it loads more characters.
    // port: XMLEntityScanner#normalizeNewlines
    fn normalize_newlines(&mut self, buffer: &mut Vec<u16>, offset: &mut usize) -> bool {
        let entity = self.entities.last_mut().unwrap();
        *offset = entity.position;
        let count = entity.ch.len();
        let c = entity.ch[*offset] as i32;
        if c == LF || c == CR {
            loop {
                let c = entity.ch[entity.position] as i32;
                entity.position += 1;
                if c == LF || c == CR {
                    if entity.position == count {
                        break;
                    }
                } else {
                    entity.position -= 1;
                    break;
                }
                if entity.position >= count - 1 {
                    break;
                }
            }
            if entity.position == count - 1 {
                buffer.extend_from_slice(&entity.ch[*offset..entity.position]);
                return true;
            }
        }
        false
    }

    /// Scans a range of content data, appending it to `content`; returns the next character
    /// (not consumed), or -1 at the end of the current entity.
    // port: XMLEntityScanner#scanContent
    fn entity_scan_content(&mut self, content: &mut Vec<u16>) -> ScanResult<i32> {
        // load more characters, if needed
        if self.current().position == self.current().ch.len() && self.load()? {
            return Ok(-1);
        }

        // normalize newlines
        let mut offset = 0;
        if self.normalize_newlines(content, &mut offset) {
            return Ok(-1);
        }

        let entity = self.entities.last_mut().unwrap();
        while entity.position < entity.ch.len() {
            let c = entity.ch[entity.position] as i32;
            entity.position += 1;
            if !xml_char::is_content(c) {
                entity.position -= 1;
                break;
            }
        }
        content.extend_from_slice(&entity.ch[offset..entity.position]);
        // return next character
        if entity.position != entity.ch.len() {
            Ok(entity.ch[entity.position] as i32)
        } else {
            Ok(-1)
        }
    }

    // -- XMLDocumentFragmentScannerImpl

    // port: XMLDocumentFragmentScannerImpl#scanContent
    fn scan_content(&mut self, content: &mut Vec<u16>) -> ScanResult<i32> {
        let mut c = self.entity_scan_content(content)?;
        if c == CR {
            // happens when there is the character reference &#13;
            self.scan_char()?;
            content.push(c as u16);
            c = -1;
        } else if c == RBRACKET {
            content.push(self.scan_char()? as u16);
            // We work on a single character basis to handle cases such as:
            // ']]]>' which we might otherwise miss.
            if self.skip_char(']')? {
                content.push(']' as u16);
                while self.skip_char(']')? {
                    content.push(']' as u16);
                }
                if self.skip_char('>')? {
                    return Err(self.report_fatal_error(
                        "The character sequence \"]]>\" must not appear in content unless used \
                         to mark the end of a CDATA section."
                            .into(),
                    ));
                }
            }
            c = -1;
        }
        Ok(c)
    }

    /// `AbstractSAXParser#characters`: empty character data is not reported.
    fn characters(&mut self, text: &[u16]) {
        if text.is_empty() {
            return;
        }
        self.handler.characters(text);
    }

    // port: XMLDocumentScannerImpl#scanDocument
    fn scan_document(&mut self) -> ScanResult<()> {
        self.scan_xml_decl()?;
        // prolog
        let mut seen_doctype = false;
        loop {
            self.skip_spaces()?;
            match self.peek_char()? {
                -1 => return Err(self.report_fatal_error("Premature end of file.".into())),
                LT => {
                    self.scan_char()?;
                    if self.skip_char('?')? {
                        self.scan_pi()?;
                    } else if self.skip_string("!--")? {
                        self.scan_comment()?;
                    } else if !seen_doctype && self.skip_string("!DOCTYPE")? {
                        seen_doctype = true;
                        self.scan_doctype_decl()?;
                    } else if xml_char::is_name_start(self.peek_char()?) {
                        break;
                    } else {
                        return Err(self.report_fatal_error(
                            "The markup in the document preceding the root element must be \
                             well-formed."
                                .into(),
                        ));
                    }
                }
                _ => {
                    return Err(self.report_fatal_error("Content is not allowed in prolog.".into()));
                }
            }
        }
        // root element
        self.state = ScannerState::StartElementTag;
        self.scan_element_content()?;
        // trailing misc
        loop {
            self.skip_spaces()?;
            match self.peek_char()? {
                -1 => return Ok(()),
                LT => {
                    self.scan_char()?;
                    if self.skip_char('?')? {
                        self.scan_pi()?;
                    } else if self.skip_string("!--")? {
                        self.scan_comment()?;
                    } else {
                        return Err(self.report_fatal_error(
                            "The markup in the document following the root element must be \
                             well-formed."
                                .into(),
                        ));
                    }
                }
                _ => {
                    return Err(self
                        .report_fatal_error("Content is not allowed in trailing section.".into()));
                }
            }
        }
    }

    // port: XMLDocumentScannerImpl#scanXMLDeclOrTextDecl
    fn scan_xml_decl(&mut self) -> ScanResult<()> {
        let entity = self.current();
        let starts = entity.ch.starts_with(&[0x3C, 0x3F, 0x78, 0x6D, 0x6C])
            && entity
                .ch
                .get(5)
                .is_some_and(|&c| xml_char::is_space(c as i32));
        if !starts {
            return Ok(());
        }
        self.current_mut().position = 5;
        let mut version = None;
        let mut first = true;
        loop {
            let had_space = self.skip_spaces()?;
            if self.skip_string("?>")? {
                break;
            }
            let Some(name) = self.scan_name()? else {
                return Err(
                    self.report_fatal_error("The XML declaration must end with \"?>\".".into())
                );
            };
            if !had_space && !first {
                return Err(self.report_fatal_error(
                    "White space is required before the encoding pseudo attribute in the XML \
                     declaration."
                        .into(),
                ));
            }
            first = false;
            self.skip_spaces()?;
            if !self.skip_char('=')? {
                return Err(self.report_fatal_error(format!(
                    "The ' = ' character must follow \"{name}\" in the XML declaration."
                )));
            }
            self.skip_spaces()?;
            let quote = self.peek_char()?;
            if quote != '"' as i32 && quote != '\'' as i32 {
                return Err(self.report_fatal_error(format!(
                    "The value following \"{name}\" in the XML declaration must be a quoted string."
                )));
            }
            self.scan_char()?;
            let mut value = Vec::new();
            loop {
                let c = self.scan_char()?;
                if c == -1 {
                    return Err(
                        self.report_fatal_error("The XML declaration must end with \"?>\".".into())
                    );
                }
                if c == quote {
                    break;
                }
                value.push(c as u16);
            }
            let value = String::from_utf16_lossy(&value);
            match name.as_str() {
                "version" => version = Some(value),
                "encoding" => {}
                "standalone" => {
                    if value == "yes" {
                        self.standalone = true;
                    } else if value != "no" {
                        return Err(self.report_fatal_error(format!(
                            "The standalone document declaration value must be \"yes\" or \"no\", \
                             not \"{value}\"."
                        )));
                    }
                }
                _ => {
                    return Err(
                        self.report_fatal_error("The XML declaration must end with \"?>\".".into())
                    );
                }
            }
        }
        if version.is_none() {
            return Err(
                self.report_fatal_error("The version is required in the XML declaration.".into())
            );
        }
        Ok(())
    }

    // port: XMLDocumentScannerImpl#scanDoctypeDecl
    fn scan_doctype_decl(&mut self) -> ScanResult<()> {
        if !self.skip_spaces()? {
            return Err(self.report_fatal_error(
                "White space is required after \"<!DOCTYPE\" in the document type declaration."
                    .into(),
            ));
        }
        let Some(root) = self.scan_name()? else {
            return Err(self.report_fatal_error(
                "The root element type must appear after \"<!DOCTYPE\" in the document type \
                 declaration."
                    .into(),
            ));
        };
        self.skip_spaces()?;
        if let Some(system_id) = self.scan_external_id()? {
            self.has_external_dtd = system_id.is_some();
        }
        self.skip_spaces()?;
        if self.skip_char('[')? {
            self.scan_internal_subset()?;
            self.skip_spaces()?;
        }
        if !self.skip_char('>')? {
            return Err(self.report_fatal_error(format!(
                "The document type declaration for root element type \"{root}\" must end with '>'."
            )));
        }
        Ok(())
    }

    /// Scans `SYSTEM "sys"` or `PUBLIC "pub" "sys"`; `Some(Some(system id))` when present.
    // port: XMLScanner#scanExternalID
    fn scan_external_id(&mut self) -> ScanResult<Option<Option<String>>> {
        if self.skip_string("PUBLIC")? {
            self.skip_spaces()?;
            self.scan_quoted_literal()?;
            self.skip_spaces()?;
            let system_id = self.scan_quoted_literal()?;
            return Ok(Some(Some(system_id)));
        }
        if self.skip_string("SYSTEM")? {
            self.skip_spaces()?;
            let system_id = self.scan_quoted_literal()?;
            return Ok(Some(Some(system_id)));
        }
        Ok(None)
    }

    fn scan_quoted_literal(&mut self) -> ScanResult<String> {
        let quote = self.peek_char()?;
        if quote != '"' as i32 && quote != '\'' as i32 {
            return Err(self.report_fatal_error(
                "The system identifier must begin with either a single or double quote character."
                    .into(),
            ));
        }
        self.scan_char()?;
        let mut value = Vec::new();
        loop {
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error("Premature end of file.".into()));
            }
            if c == quote {
                return Ok(String::from_utf16_lossy(&value));
            }
            value.push(c as u16);
        }
    }

    /// The internal DTD subset (`XMLDTDScannerImpl#scanDecls`): general entity declarations are
    /// recorded, every other declaration is skipped.
    // port: XMLDTDScannerImpl#scanDecls
    fn scan_internal_subset(&mut self) -> ScanResult<()> {
        loop {
            self.skip_spaces()?;
            if self.skip_char(']')? {
                return Ok(());
            }
            if self.skip_string("<!--")? {
                self.scan_comment()?;
            } else if self.skip_string("<?")? {
                self.scan_pi()?;
            } else if self.skip_string("<!ATTLIST")? {
                self.scan_attlist_decl()?;
            } else if self.skip_string("<!ENTITY")? {
                self.scan_entity_decl()?;
            } else if self.skip_char('%')? {
                if self.scan_name()?.is_none() || !self.skip_char(';')? {
                    return Err(self.report_fatal_error(
                        "The markup declarations contained or pointed to by the document type \
                         declaration must be well-formed."
                            .into(),
                    ));
                }
            } else if self.skip_string("<!")? {
                self.skip_markup_decl()?;
            } else {
                return Err(self.report_fatal_error(
                    "The markup declarations contained or pointed to by the document type \
                     declaration must be well-formed."
                        .into(),
                ));
            }
        }
    }

    // port: XMLDTDScannerImpl#scanAttlistDecl
    fn scan_attlist_decl(&mut self) -> ScanResult<()> {
        self.skip_spaces()?;
        let Some(element) = self.scan_name()? else {
            return Err(self.report_fatal_error(
                "The element type is required in the attribute-list declaration.".into(),
            ));
        };
        loop {
            self.skip_spaces()?;
            if self.skip_char('>')? {
                return Ok(());
            }
            let Some(name) = self.scan_name()? else {
                return Err(self.report_fatal_error(format!(
                    "The attribute name is required in the attribute-list declaration for \
                     element \"{element}\"."
                )));
            };
            self.skip_spaces()?;
            // AttType
            let is_cdata = if self.skip_char('(')? {
                self.skip_enumeration()?;
                false
            } else {
                let Some(att_type) = self.scan_name()? else {
                    return Err(self.report_fatal_error(format!(
                        "The attribute type is required in the declaration of attribute \
                         \"{name}\" for element \"{element}\"."
                    )));
                };
                if att_type == "NOTATION" {
                    self.skip_spaces()?;
                    if self.skip_char('(')? {
                        self.skip_enumeration()?;
                    }
                }
                att_type == "CDATA"
            };
            self.skip_spaces()?;
            // DefaultDecl
            let default = if self.skip_string("#REQUIRED")? || self.skip_string("#IMPLIED")? {
                None
            } else {
                if self.skip_string("#FIXED")? {
                    self.skip_spaces()?;
                }
                let quote = self.peek_char()?;
                if quote != '"' as i32 && quote != '\'' as i32 {
                    return Err(self.report_fatal_error(format!(
                        "The default value of attribute \"{name}\" for element \"{element}\" \
                         must be a quoted string."
                    )));
                }
                self.scan_char()?;
                let depth = self.entities.len();
                let mut value = Vec::new();
                self.scan_attribute_value(&mut value, quote, depth, &element, &name)?;
                if !is_cdata {
                    value = normalize_non_cdata(&value);
                }
                Some(value)
            };
            let decls = self.attribute_decls.entry(element.clone()).or_default();
            if !decls.contains_key(&name) {
                decls.insert(name, AttributeDecl { is_cdata, default });
            }
        }
    }

    fn skip_enumeration(&mut self) -> ScanResult<()> {
        loop {
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error("Premature end of file.".into()));
            }
            if c == ')' as i32 {
                return Ok(());
            }
        }
    }

    fn skip_markup_decl(&mut self) -> ScanResult<()> {
        let mut quote = -1;
        loop {
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error("Premature end of file.".into()));
            }
            if quote != -1 {
                if c == quote {
                    quote = -1;
                }
            } else if c == '"' as i32 || c == '\'' as i32 {
                quote = c;
            } else if c == '>' as i32 {
                return Ok(());
            }
        }
    }

    // port: XMLDTDScannerImpl#scanEntityDecl
    fn scan_entity_decl(&mut self) -> ScanResult<()> {
        self.skip_spaces()?;
        let is_pe = self.skip_char('%')?;
        self.skip_spaces()?;
        let Some(name) = self.scan_name()? else {
            return Err(self.report_fatal_error(
                "The name of the entity is required in the entity declaration.".into(),
            ));
        };
        self.skip_spaces()?;
        if let Some(_system_id) = self.scan_external_id()? {
            self.skip_spaces()?;
            let unparsed = self.skip_string("NDATA")?;
            if unparsed {
                self.skip_spaces()?;
                self.scan_name()?;
            }
            if !is_pe && !self.general_entities.contains_key(&name) {
                if unparsed {
                    self.unparsed_entities.insert(name.clone());
                }
                self.general_entities.insert(name.clone(), None);
            }
        } else {
            let value = self.scan_entity_value()?;
            if !is_pe && !self.general_entities.contains_key(&name) {
                self.general_entities.insert(name.clone(), Some(value));
            }
        }
        self.skip_spaces()?;
        if !self.skip_char('>')? {
            return Err(self.report_fatal_error(format!(
                "The declaration for the entity \"{name}\" must end with '>'."
            )));
        }
        Ok(())
    }

    /// `EntityValue`: character references are replaced, general entity references bypassed.
    // port: XMLDTDScannerImpl#scanEntityValue
    fn scan_entity_value(&mut self) -> ScanResult<Vec<u16>> {
        let quote = self.peek_char()?;
        if quote != '"' as i32 && quote != '\'' as i32 {
            return Err(self.report_fatal_error(
                "The replacement text must begin with either a single or double quote character."
                    .into(),
            ));
        }
        self.scan_char()?;
        let mut value = Vec::new();
        loop {
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error("Premature end of file.".into()));
            }
            if c == quote {
                return Ok(value);
            }
            if c == AMP && self.skip_char('#')? {
                self.scan_char_reference_value(&mut value)?;
            } else {
                value.push(c as u16);
            }
        }
    }

    /// Scans the content of the root element, its descendants and its end tag.
    // port: XMLDocumentFragmentScannerImpl$FragmentContentDriver#next
    fn scan_element_content(&mut self) -> ScanResult<()> {
        loop {
            if self.state == ScannerState::Content {
                match self.peek_char()? {
                    -1 => {
                        return Err(self.report_fatal_error(
                            "XML document structures must start and end within the same entity."
                                .into(),
                        ));
                    }
                    LT => {
                        self.scan_char()?;
                        self.state = ScannerState::StartOfMarkup;
                    }
                    AMP => {
                        self.scan_char()?;
                        self.state = ScannerState::Reference;
                    }
                    _ => self.state = ScannerState::CharacterData,
                }
            }
            if self.state == ScannerState::StartOfMarkup {
                self.start_of_markup()?;
            }
            match self.state {
                ScannerState::StartElementTag => {
                    // returns true if the element is empty
                    self.empty_element = self.scan_start_element()?;
                    if self.empty_element {
                        self.state = ScannerState::EndElementTag;
                    } else {
                        self.state = ScannerState::Content;
                    }
                }
                ScannerState::CharacterData => self.scan_character_data()?,
                ScannerState::EndElementTag => {
                    if self.empty_element {
                        self.empty_element = false;
                    } else {
                        self.scan_end_element()?;
                    }
                    self.state = ScannerState::Content;
                    if self.element_stack.is_empty() {
                        return Ok(());
                    }
                }
                ScannerState::Comment => {
                    self.scan_comment()?;
                    self.state = ScannerState::Content;
                }
                ScannerState::Pi => {
                    self.scan_pi()?;
                    self.state = ScannerState::Content;
                }
                ScannerState::Cdata => {
                    let mut content = Vec::new();
                    self.scan_cdata_section(&mut content)?;
                    self.state = ScannerState::Content;
                    self.characters(&content);
                }
                ScannerState::Reference => {
                    self.state = ScannerState::Content;
                    if self.skip_char('#')? {
                        let mut content = Vec::new();
                        self.scan_char_reference_value(&mut content)?;
                        self.characters(&content);
                    } else {
                        self.scan_entity_reference()?;
                    }
                }
                ScannerState::Content | ScannerState::StartOfMarkup => unreachable!(),
            }
        }
    }

    // port: XMLDocumentFragmentScannerImpl#startOfMarkup
    fn start_of_markup(&mut self) -> ScanResult<()> {
        let ch = self.peek_char()?;
        if xml_char::is_name_start(ch) {
            self.state = ScannerState::StartElementTag;
            return Ok(());
        }
        match ch {
            0x3F => {
                // '?'
                self.state = ScannerState::Pi;
                self.skip_char('?')?;
            }
            0x21 => {
                // '!'
                self.skip_char('!')?;
                if self.skip_char('-')? {
                    if !self.skip_char('-')? {
                        return Err(
                            self.report_fatal_error("Comment must start with \"<!--\".".into())
                        );
                    }
                    self.state = ScannerState::Comment;
                } else if self.skip_string("[CDATA[")? {
                    self.state = ScannerState::Cdata;
                } else {
                    return Err(self.markup_not_recognized_in_content());
                }
            }
            0x2F => {
                // '/'
                self.state = ScannerState::EndElementTag;
                self.skip_char('/')?;
            }
            _ => return Err(self.markup_not_recognized_in_content()),
        }
        Ok(())
    }

    fn markup_not_recognized_in_content(&self) -> SaxParseException {
        self.report_fatal_error(
            "The content of elements must consist of well-formed character data or markup.".into(),
        )
    }

    // port: XMLDocumentFragmentScannerImpl$FragmentContentDriver#next (SCANNER_STATE_CHARACTER_DATA)
    fn scan_character_data(&mut self) -> ScanResult<()> {
        let mut content = Vec::new();
        let mut c = self.entity_scan_content(&mut content)?;

        if self.skip_char('<')? {
            // check if we have reached end of element
            if self.skip_char('/')? {
                self.state = ScannerState::EndElementTag;
                // check if its start of new element
            } else if xml_char::is_name_start(self.peek_char()?) {
                self.state = ScannerState::StartElementTag;
            } else {
                self.state = ScannerState::StartOfMarkup;
            }
            self.characters(&content);
            return Ok(());
        }
        if c == CR {
            // happens when there is the character reference &#13;
            self.scan_char()?;
            content.push(c as u16);
            c = -1;
        } else if c == RBRACKET {
            content.push(self.scan_char()? as u16);
            // We work on a single character basis to handle cases such as:
            // ']]]>' which we might otherwise miss.
            if self.skip_char(']')? {
                content.push(']' as u16);
                while self.skip_char(']')? {
                    content.push(']' as u16);
                }
                if self.skip_char('>')? {
                    return Err(self.report_fatal_error(
                        "The character sequence \"]]>\" must not appear in content unless used \
                         to mark the end of a CDATA section."
                            .into(),
                    ));
                }
            }
            c = -1;
        }

        if c == LT {
            self.scan_char()?;
            self.state = ScannerState::StartOfMarkup;
        } else if c == AMP {
            self.scan_char()?;
            self.state = ScannerState::Reference;
        } else if c != -1 && xml_char::is_invalid(c) {
            if xml_char::is_high_surrogate(c) {
                // special case: surrogates
                self.scan_surrogates(&mut content)?;
                self.state = ScannerState::Content;
            } else {
                return Err(self.report_fatal_error(format!(
                    "An invalid XML character (Unicode: 0x{c:x}) was found in the element \
                     content of the document."
                )));
            }
        } else {
            self.scan_content(&mut content)?;
            self.state = ScannerState::Content;
        }
        self.characters(&content);
        Ok(())
    }

    // port: XMLScanner#scanSurrogates
    fn scan_surrogates(&mut self, buf: &mut Vec<u16>) -> ScanResult<()> {
        let high = self.scan_char()?;
        let low = self.peek_char()?;
        if !xml_char::is_low_surrogate(low) {
            return Err(self.report_fatal_error(format!(
                "An invalid XML character (Unicode: 0x{high:x}) was found in the element content \
                 of the document."
            )));
        }
        self.scan_char()?;
        let c = xml_char::supplemental(high as u16, low as u16);
        if xml_char::is_invalid(c) {
            return Err(self.report_fatal_error(format!(
                "An invalid XML character (Unicode: 0x{c:x}) was found in the element content of \
                 the document."
            )));
        }
        buf.push(high as u16);
        buf.push(low as u16);
        Ok(())
    }

    // port: XMLDocumentFragmentScannerImpl#scanStartElement
    fn scan_start_element(&mut self) -> ScanResult<bool> {
        let raw_name = self.scan_name()?.unwrap();
        let mut attributes = Attributes::default();
        let empty;
        loop {
            let saw_space = self.skip_spaces()?;
            let c = self.peek_char()?;
            if c == '>' as i32 {
                self.scan_char()?;
                empty = false;
                break;
            } else if c == '/' as i32 {
                self.scan_char()?;
                if !self.skip_char('>')? {
                    return Err(self.element_unterminated(&raw_name));
                }
                empty = true;
                break;
            } else if !xml_char::is_name_start(c) || !saw_space {
                return Err(self.element_unterminated(&raw_name));
            }
            self.scan_attribute(&raw_name, &mut attributes)?;
        }
        // XMLDTDValidator#addDTDDefaultAttrsAndValidate: values of non-CDATA attributes are
        // normalized, declared defaults are added for attributes that are not specified.
        if let Some(decls) = self.attribute_decls.get(&raw_name) {
            for (i, name) in attributes.names.iter().enumerate() {
                if decls.get(name).is_some_and(|decl| !decl.is_cdata) {
                    let value = normalize_non_cdata(attributes.values[i].as_units());
                    attributes.values[i] = JsString::from_units(value);
                }
            }
            for (name, decl) in decls {
                if let Some(default) = &decl.default
                    && !attributes.names.contains(name)
                {
                    attributes.names.push(name.clone());
                    attributes
                        .values
                        .push(JsString::from_units(default.clone()));
                }
            }
        }
        self.element_stack.push(raw_name.clone());
        self.handler.start_element("", "", &raw_name, &attributes);
        if empty {
            // AbstractSAXParser#emptyElement
            self.element_stack.pop();
            self.handler.end_element("", "", &raw_name);
        }
        Ok(empty)
    }

    fn element_unterminated(&self, raw_name: &str) -> SaxParseException {
        self.report_fatal_error(format!(
            "Element type \"{raw_name}\" must be followed by either attribute specifications, \
             \">\" or \"/>\"."
        ))
    }

    // port: XMLDocumentFragmentScannerImpl#scanAttribute
    fn scan_attribute(&mut self, element: &str, attributes: &mut Attributes) -> ScanResult<()> {
        let name = self.scan_name()?.unwrap();
        self.skip_spaces()?;
        if !self.skip_char('=')? {
            return Err(self.report_fatal_error(format!(
                "Attribute name \"{name}\" associated with an element type \"{element}\" must be \
                 followed by the ' = ' character."
            )));
        }
        self.skip_spaces()?;
        let quote = self.peek_char()?;
        if quote != '"' as i32 && quote != '\'' as i32 {
            return Err(self.report_fatal_error(format!(
                "Open quote is expected for attribute \"{name}\" associated with an  element \
                 type  \"{element}\"."
            )));
        }
        self.scan_char()?;
        let depth = self.entities.len();
        let mut value = Vec::new();
        self.scan_attribute_value(&mut value, quote, depth, element, &name)?;
        if attributes.names.contains(&name) {
            return Err(self.report_fatal_error(format!(
                "Attribute \"{name}\" was already specified for element \"{element}\"."
            )));
        }
        attributes.names.push(name);
        attributes.values.push(JsString::from_units(value));
        Ok(())
    }

    /// The value of an attribute of (undeclared, so CDATA) type: white space characters become
    /// spaces, references are replaced.
    // port: XMLScanner#scanAttributeValue
    fn scan_attribute_value(
        &mut self,
        value: &mut Vec<u16>,
        quote: i32,
        depth: usize,
        element: &str,
        name: &str,
    ) -> ScanResult<()> {
        loop {
            let in_entity = self.entities.len() > depth;
            if in_entity && self.current().position == self.current().ch.len() {
                self.entities.pop();
                continue;
            }
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error(format!(
                    "Close quote is expected for attribute \"{name}\" associated with an element \
                     type \"{element}\"."
                )));
            }
            if c == quote && !in_entity {
                return Ok(());
            }
            if c == LT {
                // the '<' is reported before it is consumed
                self.current_mut().position -= 1;
                return Err(self.report_fatal_error(format!(
                    "The value of attribute \"{name}\" associated with an element type \
                     \"{element}\" must not contain the '<' character."
                )));
            } else if c == AMP {
                if self.skip_char('#')? {
                    self.scan_char_reference_value(value)?;
                } else {
                    let entity_name = self.scan_reference_name()?;
                    match builtin_entity(&entity_name) {
                        Some(ch) => value.push(ch),
                        None => match self.general_entities.get(&entity_name).cloned() {
                            Some(Some(text)) => self.entities.push(Entity {
                                ch: text,
                                position: 0,
                                name: Some(entity_name),
                                element_depth: self.element_stack.len(),
                            }),
                            Some(None) => {
                                return Err(self.report_fatal_error(format!(
                                    "The external entity reference \"&{entity_name};\" is not \
                                     permitted in an attribute value."
                                )));
                            }
                            None => self.undeclared_entity(&entity_name)?,
                        },
                    }
                }
            } else if c == 0x20 || c == 0xA || c == 0xD || c == 0x9 {
                value.push(0x20);
            } else if xml_char::is_high_surrogate(c) {
                value.push(c as u16);
                let low = self.peek_char()?;
                if xml_char::is_low_surrogate(low) {
                    self.scan_char()?;
                    value.push(low as u16);
                }
            } else if xml_char::is_invalid(c) {
                return Err(self.report_fatal_error(format!(
                    "An invalid XML character (Unicode: 0x{c:x}) was found in the value of \
                     attribute \"{name}\" and element is \"{element}\"."
                )));
            } else {
                value.push(c as u16);
            }
        }
    }

    // port: XMLDocumentFragmentScannerImpl#scanEndElement
    fn scan_end_element(&mut self) -> ScanResult<()> {
        let start = self.element_stack.pop().unwrap();
        let entity = self.current();
        let units: Vec<u16> = start.encode_utf16().collect();
        let matches = entity.ch[entity.position..].starts_with(&units)
            && !entity
                .ch
                .get(entity.position + units.len())
                .is_some_and(|&c| xml_char::is_name(c as i32));
        if !matches {
            return Err(self.report_fatal_error(format!(
                "The element type \"{start}\" must be terminated by the matching end-tag \
                 \"</{start}>\"."
            )));
        }
        self.current_mut().position += units.len();
        self.skip_spaces()?;
        if !self.skip_char('>')? {
            return Err(self.report_fatal_error(format!(
                "The end-tag for element type \"{start}\" must end with a '>' delimiter."
            )));
        }
        self.handler.end_element("", "", &start);
        Ok(())
    }

    // port: XMLScanner#scanComment
    fn scan_comment(&mut self) -> ScanResult<()> {
        loop {
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error("The comment must end with \"-->\".".into()));
            }
            if c == '-' as i32 && self.skip_char('-')? {
                if self.skip_char('>')? {
                    return Ok(());
                }
                return Err(self.report_fatal_error(
                    "The string \"--\" is not permitted within comments.".into(),
                ));
            }
            self.check_char(c, "comment")?;
        }
    }

    fn check_char(&mut self, c: i32, place: &str) -> ScanResult<()> {
        if xml_char::is_high_surrogate(c) && xml_char::is_low_surrogate(self.peek_char()?) {
            self.scan_char()?;
            return Ok(());
        }
        if xml_char::is_invalid(c) {
            return Err(self.report_fatal_error(format!(
                "An invalid XML character (Unicode: 0x{c:x}) was found in the {place}."
            )));
        }
        Ok(())
    }

    // port: XMLScanner#scanPI
    fn scan_pi(&mut self) -> ScanResult<()> {
        let Some(target) = self.scan_name()? else {
            return Err(self.report_fatal_error(
                "The processing instruction must begin with the name of the target.".into(),
            ));
        };
        if target.eq_ignore_ascii_case("xml") {
            return Err(self.report_fatal_error(
                "The processing instruction target matching \"[xX][mM][lL]\" is not allowed."
                    .into(),
            ));
        }
        let mut data = Vec::new();
        if !self.skip_spaces()? {
            if self.skip_string("?>")? {
                self.handler
                    .processing_instruction(&target, &JsString::from_units(data));
                return Ok(());
            }
            return Err(self.report_fatal_error(
                "White space is required between the processing instruction target and data."
                    .into(),
            ));
        }
        loop {
            if self.skip_string("?>")? {
                self.handler
                    .processing_instruction(&target, &JsString::from_units(data));
                return Ok(());
            }
            let c = self.scan_char()?;
            if c == -1 {
                return Err(self.report_fatal_error(
                    "The processing instruction must end with \"?>\".".into(),
                ));
            }
            self.check_char(c, "processing instruction")?;
            data.push(c as u16);
        }
    }

    // port: XMLDocumentFragmentScannerImpl#scanCDATASection
    fn scan_cdata_section(&mut self, content: &mut Vec<u16>) -> ScanResult<()> {
        loop {
            if self.skip_string("]]>")? {
                return Ok(());
            }
            let c = self.scan_char()?;
            if c == -1 {
                return Err(
                    self.report_fatal_error("The CDATA section must end with \"]]>\".".into())
                );
            }
            if xml_char::is_high_surrogate(c) && xml_char::is_low_surrogate(self.peek_char()?) {
                content.push(c as u16);
                content.push(self.scan_char()? as u16);
                continue;
            }
            if xml_char::is_invalid(c) {
                return Err(self.report_fatal_error(format!(
                    "An invalid XML character (Unicode: 0x{c:x}) was found in the CDATA section."
                )));
            }
            content.push(c as u16);
        }
    }

    // port: XMLScanner#scanCharReferenceValue
    fn scan_char_reference_value(&mut self, buf: &mut Vec<u16>) -> ScanResult<()> {
        let hex = self.skip_char('x')?;
        let mut digits = String::new();
        loop {
            let c = self.peek_char()?;
            let is_digit = if hex {
                (c as u8 as char).is_ascii_hexdigit() && c < 0x80
            } else {
                (c as u8 as char).is_ascii_digit() && c < 0x80
            };
            if c < 0 || !is_digit {
                break;
            }
            self.scan_char()?;
            digits.push(c as u8 as char);
        }
        if digits.is_empty() {
            return Err(self.report_fatal_error(if hex {
                "A hexadecimal representation must immediately follow the \"&#x\" in a character \
                 reference."
                    .into()
            } else {
                "A decimal representation must immediately follow the \"&#\" in a character \
                 reference."
                    .into()
            }));
        }
        if !self.skip_char(';')? {
            return Err(self.report_fatal_error(
                "The character reference must end with the ';' delimiter.".into(),
            ));
        }
        let value = i64::from_str_radix(&digits, if hex { 16 } else { 10 }).unwrap_or(-1);
        if !(0..=0x10FFFF).contains(&value) || xml_char::is_invalid(value as i32) {
            let text = if hex { format!("x{digits}") } else { digits };
            return Err(self.report_fatal_error(format!(
                "Character reference \"&#{text}\" is an invalid XML character."
            )));
        }
        let value = value as u32;
        if value < 0x10000 {
            buf.push(value as u16);
        } else {
            let v = value - 0x10000;
            buf.push((0xD800 + (v >> 10)) as u16);
            buf.push((0xDC00 + (v & 0x3FF)) as u16);
        }
        Ok(())
    }

    fn scan_reference_name(&mut self) -> ScanResult<String> {
        let Some(name) = self.scan_name()? else {
            return Err(self.report_fatal_error(
                "The entity name must immediately follow the '&' in the entity reference.".into(),
            ));
        };
        if !self.skip_char(';')? {
            return Err(self.report_fatal_error(format!(
                "The reference to entity \"{name}\" must end with the ';' delimiter."
            )));
        }
        Ok(name)
    }

    /// An undeclared entity is a fatal error unless an external DTD that was not read could
    /// declare it; then the entity is skipped.
    fn undeclared_entity(&mut self, name: &str) -> ScanResult<()> {
        if self.has_external_dtd && !self.standalone {
            self.handler.skipped_entity(name);
            Ok(())
        } else {
            Err(self.report_fatal_error(format!(
                "The entity \"{name}\" was referenced, but not declared."
            )))
        }
    }

    // port: XMLDocumentFragmentScannerImpl#scanEntityReference
    fn scan_entity_reference(&mut self) -> ScanResult<()> {
        let name = self.scan_reference_name()?;
        if self.unparsed_entities.contains(&name) {
            return Err(self.report_fatal_error(format!(
                "The unparsed entity reference \"&{name};\" is not permitted."
            )));
        }
        // handle built-in entities: XMLDocumentFragmentScannerImpl#handleCharacter
        if let Some(c) = builtin_entity(&name) {
            self.handler.characters(&[c]);
            return Ok(());
        }
        match self.general_entities.get(&name).cloned() {
            // external general entities are not read
            Some(None) => self.handler.skipped_entity(&name),
            Some(Some(text)) => {
                if self
                    .entities
                    .iter()
                    .any(|e| e.name.as_deref() == Some(name.as_str()))
                {
                    return Err(self.report_fatal_error(format!(
                        "Recursive entity reference \"{name}\". (Reference path: {name} -> \
                         {name}),"
                    )));
                }
                self.entities.push(Entity {
                    ch: text,
                    position: 0,
                    name: Some(name),
                    element_depth: self.element_stack.len(),
                });
            }
            None => self.undeclared_entity(&name)?,
        }
        Ok(())
    }
}

struct AttributeDecl {
    is_cdata: bool,
    default: Option<Vec<u16>>,
}

/// The normalization of a non-CDATA attribute value: leading and trailing spaces are removed and
/// each run of spaces becomes a single space.
// port: XMLDTDValidator#normalizeAttrValue
fn normalize_non_cdata(value: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(value.len());
    let mut pending_space = false;
    for &c in value {
        if c == 0x20 {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(0x20);
                pending_space = false;
            }
            out.push(c);
        }
    }
    out
}

fn builtin_entity(name: &str) -> Option<u16> {
    match name {
        "amp" => Some('&' as u16),
        "lt" => Some('<' as u16),
        "gt" => Some('>' as u16),
        "quot" => Some('"' as u16),
        "apos" => Some('\'' as u16),
        _ => None,
    }
}
