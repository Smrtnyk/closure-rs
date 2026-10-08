//! Literals: numbers (dtoa edge spellings, separators, radix forms, -0/NaN/huge values),
//! strings (escapes, lone surrogates, astral code points, line continuations), templates
//! (incl. tagged and `String.raw`), regexps with every accepted flag, booleans, null, BigInt.
//! Legacy octal forms are emitted only in the sloppy dialect (see `dialect.rs`).

use super::dialect;
use crate::engine::{Binding, Ctx, Gen, Production, always};

const NUMBERS: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "5",
    "7",
    "8",
    "10",
    "16",
    "31",
    "32",
    "42",
    "100",
    "255",
    "256",
    "1000",
    "65535",
    "65536",
    "0.5",
    "0.1",
    "0.2",
    "0.3",
    "1.5",
    "2.5",
    "4.35",
    "3.14159",
    "1e21",
    "1e20",
    "1e-6",
    "1e-7",
    "1e400",
    "123456789012345680000",
    "123456789012345678901234567890",
    "0x1f",
    "0xFF",
    "0XAbC",
    "0x7fffffffffffffff",
    "0o17",
    "0O777",
    "0b101",
    "0B11",
    "2147483647",
    "2147483648",
    "4294967295",
    "4294967296",
    "9007199254740991",
    "9007199254740992",
    "9007199254740993",
    "18446744073709551616",
    "5e-324",
    "2.2250738585072014e-308",
    "1.7976931348623157e308",
    "1.7976931348623159e308",
    ".5",
    "100.",
    "0.000001",
    "1.0",
    "1e+3",
    "1E3",
    "5e-7",
    "0.1e-1",
    "1_000",
    "1_000_000",
    "0.000_001",
    "1e1_0",
    "0xFF_FF",
    "0b1010_0101",
    "0o7_7",
    "123_456.789_012",
];

/// Special values written as expressions (each is a valid operand on its own).
const SPECIAL_NUMBERS: &[&str] = &[
    "-0",
    "NaN",
    "Infinity",
    "-Infinity",
    "0 / 0",
    "1 / -0",
    "-1",
    "-2147483648",
    "-9007199254740993",
    "-1e-7",
    "Number.MAX_SAFE_INTEGER",
    "Number.MIN_VALUE",
    "Number.EPSILON",
];

/// Sloppy only: legacy octal literals (`017`).
// (`08`/`09`/`019` are rejected by IRFactory in every mode: "Invalid octal digit".)
const LEGACY_NUMBERS: &[&str] = &["010", "017", "0777", "07", "00", "0377"];

const STRINGS: &[&str] = &[
    "''",
    "'a'",
    "'abc'",
    "\"x y\"",
    "'\\n'",
    "'\\t\\\\'",
    "'\\b\\f\\v\\r'",
    "'\\u00e9'",
    "'\\uD83D\\uDE00'",
    "'\\uD800'",
    "'\\uDC00'",
    "'a\\uD800b'",
    "'\\uDBFF\\uDFFF'",
    "'\\uDFFF\\uD800'",
    "'\\x41'",
    "'\\x00'",
    "'\\0'",
    "'\\u{1F600}'",
    "'\\u{10FFFF}'",
    "'\\u{0}'",
    "'\\u{D800}'",
    "'length'",
    "'prototype'",
    "'__proto__'",
    "'constructor'",
    "'toString'",
    "'0'",
    "'-1'",
    "'12'",
    "' 12 '",
    "'0x10'",
    "'1e3'",
    "'true'",
    "'undefined'",
    "'NaN'",
    "'\\u2028'",
    "'\\u2029'",
    "'\\uFEFF'",
    "'\"'",
    "\"'\"",
    "'\u{e9}'",
    "'na\u{ef}ve'",
    "'\u{1F600}'",
    "'\u{4e2d}\u{6587}'",
    "'\\\\'",
    "'a\\\nb'",
    "'</script>'",
    "'\\/'",
    "'\\a\\c'",
];

/// Sloppy only: legacy octal escapes and `\8`/`\9` (NonOctalDecimalEscapeSequence).
const LEGACY_STRINGS: &[&str] = &[
    "'\\101'", "'\\7'", "'\\08'", "'\\8'", "'\\9'", "'\\377'", "'a\\12b'",
];

const REGEXPS: &[&str] = &[
    "/a/",
    "/ab+c/g",
    "/^\\d+$/m",
    "/[a-z]/i",
    "/(x)(y)?/",
    "/\\s+/gu",
    "/./s",
    "/(?<year>\\d{4})-(?<month>\\d\\d)/u",
    "/(?<=\\$)\\d+/",
    "/(?<!a)b/g",
    "/\\p{L}+/u",
    "/\\P{Lu}/u",
    "/a/y",
    "/a/d",
    "/a/dgimsuy",
    "/[\\]\\/]/",
    "/\\//",
    "/\\u{1F600}/u",
    "/[^]/",
    "/\\bword\\b/",
    "/(?:a|b)*?/",
    "/a{2,3}/",
    "/\\cJ/",
    "/\\0/",
    "/=/",
    "/[\\uD800-\\uDBFF][\\uDC00-\\uDFFF]/",
    "/(a)\\1/",
    "/(?<n>a)\\k<n>/",
    "/^(?=x)x$/",
];

const BIGINTS: &[&str] = &[
    "0n",
    "1n",
    "2n",
    "123n",
    "0x10n",
    "0XFFn",
    "0o17n",
    "0b101n",
    "9007199254740993n",
    "18446744073709551616n",
    "1_000n",
    "123456789012345678901234567890n",
];

/// A number literal (sloppy dialect adds legacy octal forms).
pub fn number_lit(g: &mut Gen) {
    if dialect::sloppy() && !g.ctx.in_class_method && g.rng.chance(1, 8) {
        let n = *g.rng.pick(LEGACY_NUMBERS);
        g.w(n);
        return;
    }
    let n = *g.rng.pick(NUMBERS);
    g.w(n);
}

/// A string literal (sloppy dialect adds legacy escapes).
pub fn string_lit(g: &mut Gen) {
    if dialect::sloppy() && !g.ctx.in_class_method && g.rng.chance(1, 8) {
        let s = *g.rng.pick(LEGACY_STRINGS);
        g.w(s);
        return;
    }
    let s = *g.rng.pick(STRINGS);
    g.w(s);
}

fn number(g: &mut Gen) {
    number_lit(g);
}

fn special_number(g: &mut Gen) {
    let n = *g.rng.pick(SPECIAL_NUMBERS);
    g.w(n);
}

fn string(g: &mut Gen) {
    string_lit(g);
}

fn boolean(g: &mut Gen) {
    let s = if g.rng.chance(1, 2) { "true" } else { "false" };
    g.w(s);
}

fn nullish(g: &mut Gen) {
    let s = *g.rng.pick(&["null", "undefined", "void 0"]);
    g.w(s);
}

/// Regexps Java can transpile to ES5/ES2015: no ES2018+ syntax (`(?<` covers named groups
/// and lookbehind, `\p{`/`\P{` property escapes) and no `s`/`d` flags.
pub fn transpilable_regexp(r: &str) -> bool {
    let flags = &r[r.rfind('/').unwrap_or(0) + 1..];
    !r.contains("(?<")
        && !r.contains("\\p{")
        && !r.contains("\\P{")
        && !flags.contains('s')
        && !flags.contains('d')
}

fn regexp(g: &mut Gen) {
    let r = if dialect::low_target() {
        let ok: Vec<&str> = REGEXPS
            .iter()
            .copied()
            .filter(|r| transpilable_regexp(r))
            .collect();
        *g.rng.pick(&ok)
    } else {
        *g.rng.pick(REGEXPS)
    };
    g.w(r);
}

fn not_low_target(_: &Ctx) -> bool {
    !dialect::low_target()
}

fn bigint(g: &mut Gen) {
    let v = *g.rng.pick(BIGINTS);
    g.w(v);
}

const TEMPLATE_CHUNKS: &[&str] = &[
    "a",
    "b c",
    "",
    "\\n",
    "\\t",
    "\\u0041",
    "\\u{1F600}",
    "\\uD800",
    "\\x41",
    "\\`",
    "\\${",
    "$",
    "{}",
    "\u{e9}",
    "line1\nline2",
    "\\\\",
    "\\0",
];

fn template_chunk(g: &mut Gen) {
    let c = *g.rng.pick(TEMPLATE_CHUNKS);
    g.w(c);
}

/// `` `chunk${expr}chunk...` `` (substitutions are full expressions).
pub fn template_body(g: &mut Gen) {
    g.w("`");
    template_chunk(g);
    let n = g.rng.range(0, 3);
    for _ in 0..n {
        g.w("${");
        g.expr();
        g.w("}");
        template_chunk(g);
    }
    g.w("`");
}

fn template(g: &mut Gen) {
    template_body(g);
}

/// Tagged templates: `String.raw`, a visible function, or an inline tag. Raw strings keep
/// escapes; ES2018 allows invalid escapes in tagged templates.
fn tagged_template(g: &mut Gen) {
    match g.rng.below(4) {
        0 => g.w("String.raw"),
        1 => match g.visible(|b| b == Binding::Function) {
            Some(f) => g.w(&f),
            None => g.w("String.raw"),
        },
        2 => g.w("((s, ...v) => s.raw.join('|') + v.length)"),
        _ => g.w("((s) => s[0])"),
    }
    if g.rng.chance(1, 6) {
        g.w("`\\unicode and \\xZ`");
    } else {
        template_body(g);
    }
}

pub static EXPRESSIONS: &[Production] = &[
    Production {
        name: "number",
        weight: 30,
        leaf: true,
        when: always,
        emit: number,
    },
    Production {
        name: "special_number",
        weight: 4,
        leaf: true,
        when: always,
        emit: special_number,
    },
    Production {
        name: "string",
        weight: 18,
        leaf: true,
        when: always,
        emit: string,
    },
    Production {
        name: "boolean",
        weight: 6,
        leaf: true,
        when: always,
        emit: boolean,
    },
    Production {
        name: "nullish",
        weight: 4,
        leaf: true,
        when: always,
        emit: nullish,
    },
    Production {
        name: "regexp",
        weight: 3,
        leaf: true,
        when: always,
        emit: regexp,
    },
    Production {
        name: "bigint",
        weight: 2,
        leaf: true,
        when: not_low_target,
        emit: bigint,
    },
    Production {
        name: "template",
        weight: 4,
        leaf: false,
        when: always,
        emit: template,
    },
    Production {
        name: "tagged_template",
        weight: 2,
        leaf: false,
        when: always,
        emit: tagged_template,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_forms_are_not_in_strict_tables() {
        for n in NUMBERS {
            let t = n.replace('_', "");
            let b = t.as_bytes();
            let legacy = b.len() > 1 && b[0] == b'0' && b[1].is_ascii_digit();
            assert!(!legacy, "{n} is a legacy octal / leading-zero literal");
        }
        for s in STRINGS {
            for k in 1..=9u8 {
                let esc = format!("\\{}", k);
                assert!(
                    !s.contains(&esc) || s.contains("\\\\"),
                    "{s} has a legacy escape"
                );
            }
        }
    }

    #[test]
    fn regex_flags_are_accepted_by_closure() {
        // IRFactory.validateRegExpFlags accepts exactly g i m u y s d.
        for r in REGEXPS {
            let flags = &r[r.rfind('/').unwrap() + 1..];
            assert!(flags.chars().all(|c| "gimuysd".contains(c)), "{r}");
        }
    }
}
