//! Unit tests for the core-ECMAScript productions (`lang/`). They check determinism,
//! dialect isolation, syntactic balance and feature coverage; Java parse acceptance is
//! measured separately with the oracle (`build/fuzz/lang/parse_rate.py`).

use super::dialect::{self, Dialect};
use crate::Config;

fn lang_only() -> Config {
    Config {
        closure: false,
        ..Config::default()
    }
}

fn corpus(d: Dialect, n: u64) -> Vec<String> {
    let cfg = lang_only();
    (0..n)
        .map(|i| dialect::generate_nth(9, i, &cfg, d))
        .collect()
}

/// Strip string, template and regex contents crudely enough for a bracket-balance check:
/// the generator never emits brackets inside literals except in the fixed tables, which
/// this skips.
fn balanced(src: &str) -> bool {
    let b = src.as_bytes();
    let mut stack = vec![];
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'\'' | b'"' => {
                i += 1;
                while i < b.len() && b[i] != c {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'`' => {
                // Templates may nest substitutions; treat `${` as an opening brace.
                stack.push(b'`');
            }
            b'(' | b'[' | b'{' => stack.push(c),
            b')' | b']' | b'}' => {
                let want = match c {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                // `}` may close a `${` inside a template.
                match stack.pop() {
                    Some(o) if o == want => {}
                    _ => return false,
                }
            }
            _ => {}
        }
        i += 1;
    }
    stack.iter().all(|&c| c == b'`')
}

#[test]
fn default_dialect_equals_plain_generator() {
    let cfg = lang_only();
    for i in 0..40 {
        assert_eq!(
            dialect::generate_nth(3, i, &cfg, Dialect::DEFAULT),
            crate::generate_nth(3, i, &cfg)
        );
    }
}

#[test]
fn dialect_is_restored_after_generation() {
    let cfg = lang_only();
    let _ = dialect::generate_nth(1, 0, &cfg, Dialect::SLOPPY);
    assert_eq!(dialect::current(), Dialect::DEFAULT);
    let r = std::panic::catch_unwind(|| {
        dialect::with_dialect(Dialect::UNSUPPORTED, || panic!("boom"));
    });
    assert!(r.is_err());
    assert_eq!(dialect::current(), Dialect::DEFAULT);
}

#[test]
fn deterministic_per_dialect() {
    let cfg = lang_only();
    for d in [Dialect::DEFAULT, Dialect::SLOPPY, Dialect::UNSUPPORTED] {
        for i in 0..30 {
            assert_eq!(
                dialect::generate_nth(77, i, &cfg, d),
                dialect::generate_nth(77, i, &cfg, d)
            );
        }
    }
}

#[test]
fn default_dialect_has_no_sloppy_or_private_syntax() {
    for p in corpus(Dialect::DEFAULT, 400) {
        assert!(!p.contains("with ("), "{p}");
        assert!(!p.contains(" #"), "{p}");
        assert!(!p.contains("delete v"), "{p}");
        for legacy in ["'\\101'", "'\\8'", " 017", " 0777"] {
            assert!(!p.contains(legacy), "{legacy} in {p}");
        }
    }
}

#[test]
fn dialects_emit_their_features() {
    let sloppy = corpus(Dialect::SLOPPY, 400).concat();
    assert!(sloppy.contains("with ("));
    let uns = corpus(Dialect::UNSUPPORTED, 400).concat();
    assert!(uns.contains("#q") || uns.contains("#pm"));
    assert_eq!(Dialect::SLOPPY.flags(), vec!["--strict_mode_input=false"]);
    assert!(Dialect::UNSUPPORTED.flags().is_empty());
}

#[test]
fn brackets_balance() {
    for d in [Dialect::DEFAULT, Dialect::SLOPPY, Dialect::UNSUPPORTED] {
        for (i, p) in corpus(d, 300).iter().enumerate() {
            // Regex and template tables contain brackets in escaped positions; skip programs
            // that use them so the crude scanner stays sound.
            if p.contains('/') || p.contains('`') {
                continue;
            }
            assert!(balanced(p), "dialect {} program {i}:\n{p}", d.name());
        }
    }
}

#[test]
fn covers_core_features() {
    let all = corpus(Dialect::DEFAULT, 600).concat();
    for needle in [
        "class ",
        "static {",
        "extends ",
        "super",
        "get ",
        "set ",
        "function*",
        "async ",
        "await ",
        "yield",
        "for await (",
        "=> ",
        "...",
        "?.",
        "??",
        "&&=",
        "||=",
        "??=",
        "**",
        "`",
        "String.raw`",
        "n,",
        "_",
        "/g",
        "switch ",
        "default:",
        "try ",
        "catch {",
        "finally ",
        "throw ",
        "LL",
        "continue",
        "break",
        "do ",
        "while (",
        "for (const ",
        " of ",
        " in ",
        "new ",
        "typeof ",
        "void ",
        "delete ",
        "debugger;",
        "'use strict';",
        "-0",
        "1e400",
        "\\uD800",
        "\\u{",
        "0b",
        "0o",
        "0x",
        "arguments",
        "new.target",
    ] {
        assert!(all.contains(needle), "no program contains {needle:?}");
    }
}

#[test]
fn programs_reference_and_call_what_they_declare() {
    // Semantic richness: declared names are used later, and functions are called.
    let cfg = lang_only();
    let mut calls = 0;
    let mut used = 0;
    let n = 300;
    for i in 0..n {
        let p = crate::generate_nth(5, i, &cfg);
        let declared: Vec<&str> = p
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| {
                t.len() > 1 && t.starts_with('f') && t[1..].chars().all(|c| c.is_ascii_digit())
            })
            .collect();
        if declared
            .iter()
            .any(|f| p.matches(&format!("{f}(")).count() >= 2)
        {
            calls += 1;
        }
        if p.matches("v").count() > 2 {
            used += 1;
        }
    }
    assert!(
        calls * 3 > n,
        "only {calls}/{n} programs call a declared function"
    );
    assert!(used * 2 > n);
}

#[test]
fn depth_cap_bounds_output() {
    let cfg = Config {
        max_depth: 64,
        closure: false,
        ..Config::default()
    };
    for i in 0..20 {
        let p = crate::generate_nth(11, i, &cfg);
        assert!(p.len() < 4_000_000, "program {i} is {} bytes", p.len());
    }
}
