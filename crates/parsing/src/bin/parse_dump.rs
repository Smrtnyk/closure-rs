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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/src/com/google/javascript/jscomp/OracleWorker.java,
//   oracle/src/com/google/javascript/jscomp/ParseDump.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CompilerInput.java.

//! Oracle-compatible parse_dump JSON-lines tool. Library modules contain only the Java ports.
#[path = "parse_dump/dump.rs"]
mod dump;
#[path = "parse_dump/options.rs"]
mod options;
#[path = "parse_dump/source_file.rs"]
mod source_file;

use closure_parsing::{
    parser::{
        trees::comment::Comment,
        util::{source_position::SourcePosition, source_range::SourceRange},
    },
    parser_runner::ParserRunner,
};
use closure_rhino::{
    error_reporter::{ErrorReporter, NullErrorReporter},
    input_id::InputId,
    ir::IR,
    js_string::JsString,
    node::Ast,
    static_source_file::{SourceKind, StaticSourceFile},
};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Read, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use dump::{Dump, feature_set, js_string};
use options::Options;
use source_file::SourceFile;

#[derive(Default)]
struct RecordingReporter {
    reports: Vec<Value>,
}
impl RecordingReporter {
    // port: ErrorReporter#error
    // port: ErrorReporter#warning
    fn record(&mut self, level: &str, message: &str, source_name: &str, lineno: i32, charno: i32) {
        self.reports.push(json!({"level":level,"message":message,"source_name":source_name,"lineno":lineno,"charno":charno}));
    }
}
impl ErrorReporter for RecordingReporter {
    // port: ErrorReporter#error
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.record("ERROR", message, source_name, line, line_offset);
    }
    // port: ErrorReporter#warning
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.record("WARNING", message, source_name, line, line_offset);
    }
    // port: ErrorReporter#error
    fn error_js_string(
        &mut self,
        message: &JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.record(
            "ERROR",
            &diagnostic_utf8(message),
            source_name,
            line,
            line_offset,
        );
    }
    // port: ErrorReporter#warning
    fn warning_js_string(
        &mut self,
        message: &JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.record(
            "WARNING",
            &diagnostic_utf8(message),
            source_name,
            line,
            line_offset,
        );
    }
}
// port: CharsetEncoder#CharsetEncoder (default malformed-input replacement is ASCII '?')
// The oracle's PrintStream encodes strings added by addProperty directly; value() uses jsString.
fn diagnostic_utf8(message: &JsString) -> String {
    std::char::decode_utf16(message.as_units().iter().copied())
        .map(|ch| ch.unwrap_or('?'))
        .collect()
}
// port: ParseDump#pos
fn position(p: &SourcePosition) -> Value {
    json!({"line":p.line,"column":p.column,"offset":p.offset})
}
// port: ParseDump#range
fn range(r: &SourceRange) -> Value {
    json!({"start":position(&r.start),"end":position(&r.end)})
}
// port: ParseDump#run
fn comment(c: &Comment) -> Value {
    json!({"type":format!("{:?}",c.type_),"value":js_string(&c.value),"location":range(&c.location)})
}

// port: ParseDump#jsString (inverse for request text)
fn request_string(v: &Value) -> Result<JsString, String> {
    if let Some(s) = v.as_str() {
        return Ok(JsString::from(s));
    }
    if let Some(units) = v.get("utf16").and_then(Value::as_array) {
        let mut result = Vec::with_capacity(units.len());
        for unit in units {
            result.push(
                u16::try_from(unit.as_u64().ok_or("invalid UTF-16 code unit")?)
                    .map_err(|_| "invalid UTF-16 code unit")?,
            );
        }
        return Ok(JsString::from_units(result));
    }
    Err("expected a Java String".into())
}

// JSON allows escaped lone UTF-16 surrogates; serde_json's String requires Unicode scalar values.
// Decode JSON string tokens to JsString before passing the remaining structure to serde_json.
fn decode_request(text: &str) -> Result<Value, String> {
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            let ch = text[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
            continue;
        }
        i += 1;
        let mut units = Vec::new();
        let mut closed = false;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    i += 1;
                    closed = true;
                    break;
                }
                b'\\' => {
                    i += 1;
                    let escape = *bytes.get(i).ok_or("incomplete JSON escape")?;
                    i += 1;
                    let ch = match escape {
                        b'"' => 34,
                        b'\\' => 92,
                        b'/' => 47,
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            let end = i + 4;
                            let hex = text.get(i..end).ok_or("incomplete JSON Unicode escape")?;
                            let unit = u16::from_str_radix(hex, 16)
                                .map_err(|_| "invalid JSON Unicode escape")?;
                            i = end;
                            unit
                        }
                        _ => return Err("invalid JSON escape".into()),
                    };
                    units.push(ch);
                }
                _ => {
                    let ch = text[i..].chars().next().unwrap();
                    if ch < ' ' {
                        return Err("unescaped JSON control character".into());
                    }
                    let mut buf = [0; 2];
                    units.extend_from_slice(ch.encode_utf16(&mut buf));
                    i += ch.len_utf8();
                }
            }
        }
        if !closed {
            return Err("unterminated JSON string".into());
        }
        let value = js_string(&JsString::from_units(units));
        result.push_str(&serde_json::to_string(&value).map_err(|e| e.to_string())?);
    }
    serde_json::from_str(&result).map_err(|e| e.to_string())
}

// port: ParseDump#run
// port: CompilerInput.JsAst#parse
fn run(req: &Value) -> Result<Value, String> {
    let (name, code) = if let Some(content) = req.get("content") {
        (
            req.get("name")
                .map(|name| request_string(name).map(|name| diagnostic_utf8(&name)))
                .transpose()?
                .unwrap_or_else(|| "input.js".to_owned()),
            request_string(content)?,
        )
    } else {
        let path = req
            .get("path")
            .and_then(Value::as_str)
            .ok_or("missing content or path")?;
        let code =
            std::fs::read_to_string(path).map_err(|e| format!("java.io.IOException: {e}"))?;
        (
            req.get("name")
                .map(|name| request_string(name).map(|name| diagnostic_utf8(&name)))
                .transpose()?
                .unwrap_or_else(|| path.to_owned()),
            JsString::from(code),
        )
    };
    let options = Options::from_request(req)?;
    let kind = if req.get("kind").and_then(Value::as_str) == Some("extern") {
        SourceKind::EXTERN
    } else {
        SourceKind::STRONG
    };
    let sf = Arc::new(SourceFile::new(name.clone(), code, kind));
    let static_sf: Arc<dyn StaticSourceFile> = sf.clone();
    let mut ast = Ast::new();
    let mut reports = RecordingReporter::default();
    let pr = ParserRunner::try_parse(
        &mut ast,
        static_sf.clone(),
        sf.code.clone(),
        &options.config,
        &mut reports,
    )
    .map_err(|e| {
        format!(
            "Exception parsing \"{name}\": {}: {}",
            e.class,
            e.message
                .as_ref()
                .map_or(String::new(), JsString::to_string_lossy)
        )
    })?;
    let root = pr.ast.unwrap_or_else(|| IR::script(&mut ast));
    root.set_static_source_file(&mut ast, Some(static_sf.clone()));
    root.set_input_id(&mut ast, Some(Arc::new(InputId::new(name.clone()))));
    // CompilerInput retains the first parse's features even when it substitutes an empty script.
    let features = pr.features;
    let second = ParserRunner::try_parse(
        &mut ast,
        static_sf,
        sf.code.clone(),
        &options.config,
        &mut NullErrorReporter,
    )
    .map_err(|e| {
        format!(
            "Exception parsing \"{name}\": {}: {}",
            e.class,
            e.message
                .as_ref()
                .map_or(String::new(), JsString::to_string_lossy)
        )
    })?;
    let dump = Dump {
        ast: &ast,
        source: &sf,
    };
    Ok(
        json!({"schema":"closure-rs/parse_dump/2","ok":true,"source_name":name,
        "language_in":options.language_in,"parser_config":format!("{:?}",options.config),
        "parser_reports":reports.reports,"features":feature_set(features),
        "source_map_url":second.source_map_url.as_ref().map(diagnostic_utf8),
        "comments":second.comments.iter().map(comment).collect::<Vec<_>>(),
        "top_level_statement_ranges":second.top_level_statement_ranges.iter().map(range).collect::<Vec<_>>(),
        "ast":dump.node(root)}),
    )
}
// port: OracleWorker#handleOnThisThread
fn handle(text: &str) -> Value {
    let request = match decode_request(text) {
        Ok(r) => r,
        Err(e) => return json!({"ok":false,"error":e}),
    };
    let result = catch_unwind(AssertUnwindSafe(|| run(&request)));
    let mut response = match result {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => json!({"ok":false,"error":e}),
        Err(p) => {
            let message = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "java.lang.RuntimeException".into());
            json!({"ok":false,"error":message})
        }
    };
    if let Some(id) = request.get("id") {
        response["id"] = id.clone();
    }
    response
}
fn serve(single: bool) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    if single {
        let mut request = String::new();
        stdin.lock().read_to_string(&mut request)?;
        serde_json::to_writer(&mut stdout, &handle(&request))?;
        writeln!(stdout)?;
        stdout.flush()?;
    } else {
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            serde_json::to_writer(&mut stdout, &handle(&line))?;
            writeln!(stdout)?;
            stdout.flush()?;
        }
    }
    Ok(())
}
fn main() {
    let single = std::env::args().skip(1).any(|a| a == "--request");
    // OracleWorker / CompilerExecutor similarly use a large stack for deep parser/AST recursion.
    std::thread::Builder::new()
        .name("parse_dump".into())
        .stack_size(1 << 30)
        .spawn(move || serve(single))
        .expect("start parser thread")
        .join()
        .expect("parser thread")
        .expect("parse_dump I/O");
}
