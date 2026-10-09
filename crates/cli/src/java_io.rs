/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java.

use closure_rhino::{java_lang::charset::Charset, js_string::JsString};
use std::io::{self, Write};
// port: StreamEncoder#implWrite / CharsetEncoder#encode (the six shared Charset variants)
pub fn encode(value: &JsString, charset: Charset, emit_bom: bool) -> Vec<u8> {
    encode_units(value.as_units(), charset, emit_bom)
}
/// Rust-only: `encode` of code units, with a fast path for ASCII in UTF-8 (D-025).
fn encode_units(units: &[u16], charset: Charset, emit_bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(units.len());
    if charset == Charset::UTF_16 && emit_bom && !units.is_empty() {
        out.extend([0xfe, 0xff]);
    }
    let mut units = units;
    if charset == Charset::UTF_8 {
        let ascii = units.iter().position(|u| *u >= 0x80).unwrap_or(units.len());
        out.extend(units[..ascii].iter().map(|u| *u as u8));
        units = &units[ascii..];
    }
    for c in char::decode_utf16(units.iter().copied()) {
        match charset {
            Charset::UTF_8 => match c {
                Ok(c) => {
                    let mut bytes = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut bytes).as_bytes());
                }
                Err(_) => out.push(b'?'),
            },
            Charset::US_ASCII | Charset::ISO_8859_1 => {
                let max = if charset == Charset::US_ASCII {
                    127
                } else {
                    255
                };
                out.push(
                    c.ok()
                        .filter(|c| *c as u32 <= max)
                        .map_or(b'?', |c| c as u8),
                );
            }
            Charset::UTF_16 | Charset::UTF_16BE | Charset::UTF_16LE => {
                let mut chars = [0u16; 2];
                for &unit in c.unwrap_or('\u{fffd}').encode_utf16(&mut chars).iter() {
                    out.extend(if charset == Charset::UTF_16LE {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
            }
        }
    }
    out
}
pub struct EncodedWriter<'a> {
    stream: Box<dyn Write + 'a>,
    charset: Charset,
    pending: Vec<u8>,
    pending_chars: Vec<u16>,
    emit_bom: bool,
    print_stream: Option<PrintStream>,
    closed: bool,
}
/// Java Appendable accepts UTF-16 text before a Writer applies its charset.
pub trait JavaAppendable: Write {
    fn append(&mut self, value: &JsString) -> io::Result<()>;
}
impl<T: JavaAppendable + ?Sized> JavaAppendable for Box<T> {
    fn append(&mut self, value: &JsString) -> io::Result<()> {
        (**self).append(value)
    }
}
impl JavaAppendable for Vec<u8> {
    fn append(&mut self, value: &JsString) -> io::Result<()> {
        self.write_all(&encode(value, Charset::UTF_8, false))
    }
}
impl JavaAppendable for io::Cursor<Vec<u8>> {
    fn append(&mut self, value: &JsString) -> io::Result<()> {
        self.write_all(&encode(value, Charset::UTF_8, false))
    }
}
#[derive(Default)]
pub struct StringBuilder {
    units: Vec<u16>,
}
impl StringBuilder {
    pub fn into_js_string(self) -> JsString {
        JsString::from_units(self.units)
    }
}
impl JavaAppendable for StringBuilder {
    // port: StringBuilder#append
    fn append(&mut self, value: &JsString) -> io::Result<()> {
        self.units.extend_from_slice(value.as_units());
        Ok(())
    }
}
impl Write for StringBuilder {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.units
            .extend(String::from_utf8_lossy(bytes).encode_utf16());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub trait OutputWriter: JavaAppendable {
    fn close(&mut self) -> io::Result<()>;
}
impl OutputWriter for io::Cursor<Vec<u8>> {
    fn close(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl<'a> EncodedWriter<'a> {
    // port: AbstractCommandLineRunner#createWriter
    pub fn new(stream: Box<dyn Write + 'a>, charset: Charset) -> Self {
        Self {
            stream,
            charset,
            pending: Vec::new(),
            pending_chars: Vec::new(),
            emit_bom: true,
            print_stream: None,
            closed: false,
        }
    }
    pub fn for_print_stream(stream: PrintStream, charset: Charset) -> Self {
        let mut writer = Self::new(Box::new(stream.clone()), charset);
        writer.print_stream = Some(stream);
        writer
    }
}
impl JavaAppendable for EncodedWriter<'_> {
    // port: BufferedWriter#append
    fn append(&mut self, value: &JsString) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::other("Stream closed"));
        }
        self.pending_chars
            .extend(String::from_utf8_lossy(&self.pending).encode_utf16());
        self.pending.clear();
        self.pending_chars.extend_from_slice(value.as_units());
        Ok(())
    }
}
impl OutputWriter for EncodedWriter<'_> {
    // port: BufferedWriter#close / OutputStreamWriter#close
    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let result = self.flush();
        self.closed = true;
        if let Some(stream) = &mut self.print_stream {
            stream.close();
        }
        result
    }
}
impl Write for EncodedWriter<'_> {
    // port: BufferedWriter#write
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::other("Stream closed"));
        }
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    // port: BufferedWriter#flush / OutputStreamWriter#flush
    fn flush(&mut self) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::other("Stream closed"));
        }
        // Rust-only fast path (D-025): bytes written as UTF-8 reach a UTF-8 stream unchanged
        // (after the replacement of malformed input that the UTF-16 round trip makes).
        if self.charset == Charset::UTF_8 && self.pending_chars.is_empty() {
            if !self.pending.is_empty() {
                let text = String::from_utf8_lossy(&self.pending);
                self.stream.write_all(text.as_bytes())?;
                self.pending.clear();
                self.emit_bom = false;
            }
            return self.stream.flush();
        }
        self.pending_chars
            .extend(String::from_utf8_lossy(&self.pending).encode_utf16());
        self.pending.clear();
        if !self.pending_chars.is_empty() {
            self.stream.write_all(&encode_units(
                &self.pending_chars,
                self.charset,
                self.emit_bom,
            ))?;
            self.pending_chars.clear();
            self.emit_bom = false;
        }
        self.stream.flush()
    }
}

/// Connects the compiler's Send writer to the runner's injectable stream.
/// The runner drains these ordered bytes after reports and when doRun returns or unwinds.
pub struct StreamProxy(pub std::sync::mpsc::Sender<Vec<u8>>);
impl std::io::Write for StreamProxy {
    // port: PrintStream#write(byte[], int, int)
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let _ = self.0.send(bytes.to_vec());
        Ok(bytes.len())
    }
    // port: PrintStream#flush
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// The OS reason is Java's strerror text; Rust adds an os-error suffix.
pub fn error_message(error: &std::io::Error) -> String {
    let text = error.to_string();
    text.split(" (os error ").next().unwrap().to_owned()
}
pub fn file_not_found(
    error: std::io::Error,
    path: &str,
    output: bool,
    string_constructor: bool,
) -> crate::abstract_command_line_runner::RunnerException {
    use crate::abstract_command_line_runner::RunnerException;
    let mut frames = if output {
        vec![
            "java.base/java.io.FileOutputStream.open0(Native Method)",
            "java.base/java.io.FileOutputStream.open(FileOutputStream.java:289)",
            "java.base/java.io.FileOutputStream.<init>(FileOutputStream.java:230)",
        ]
    } else {
        vec![
            "java.base/java.io.FileInputStream.open0(Native Method)",
            "java.base/java.io.FileInputStream.open(FileInputStream.java:213)",
            "java.base/java.io.FileInputStream.<init>(FileInputStream.java:152)",
        ]
    };
    if string_constructor {
        frames.push(if output {
            "java.base/java.io.FileOutputStream.<init>(FileOutputStream.java:118)"
        } else {
            "java.base/java.io.FileInputStream.<init>(FileInputStream.java:106)"
        });
    }
    RunnerException::java_exception(
        "java.io.FileNotFoundException",
        format!("{path} ({})", error_message(&error)),
        frames.into_iter().map(str::to_owned).collect(),
    )
}

/// Preserve the exception types erased by VariableMap::load's io::Result adapter.
pub fn variable_map_load_error(
    error: io::Error,
    path: &str,
) -> crate::abstract_command_line_runner::RunnerException {
    use crate::abstract_command_line_runner::RunnerException;
    if let Some(parse) = error
        .get_ref()
        .and_then(|e| e.downcast_ref::<closure_rhino::java_lang::parse_exception::ParseException>())
    {
        let cause = RunnerException::java_exception(
            "java.text.ParseException",
            parse.get_message().into(),
            vec![],
        )
        .at_cli("VariableMap", "fromLines", 160)
        .at_cli("VariableMap", "fromBytes", 117)
        .at_cli("VariableMap", "load", 86);
        return RunnerException::java_exception("java.io.IOException", parse.to_string(), vec![])
            .at_cli("VariableMap", "load", 89)
            .caused_by(cause);
    }
    file_not_found(error, path, false, false)
        .at("com.google.common.io.Files$FileByteSource.openStream(Files.java:132)")
        .at("com.google.common.io.Files$FileByteSource.read(Files.java:156)")
        .at("com.google.common.io.Files.toByteArray(Files.java:238)")
        .at_cli("VariableMap", "load", 86)
}

/// Java PrintStream suppresses write/flush IOException and remembers the trouble bit.
/// FileOutputStreams opened for output writers retain their ordinary throwing behavior.
struct PrintStreamState {
    stream: Box<dyn Write>,
    trouble: bool,
    closed: bool,
}
#[derive(Clone)]
pub struct PrintStream(std::rc::Rc<std::cell::RefCell<PrintStreamState>>);
impl PrintStream {
    // port: PrintStream#PrintStream(OutputStream)
    pub fn new(stream: Box<dyn Write>) -> Self {
        Self(std::rc::Rc::new(std::cell::RefCell::new(
            PrintStreamState {
                stream,
                trouble: false,
                closed: false,
            },
        )))
    }
    // port: PrintStream#close
    pub fn close(&mut self) {
        let mut state = self.0.borrow_mut();
        if !state.closed {
            if state.stream.flush().is_err() {
                state.trouble = true;
            }
            state.stream = Box::new(io::sink());
            state.closed = true;
        }
    }
    // port: PrintStream#checkError
    pub fn check_error(&mut self) -> bool {
        if !self.0.borrow().closed {
            let _ = self.flush();
        }
        self.0.borrow().trouble
    }
}
impl Write for PrintStream {
    // port: PrintStream#write(byte[], int, int)
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut();
        if state.closed || state.stream.write_all(bytes).is_err() {
            state.trouble = true;
        }
        Ok(bytes.len())
    }
    // port: PrintStream#flush
    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.0.borrow_mut();
        if state.closed || state.stream.flush().is_err() {
            state.trouble = true;
        }
        Ok(())
    }
}
