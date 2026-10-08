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

mod common;
use closure_cli::{
    abstract_command_line_runner::{AbstractCommandLineRunner, FlagEntry},
    java_io::{EncodedWriter, JavaAppendable, OutputWriter, StringBuilder},
};
use closure_rhino::{java_lang::charset::Charset, js_string::JsString};
use common::Capture;
use std::io::Write;

// Reference values: tools/Utf16OutputProbe.java calls Java's actual writeOutput overload.
#[test]
fn write_output_preserves_utf16_until_the_writer_encodes_it() {
    let base = AbstractCommandLineRunner::<
        closure_jscomp::compiler::Compiler,
        closure_jscomp::compiler_options::CompilerOptions,
    >::new(
        Box::new(std::io::empty()),
        Box::new(std::io::sink()),
        Box::new(std::io::sink()),
    );
    let code = JsString::from_units(vec![65, 0xd800, 66, 0xdc00, 67, 0xd83d, 0xde00]);
    let mut builder = StringBuilder::default();
    base.write_output_code(
        &mut builder,
        None,
        &code,
        "[%output%]",
        "%output%",
        None,
        "out.js",
    )
    .unwrap();
    assert_eq!(
        builder.into_js_string().as_units(),
        &[91, 65, 0xd800, 66, 0xdc00, 67, 0xd83d, 0xde00, 93, 10]
    );
    for (charset, expected) in [
        (Charset::UTF_8, "5b413f423f43f09f98805d0a"),
        (
            Charset::UTF_16BE,
            "005b0041fffd0042fffd0043d83dde00005d000a",
        ),
        (
            Charset::UTF_16LE,
            "5b004100fdff4200fdff43003dd800de5d000a00",
        ),
        (
            Charset::UTF_16,
            "feff005b0041fffd0042fffd0043d83dde00005d000a",
        ),
        (Charset::US_ASCII, "5b413f423f433f5d0a"),
        (Charset::ISO_8859_1, "5b413f423f433f5d0a"),
    ] {
        let output = Capture::default();
        let mut writer = EncodedWriter::new(Box::new(output.clone()), charset);
        base.write_output_code(
            &mut writer,
            None,
            &code,
            "[%output%]",
            "%output%",
            None,
            "out.js",
        )
        .unwrap();
        writer.close().unwrap();
        let expected: Vec<u8> = expected
            .as_bytes()
            .chunks_exact(2)
            .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(output.bytes(), expected, "{charset:?}");
    }
    let mut escaped = StringBuilder::default();
    base.write_output_code(
        &mut escaped,
        None,
        &code,
        "[%output%]",
        "%output%",
        Some(AbstractCommandLineRunner::<(), ()>::get_javascript_escaper),
        "out.js",
    )
    .unwrap();
    assert_eq!(
        escaped.into_js_string(),
        "[A\\ud800B\\udc00C\\ud83d\\ude00]\n"
    );
}

#[test]
fn encoded_writer_keeps_split_utf8_and_utf16_pairs_in_order() {
    let output = Capture::default();
    let mut writer = EncodedWriter::new(Box::new(output.clone()), Charset::UTF_8);
    writer.write_all(&[0xf0, 0x9f]).unwrap();
    writer.write_all(&[0x98, 0x80]).unwrap();
    writer.append(&JsString::from_units(vec![0xd83d])).unwrap();
    writer.append(&JsString::from_units(vec![0xde00])).unwrap();
    writer.write_all(b"end").unwrap();
    writer.close().unwrap();
    assert_eq!(output.text(), "😀😀end");
}

#[test]
fn flag_entry_uses_java_string_hash_and_wrapping_addition() {
    let entry = FlagEntry::new(i32::MAX, "😀".into());
    // String.hashCode: 31 * 0xd83d + 0xde00.
    assert_eq!(entry.hash_code(), i32::MAX.wrapping_add(1_772_899));
    assert_eq!(entry, FlagEntry::new(i32::MAX, "😀".into()));
    assert_ne!(entry, FlagEntry::new(0, "😀".into()));
    assert_ne!(entry, FlagEntry::new(i32::MAX, "a".into()));
}
