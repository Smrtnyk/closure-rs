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

//! Differential check of the RegExpTree port against Java golden results.
//!
//! Usage: `cargo run -p closure-regex --example regex_diff -- <regex_golden.jsonl>`
//!
//! The golden file (format in `corpus-cache/regex/golden/README.md`) holds, per unique
//! (pattern, flags), what the reference `RegExpTree` produced: `toString()` of the parse tree, a
//! structural dump (`debugTree`), the predicates, `simplify(flags).toString()`, or the exception
//! class and message. Every field is compared byte for byte; exit code 1 on any mismatch.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::panic::{self, AssertUnwindSafe};

use closure_regex::reg_exp_tree::{RegExpException, RegExpTree};
use closure_rhino::js_string::JsString;
use serde_json::{Value, json};

fn unhex(s: &str) -> Vec<u16> {
    (0..s.len() / 4)
        .map(|i| u16::from_str_radix(&s[i * 4..i * 4 + 4], 16).unwrap())
        .collect()
}

fn hex(units: &[u16]) -> String {
    units.iter().map(|u| format!("{u:04x}")).collect()
}

fn class_name(t: &RegExpTree) -> &'static str {
    match t {
        RegExpTree::Empty(_) => "Empty",
        RegExpTree::Anchor(_) => "Anchor",
        RegExpTree::WordBoundary(_) => "WordBoundary",
        RegExpTree::BackReference(_) => "BackReference",
        RegExpTree::NamedBackReference(_) => "NamedBackReference",
        RegExpTree::Text(_) => "Text",
        RegExpTree::Repetition(_) => "Repetition",
        RegExpTree::Alternation(_) => "Alternation",
        RegExpTree::LookaheadAssertion(_) => "LookaheadAssertion",
        RegExpTree::LookbehindAssertion(_) => "LookbehindAssertion",
        RegExpTree::CapturingGroup(_) => "CapturingGroup",
        RegExpTree::NamedCaptureGroup(_) => "NamedCaptureGroup",
        RegExpTree::UnicodePropertyEscape(_) => "UnicodePropertyEscape",
        RegExpTree::Charset(_) => "Charset",
        RegExpTree::Concatenation(_) => "Concatenation",
    }
}

/// Same shape as the Java harness: SimpleName(debugInfo)[child,child,...]
fn debug_tree(t: &RegExpTree, sb: &mut Vec<u16>) {
    sb.extend(class_name(t).encode_utf16());
    sb.push(b'(' as u16);
    t.append_debug_info(sb);
    sb.extend(")[".encode_utf16());
    for (i, c) in t.children().into_iter().enumerate() {
        if i != 0 {
            sb.push(b',' as u16);
        }
        debug_tree(c, sb);
    }
    sb.push(b']' as u16);
}

fn exception_json(e: &RegExpException) -> Value {
    json!({
        "exception": e.class_name(),
        "message": e.get_message().map(|m| Value::String(hex(m.as_units()))).unwrap_or(Value::Null),
    })
}

fn panic_json(p: Box<dyn std::any::Any + Send>) -> Value {
    let msg = p
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    json!({"exception": "panic", "message": msg})
}

fn run(pattern: &JsString, flags: &JsString) -> (Value, Value) {
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        RegExpTree::parse_reg_exp(pattern, flags)
    }));
    let tree = match parsed {
        Err(p) => return (panic_json(p), Value::Null),
        Ok(Err(e)) => return (exception_json(&e), Value::Null),
        Ok(Ok(tree)) => tree,
    };
    let mut debug = Vec::new();
    debug_tree(&tree, &mut debug);
    let parse = json!({
        "ok": hex(tree.to_js_string().as_units()),
        "debug": hex(&debug),
        "caseSensitive": tree.is_case_sensitive(),
        "containsAnchor": tree.contains_anchor(),
        "numCapturingGroups": tree.num_capturing_groups(),
        "matchesWholeInput": RegExpTree::matches_whole_input(&tree, flags),
    });
    let simplify = match panic::catch_unwind(AssertUnwindSafe(|| tree.simplify(flags))) {
        Err(p) => panic_json(p),
        Ok(s) => {
            let mut debug = Vec::new();
            debug_tree(&s, &mut debug);
            json!({"ok": hex(s.to_js_string().as_units()), "debug": hex(&debug)})
        }
    };
    (parse, simplify)
}

fn show(v: &Value) -> String {
    match v {
        Value::String(s) if s.len() % 4 == 0 && s.chars().all(|c| c.is_ascii_hexdigit()) => {
            format!("{:?}", String::from_utf16_lossy(&unhex(s)))
        }
        Value::Object(m) => {
            let parts: Vec<String> = m.iter().map(|(k, v)| format!("{k}: {}", show(v))).collect();
            format!("{{{}}}", parts.join(", "))
        }
        other => other.to_string(),
    }
}

fn main() {
    panic::set_hook(Box::new(|_| {}));
    let path = std::env::args()
        .nth(1)
        .expect("usage: regex_diff <regex_golden.jsonl>");
    let file = std::fs::File::open(&path).expect("open golden file");
    let mut total = 0usize;
    let mut mismatched_cases = 0usize;
    let mut per_field: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_src: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut shown = 0;
    for line in std::io::BufReader::new(file).lines() {
        let line = line.unwrap();
        if line.is_empty() {
            continue;
        }
        let golden: Value = serde_json::from_str(&line).unwrap();
        total += 1;
        let pattern = JsString::from_units(unhex(golden["pattern"].as_str().unwrap()));
        let flags = JsString::from_units(unhex(golden["flags"].as_str().unwrap()));
        let src = golden["src"].as_str().unwrap_or("?").to_string();
        let (parse, simplify) = run(&pattern, &flags);
        let mut bad_fields = Vec::new();
        let expected_parse = &golden["parse"];
        if let (Value::Object(e), Value::Object(a)) = (expected_parse, &parse) {
            let keys: std::collections::BTreeSet<&String> = e.keys().chain(a.keys()).collect();
            for k in keys {
                if e.get(k) != a.get(k) {
                    bad_fields.push(format!("parse.{k}"));
                }
            }
        } else if expected_parse != &parse {
            bad_fields.push("parse".to_string());
        }
        if golden["simplify"] != simplify {
            bad_fields.push("simplify".to_string());
        }
        let entry = per_src.entry(src.clone()).or_default();
        entry.0 += 1;
        if !bad_fields.is_empty() {
            mismatched_cases += 1;
            entry.1 += 1;
            for f in &bad_fields {
                *per_field.entry(f.clone()).or_default() += 1;
            }
            if shown < 20 {
                shown += 1;
                println!(
                    "MISMATCH [{src}] pattern={:?} flags={:?} fields={bad_fields:?}",
                    pattern.to_string_lossy(),
                    flags.to_string_lossy()
                );
                println!("  expected parse:    {}", show(expected_parse));
                println!("  actual parse:      {}", show(&parse));
                println!("  expected simplify: {}", show(&golden["simplify"]));
                println!("  actual simplify:   {}", show(&simplify));
            }
        }
    }
    println!("cases: {total}, mismatched cases: {mismatched_cases}");
    for (src, (n, bad)) in &per_src {
        println!("  src {src}: {n} cases, {bad} mismatched");
    }
    for (field, n) in &per_field {
        println!("  field {field}: {n} mismatches");
    }
    if mismatched_cases != 0 {
        std::process::exit(1);
    }
}
