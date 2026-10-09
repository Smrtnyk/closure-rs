//! Structure-aware mutator over visible D2 inputs (docs/PORTING.md §4.6).
//!
//! Inputs come from `corpus/d2/cases.jsonl` (visible cases only; the holdout is never in the
//! repository, D-008). [`syntax::analyze`] recovers statement lists, conditions and
//! expressions from a token stream, and the operators below edit them so that the result
//! stays parseable by construction:
//!
//! | operator | edit |
//! |---|---|
//! | `literal` | number, BigInt, string, boolean/null or regexp literal replaced (same class) |
//! | `operator` | binary, logical, compound-assignment, update or `typeof`/`void` operator swapped within its class |
//! | `ident` | identifier replaced by another identifier of the same program |
//! | `expr-splice` | a condition, return/throw argument, expression statement or rvalue replaced by a donor expression |
//! | `stmt-splice` | a donor statement inserted into a statement list whose context allows it |
//! | `stmt-remove`, `stmt-dup`, `stmt-swap` | statement list edits |
//! | `wrap` | 1-3 statements wrapped in a block, `if (true)`, `try/finally`, a label, `do/while(false)`, or a function/arrow IIFE |
//! | `strict` | `'use strict'` directive added or removed (program or simple-parameter function) |
//!
//! Donor statements and expressions come from other visible D2 inputs ([`Donors`]); only
//! portable ones are kept (no `yield`/`await`/`super`/`import`/`export`/`new.target`/private
//! names, no `break`/`continue` that would leave their loop, `return` only into functions).
//! After every edit the result is re-analyzed, and an edit that breaks the lexical structure
//! or adds a parser anomaly is discarded and another one is drawn.
//!
//! Deterministic: [`mutate`] and [`mutate_with`] are pure functions of their arguments.

#![forbid(unsafe_code)]

pub mod lex;
#[path = "../../references.rs"]
pub mod references;
pub mod syntax;

use jsgen::rng::Rng;
use lex::{CONTEXTUAL, Kind, ends_expr};
use serde_json::Value;
use std::path::{Path, PathBuf};
use syntax::{Analysis, ListKind, SKind, analyze};

const NUMBERS: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "7",
    "8",
    "10",
    "16",
    "31",
    "32",
    "64",
    "255",
    "256",
    "1000",
    "0.5",
    "1.5",
    "1e21",
    "1e-7",
    "4294967295",
    "4294967296",
    "2147483647",
    "2147483648",
    "9007199254740992",
    "9007199254740993",
    "0x7fffffff",
    "0xff",
    "0b101",
    "0o17",
    "1e308",
    "5e-324",
];
const BIGINTS: &[&str] = &["0n", "1n", "2n", "255n", "9007199254740993n", "0x10n"];
const STRINGS: &[&str] = &[
    "''",
    "'a'",
    "'length'",
    "'toString'",
    "'0'",
    "'-0'",
    "'\\uD800'",
    "'\\n'",
    "'use strict'",
    "'undefined'",
    "'\\u00e9\\u{1F600}'",
];
const REGEXES: &[&str] = &["/a/", "/^\\s+$/g", "/[a-z]+/i", "/(\\d+)-(\\d+)/", "/x*/y"];
const ARITH: &[&str] = &[
    "+", "-", "*", "/", "%", "&", "|", "^", "<<", ">>", ">>>", "<", ">", "<=", ">=", "==", "!=",
    "===", "!==",
];
const LOGIC: &[&str] = &["&&", "||"];
const COMPOUND: &[&str] = &[
    "+=", "-=", "*=", "/=", "%=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "**=",
];
/// Names that must never become a class member or object key by accident.
const BAD_KEYS: &[&str] = &["constructor", "__proto__", "prototype"];
/// Tokens that tie code to its enclosing function/class/module.
const NON_PORTABLE: &[&str] = &["yield", "await", "super", "import", "export"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Op {
    Literal,
    Operator,
    Ident,
    ExprSplice,
    StmtSplice,
    StmtRemove,
    StmtDup,
    StmtSwap,
    Wrap,
    Strict,
}

impl Op {
    pub const ALL: [Op; 10] = [
        Op::Literal,
        Op::Operator,
        Op::Ident,
        Op::ExprSplice,
        Op::StmtSplice,
        Op::StmtRemove,
        Op::StmtDup,
        Op::StmtSwap,
        Op::Wrap,
        Op::Strict,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Op::Literal => "literal",
            Op::Operator => "operator",
            Op::Ident => "ident",
            Op::ExprSplice => "expr-splice",
            Op::StmtSplice => "stmt-splice",
            Op::StmtRemove => "stmt-remove",
            Op::StmtDup => "stmt-dup",
            Op::StmtSwap => "stmt-swap",
            Op::Wrap => "wrap",
            Op::Strict => "strict",
        }
    }

    fn weight(self) -> u64 {
        match self {
            Op::Literal => 12,
            Op::Operator => 12,
            Op::Ident => 8,
            Op::ExprSplice => 12,
            Op::StmtSplice => 14,
            Op::StmtRemove => 10,
            Op::StmtDup => 8,
            Op::StmtSwap => 6,
            Op::Wrap => 12,
            Op::Strict => 6,
        }
    }
}

#[derive(Clone, Debug)]
struct DonorStmt {
    text: String,
    has_return: bool,
}

/// Material spliced into mutants: portable statements, expressions, strings and regexps.
#[derive(Clone, Debug, Default)]
pub struct Donors {
    stmts: Vec<DonorStmt>,
    exprs: Vec<String>,
    strings: Vec<String>,
    regexes: Vec<String>,
}

impl Donors {
    pub fn from_sources<S: AsRef<str>>(srcs: &[S]) -> Donors {
        let mut d = Donors::default();
        for s in srcs {
            d.add(s.as_ref());
        }
        d
    }

    pub fn len(&self) -> (usize, usize, usize, usize) {
        (
            self.stmts.len(),
            self.exprs.len(),
            self.strings.len(),
            self.regexes.len(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty() && self.exprs.is_empty()
    }

    /// Adds the portable parts of one program (nothing if its structure is unclear).
    pub fn add(&mut self, src: &str) {
        let a = analyze(src);
        if !a.clean() {
            return;
        }
        let depth = fn_depths(&a);
        for s in &a.stmts {
            if s.end - s.start > 1500
                || matches!(s.kind, SKind::Import | SKind::Export | SKind::Empty)
            {
                continue;
            }
            if let Some(has_return) = portable(&a, src, s.t0, s.t1, s.fn_depth, &depth, s.kind) {
                self.stmts.push(DonorStmt {
                    text: src[s.start..s.end].to_string(),
                    has_return,
                });
            }
        }
        let mut ranges: Vec<(usize, usize)> = a.conds.iter().map(|&(o, c)| (o + 1, c)).collect();
        ranges.extend(a.exprs.iter().copied());
        for (x, y) in ranges {
            if y <= x {
                continue;
            }
            let (st, en) = (a.toks()[x].start, a.toks()[y - 1].end);
            if en - st > 300 || !expr_portable(&a, src, x, y) {
                continue;
            }
            self.exprs.push(src[st..en].to_string());
        }
        for t in a.toks() {
            let text = &src[t.start..t.end];
            match t.kind {
                Kind::Str if text.len() <= 40 && !BAD_KEYS.contains(&&text[1..text.len() - 1]) => {
                    self.strings.push(text.to_string())
                }
                Kind::Regex if text.len() <= 60 => self.regexes.push(text.to_string()),
                _ => {}
            }
        }
    }
}

fn tok_text<'s>(a: &Analysis, src: &'s str, i: usize) -> &'s str {
    a.toks().get(i).map(|t| &src[t.start..t.end]).unwrap_or("")
}

/// Function-body nesting depth of every token.
fn fn_depths(a: &Analysis) -> Vec<u32> {
    let n = a.toks().len();
    let mut delta = vec![0i32; n + 1];
    for &(o, c) in &a.fn_bodies {
        delta[o + 1] += 1;
        delta[c] -= 1;
    }
    let mut d = 0i32;
    (0..n)
        .map(|i| {
            d += delta[i];
            d.max(0) as u32
        })
        .collect()
}

/// `true` if tokens `[x, y)` contain nothing tied to their surroundings (see module docs).
fn expr_portable(a: &Analysis, src: &str, x: usize, y: usize) -> bool {
    (x..y).all(|i| {
        let t = &a.toks()[i];
        let w = &src[t.start..t.end];
        t.kind != Kind::Private
            && !(t.kind == Kind::Keyword && NON_PORTABLE.contains(&w))
            && !(w == "new" && tok_text(a, src, i + 1) == ".")
            && !(t.kind == Kind::Ident && (w == "arguments" || w == "await" || w == "yield"))
    })
}

/// Portability of statement tokens `[t0, t1)`: `None` if not portable, else whether it has
/// a `return` of the enclosing function.
fn portable(
    a: &Analysis,
    src: &str,
    t0: usize,
    t1: usize,
    depth0: u32,
    depth: &[u32],
    kind: SKind,
) -> Option<bool> {
    if !expr_portable(a, src, t0, t1) {
        return None;
    }
    let mut has_return = false;
    for (i, &d) in depth.iter().enumerate().take(t1).skip(t0) {
        if d != depth0 || a.toks()[i].kind != Kind::Keyword {
            continue;
        }
        match tok_text(a, src, i) {
            "return" => has_return = true,
            "break" | "continue" => {
                let labelled = a
                    .toks()
                    .get(i + 1)
                    .is_some_and(|t| t.kind == Kind::Ident && !t.nl_before);
                if labelled || !matches!(kind, SKind::Loop | SKind::Switch) {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(has_return)
}

/// Standard globals that are defined under every D2 profile (default browser externs), so a
/// donor may use them freely.
const KNOWN_GLOBALS: &[&str] = &[
    "Array",
    "ArrayBuffer",
    "BigInt",
    "Boolean",
    "DataView",
    "Date",
    "Error",
    "EvalError",
    "Float32Array",
    "Float64Array",
    "Function",
    "Infinity",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "JSON",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "Proxy",
    "RangeError",
    "ReferenceError",
    "Reflect",
    "RegExp",
    "Set",
    "String",
    "Symbol",
    "SyntaxError",
    "TypeError",
    "URIError",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "WeakMap",
    "WeakSet",
    "clearInterval",
    "clearTimeout",
    "console",
    "decodeURIComponent",
    "document",
    "encodeURIComponent",
    "globalThis",
    "isFinite",
    "isNaN",
    "parseFloat",
    "parseInt",
    "setInterval",
    "setTimeout",
    "undefined",
    "window",
];

/// Identifiers a donor snippet uses but does not itself bind (approximate, see below).
///
/// Uses: identifier tokens not after `.`/`?.` and not object keys (`{k:` / `,k:`) or labels.
/// Bindings: names after `var`/`let`/`const`/`function`/`class`, names in parameter lists
/// (function, method and arrow parentheses, single-parameter arrows), `catch (e)`, and names
/// inside a destructuring pattern that directly follows `var`/`let`/`const`.
/// Over-approximating the uses only makes the splice filter stricter.
fn donor_free_names(text: &str) -> Vec<String> {
    let lx = lex::lex(text);
    if !lx.ok {
        return vec!["<unlexable>".into()];
    }
    let t = &lx.toks;
    let w = |i: usize| -> &str { t.get(i).map(|k| &text[k.start..k.end]).unwrap_or("") };
    let n = t.len();
    // Matching brackets.
    let mut close = vec![usize::MAX; n];
    let mut st = vec![];
    for (i, tok) in t.iter().enumerate() {
        if tok.kind != Kind::Punct {
            continue;
        }
        match w(i) {
            "(" | "[" | "{" | "${" => st.push(i),
            ")" | "]" | "}" => {
                if let Some(o) = st.pop() {
                    close[o] = i;
                }
            }
            _ => {}
        }
    }
    let mut bound = std::collections::BTreeSet::new();
    let mark_range = |bound: &mut std::collections::BTreeSet<String>, a: usize, b: usize| {
        // Idents in [a, b) that are not object keys of a pattern (`k:`) and not defaults' rvalues
        // (approximation: an ident right after `=` inside the range is a use, not a binding).
        for (off, tok) in t[a..b].iter().enumerate() {
            let j = a + off;
            if tok.kind == Kind::Ident && w(j + 1) != ":" && w(j.wrapping_sub(1)) != "=" {
                bound.insert(w(j).to_string());
            }
        }
    };
    for i in 0..n {
        let x = w(i);
        if t[i].kind == Kind::Keyword && matches!(x, "var" | "let" | "const" | "function" | "class")
        {
            let mut j = i + 1;
            if w(j) == "*" {
                j += 1;
            }
            if t.get(j).is_some_and(|k| k.kind == Kind::Ident) {
                bound.insert(w(j).to_string());
            } else if matches!(w(j), "{" | "[") && close[j] != usize::MAX {
                mark_range(&mut bound, j + 1, close[j]);
            }
        }
        if (x == "let" || x == "const" || x == "var") && t[i].kind != Kind::Ident {
            // `let a = 1, b = 2`: names after top-level commas of the declaration.
            let mut depth = 0i32;
            let mut j = i + 1;
            while j < n && !(depth == 0 && w(j) == ";") {
                match w(j) {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => {
                        depth -= 1;
                        if depth < 0 {
                            break;
                        }
                    }
                    "," if depth == 0 && t.get(j + 1).is_some_and(|k| k.kind == Kind::Ident) => {
                        bound.insert(w(j + 1).to_string());
                    }
                    _ => {}
                }
                if t[j].nl_before && depth == 0 && j > i + 1 && w(j - 1) != "," && w(j) != "," {
                    break;
                }
                j += 1;
            }
        }
        if x == "(" && close[i] != usize::MAX {
            let c = close[i];
            let prev = w(i.wrapping_sub(1));
            let prev_kind = t.get(i.wrapping_sub(1)).map(|k| k.kind);
            let is_params = w(c + 1) == "=>"
                || prev == "function"
                || prev == "catch"
                || (prev_kind == Some(Kind::Ident)
                    && (w(i.wrapping_sub(2)) == "function"
                        || w(i.wrapping_sub(2)) == "*"
                        || w(c + 1) == "{"));
            if is_params {
                mark_range(&mut bound, i + 1, c);
            }
        }
        if t[i].kind == Kind::Ident && w(i + 1) == "=>" {
            bound.insert(x.to_string());
        }
    }
    let mut free = std::collections::BTreeSet::new();
    for (i, tok) in t.iter().enumerate() {
        if tok.kind != Kind::Ident {
            continue;
        }
        let x = w(i);
        let prev = w(i.wrapping_sub(1));
        if prev == "." || prev == "?." {
            continue;
        }
        if w(i + 1) == ":" && (matches!(prev, "{" | "," | ";" | "") || i == 0) {
            continue; // object key or label
        }
        if bound.contains(x) || KNOWN_GLOBALS.contains(&x) || x == "arguments" {
            continue;
        }
        free.insert(x.to_string());
    }
    free.into_iter().collect()
}

/// Identifier names of the host program.
fn host_names(a: &Analysis, src: &str) -> std::collections::BTreeSet<String> {
    a.toks()
        .iter()
        .filter(|t| t.kind == Kind::Ident)
        .map(|t| src[t.start..t.end].to_string())
        .collect()
}

/// A donor may be spliced only if every name it uses without binding is a known global or a
/// name the host already uses (donors brought free names such as `trackSelf`
/// into hosts, which made ADVANCED fail with JSC_UNDEFINED_VARIABLE).
fn donor_fits(text: &str, host: &std::collections::BTreeSet<String>) -> bool {
    donor_free_names(text).iter().all(|n| host.contains(n))
}

fn ensure_semi(s: &str) -> String {
    if s.ends_with(';') {
        s.to_string()
    } else {
        format!("{s};")
    }
}

/// Prefix that keeps an inserted statement from continuing the previous one (ASI).
fn guard(s: &str) -> &'static str {
    match s.as_bytes().first() {
        Some(b'(' | b'[' | b'`' | b'+' | b'-' | b'/' | b'*' | b'<') => ";",
        _ => "",
    }
}

fn splice(src: &str, start: usize, end: usize, with: &str) -> String {
    let mut s = String::with_capacity(src.len() + with.len());
    s.push_str(&src[..start]);
    s.push_str(with);
    s.push_str(&src[end..]);
    s
}

fn pick_idx(rng: &mut Rng, n: usize) -> Option<usize> {
    (n > 0).then(|| rng.below(n as u64) as usize)
}

struct Cx<'a> {
    src: &'a str,
    a: &'a Analysis,
    donors: &'a Donors,
}

impl Cx<'_> {
    fn text(&self, i: usize) -> &str {
        tok_text(self.a, self.src, i)
    }

    fn kind(&self, i: usize) -> Option<Kind> {
        self.a.toks().get(i).map(|t| t.kind)
    }

    /// Previous token ends an operand (so the token at `i` is a binary operator).
    fn binary_at(&self, i: usize) -> bool {
        if i == 0 {
            return false;
        }
        let p = &self.a.toks()[i - 1];
        let w = &self.src[p.start..p.end];
        ends_expr(p.kind, w)
            && w != "}"
            && !(p.kind == Kind::Ident && matches!(w, "async" | "get" | "set" | "static" | "of"))
    }

    fn reliable_lists(&self) -> Vec<usize> {
        (0..self.a.lists.len())
            .filter(|&l| self.a.lists[l].reliable)
            .collect()
    }

    fn literal(&self, rng: &mut Rng) -> Option<String> {
        let c: Vec<usize> = (0..self.a.toks().len())
            .filter(|&i| match self.kind(i) {
                Some(Kind::Number | Kind::BigInt | Kind::Str | Kind::Regex) => true,
                Some(Kind::Keyword) => matches!(self.text(i), "true" | "false" | "null"),
                _ => false,
            })
            .collect();
        let i = c[pick_idx(rng, c.len())?];
        let t = &self.a.toks()[i];
        let old = &self.src[t.start..t.end];
        let mut repl = match t.kind {
            Kind::Number => rng.pick(NUMBERS).to_string(),
            Kind::BigInt => rng.pick(BIGINTS).to_string(),
            Kind::Str => {
                if !self.donors.strings.is_empty() && rng.chance(1, 2) {
                    rng.pick(&self.donors.strings).clone()
                } else {
                    rng.pick(STRINGS).to_string()
                }
            }
            Kind::Regex => {
                if !self.donors.regexes.is_empty() && rng.chance(1, 2) {
                    rng.pick(&self.donors.regexes).clone()
                } else {
                    rng.pick(REGEXES).to_string()
                }
            }
            _ => rng.pick(&["true", "false", "null"]).to_string(),
        };
        if repl == old {
            return None;
        }
        if self.text(i + 1) == "." && t.kind == Kind::Number {
            repl.push(' ');
        }
        Some(splice(self.src, t.start, t.end, &repl))
    }

    fn operator(&self, rng: &mut Rng) -> Option<String> {
        let c: Vec<(usize, &[&str])> = (0..self.a.toks().len())
            .filter_map(|i| {
                let w = self.text(i);
                match self.kind(i)? {
                    Kind::Punct if (ARITH.contains(&w) || w == "**") && self.binary_at(i) => {
                        Some((i, ARITH))
                    }
                    Kind::Punct if LOGIC.contains(&w) && self.binary_at(i) => Some((i, LOGIC)),
                    Kind::Punct if COMPOUND.contains(&w) => Some((i, COMPOUND)),
                    Kind::Punct if w == "++" || w == "--" => Some((i, &["++", "--"][..])),
                    Kind::Keyword if w == "typeof" || w == "void" => {
                        Some((i, &["typeof", "void"][..]))
                    }
                    _ => None,
                }
            })
            .collect();
        let (i, class) = c[pick_idx(rng, c.len())?];
        let t = &self.a.toks()[i];
        let repl = *rng.pick(class);
        if repl == &self.src[t.start..t.end] {
            return None;
        }
        Some(splice(self.src, t.start, t.end, &format!(" {repl} ")))
    }

    fn ident_ok(&self, i: usize) -> bool {
        self.kind(i) == Some(Kind::Ident)
            && !CONTEXTUAL.contains(&self.text(i))
            && self.text(i + 1) != ":"
            && !(i > 0 && matches!(self.text(i - 1), "break" | "continue"))
    }

    fn ident(&self, rng: &mut Rng) -> Option<String> {
        let c: Vec<usize> = (0..self.a.toks().len())
            .filter(|&i| self.ident_ok(i))
            .collect();
        let mut names: Vec<&str> = c.iter().map(|&i| self.text(i)).collect();
        names.sort_unstable();
        names.dedup();
        let i = c[pick_idx(rng, c.len())?];
        let repl = *rng.pick(&names);
        if repl == self.text(i) {
            return None;
        }
        let t = &self.a.toks()[i];
        Some(splice(self.src, t.start, t.end, repl))
    }

    fn expr_splice(&self, rng: &mut Rng) -> Option<String> {
        let host = host_names(self.a, self.src);
        let exprs = self.donors_exprs()?;
        let mut donor = None;
        for _ in 0..12 {
            let c = rng.pick(exprs);
            if donor_fits(c, &host) {
                donor = Some(c.clone());
                break;
            }
        }
        let donor = donor?;
        // Targets: conditions, expressions, rvalue operands.
        let mut targets: Vec<(usize, usize, bool)> = self
            .a
            .conds
            .iter()
            .map(|&(o, c)| (self.a.toks()[o + 1].start, self.a.toks()[c - 1].end, false))
            .collect();
        for &(x, y) in &self.a.exprs {
            targets.push((self.a.toks()[x].start, self.a.toks()[y - 1].end, true));
        }
        const PREV: &[&str] = &[
            "=", "return", "+", "-", "*", "%", "==", "===", "!=", "!==", "<", ">", "<=", ">=",
            "&&", "||", "?", "+=", "-=", "*=", "|=", "&=",
        ];
        const NEXT: &[&str] = &[";", ")", "]", ",", "}", "+", "-", "*", "%", "&&", "||", "?"];
        for i in 1..self.a.toks().len() {
            let k = self.kind(i);
            let operand = matches!(k, Some(Kind::Number | Kind::Str | Kind::Ident))
                || (k == Some(Kind::Keyword) && matches!(self.text(i), "true" | "false" | "null"));
            if operand
                && PREV.contains(&self.text(i - 1))
                && NEXT.contains(&self.text(i + 1))
                && !(k == Some(Kind::Ident) && CONTEXTUAL.contains(&self.text(i)))
            {
                let t = &self.a.toks()[i];
                targets.push((t.start, t.end, true));
            }
        }
        let (s, e, paren) = targets[pick_idx(rng, targets.len())?];
        let repl = if paren { format!("({donor})") } else { donor };
        Some(splice(self.src, s, e, &repl))
    }

    fn donors_exprs(&self) -> Option<&[String]> {
        (!self.donors.exprs.is_empty()).then_some(&self.donors.exprs[..])
    }

    fn stmt_splice(&self, rng: &mut Rng) -> Option<String> {
        let lists = self.reliable_lists();
        let l = &self.a.lists[lists[pick_idx(rng, lists.len())?]];
        let host = host_names(self.a, self.src);
        let mut d = None;
        for _ in 0..12 {
            let c = rng.pick(self.donors.stmts.get(..).filter(|s| !s.is_empty())?);
            if (!c.has_return || l.ctx.in_function) && donor_fits(&c.text, &host) {
                d = Some(c);
                break;
            }
        }
        let d = d?;
        let p = rng.below(l.stmts.len() as u64 + 1) as usize;
        let at = if p == 0 {
            l.open
        } else {
            self.a.stmts[l.stmts[p - 1]].end
        };
        let ins = format!("\n{}{}\n", guard(&d.text), ensure_semi(&d.text));
        Some(splice(self.src, at, at, &ins))
    }

    fn pick_stmt(&self, rng: &mut Rng, min: usize) -> Option<(usize, usize)> {
        let lists: Vec<usize> = self
            .reliable_lists()
            .into_iter()
            .filter(|&l| self.a.lists[l].stmts.len() >= min)
            .collect();
        let l = lists[pick_idx(rng, lists.len())?];
        let k = rng.below((self.a.lists[l].stmts.len() + 1 - min) as u64) as usize;
        Some((l, k))
    }

    fn stmt_remove(&self, rng: &mut Rng) -> Option<String> {
        let (l, k) = self.pick_stmt(rng, 1)?;
        let s = &self.a.stmts[self.a.lists[l].stmts[k]];
        Some(splice(self.src, s.start, s.end, ""))
    }

    fn stmt_dup(&self, rng: &mut Rng) -> Option<String> {
        let (l, k) = self.pick_stmt(rng, 1)?;
        let list = &self.a.lists[l];
        let s = &self.a.stmts[list.stmts[k]];
        let ok = match s.kind {
            SKind::Import | SKind::Export | SKind::Lexical | SKind::Class | SKind::Empty => false,
            SKind::Function => matches!(list.kind, ListKind::Program | ListKind::FunctionBody),
            _ => true,
        };
        if !ok {
            return None;
        }
        let text = &self.src[s.start..s.end];
        let ins = format!("\n{}{}", guard(text), ensure_semi(text));
        let base = if text.ends_with(';') {
            String::new()
        } else {
            ";".to_string()
        };
        Some(splice(self.src, s.end, s.end, &format!("{base}{ins}")))
    }

    fn stmt_swap(&self, rng: &mut Rng) -> Option<String> {
        let (l, k) = self.pick_stmt(rng, 2)?;
        let list = &self.a.lists[l];
        let (s1, s2) = (
            &self.a.stmts[list.stmts[k]],
            &self.a.stmts[list.stmts[k + 1]],
        );
        let t1 = &self.src[s1.start..s1.end];
        let t2 = &self.src[s2.start..s2.end];
        let between = &self.src[s1.end..s2.start];
        let repl = format!(
            "{}{}{between}{}{}",
            guard(t2),
            ensure_semi(t2),
            guard(t1),
            ensure_semi(t1)
        );
        Some(splice(self.src, s1.start, s2.end, &repl))
    }

    fn wrap(&self, rng: &mut Rng) -> Option<String> {
        let (l, k) = self.pick_stmt(rng, 1)?;
        let list = &self.a.lists[l];
        let len = (1 + rng.below(3) as usize).min(list.stmts.len() - k);
        let ss: Vec<&syntax::Stmt> = list.stmts[k..k + len]
            .iter()
            .map(|&s| &self.a.stmts[s])
            .collect();
        if ss
            .iter()
            .any(|s| matches!(s.kind, SKind::Import | SKind::Export))
        {
            return None;
        }
        let depth = fn_depths(self.a);
        let (t0, t1) = (ss[0].t0, ss[len - 1].t1);
        let mut fn_ok = true;
        for (i, &d) in depth.iter().enumerate().take(t1).skip(t0) {
            let w = self.text(i);
            if self.kind(i) == Some(Kind::Keyword) {
                if matches!(w, "yield" | "await" | "super") {
                    fn_ok = false;
                }
                if d == list.ctx.fn_depth && matches!(w, "break" | "continue") {
                    fn_ok = false;
                }
            }
            if w == "new" && self.text(i + 1) == "." {
                fn_ok = false;
            }
            if self.kind(i) == Some(Kind::Ident) && (w == "await" || w == "yield") {
                fn_ok = false;
            }
        }
        let body = &self.src[ss[0].start..ss[len - 1].end];
        let forms: &[(&str, &str)] = &[
            ("{\n", "\n}"),
            ("if (true) {\n", "\n}"),
            ("try {\n", "\n} finally {}"),
            ("LABEL: {\n", "\n}"),
            ("do {\n", "\n} while (false);"),
            (";(function () {\n", "\n})();"),
            (";(() => {\n", "\n})();"),
        ];
        let n = if fn_ok { forms.len() } else { forms.len() - 2 };
        let (pre, post) = forms[rng.below(n as u64) as usize];
        // A fresh label each time: nested duplicates are a SyntaxError.
        let pre = pre.replace("LABEL", &format!("w{}", rng.below(1 << 30)));
        Some(splice(
            self.src,
            ss[0].start,
            ss[len - 1].end,
            &format!("{pre}{body}{post}"),
        ))
    }

    fn strict(&self, rng: &mut Rng) -> Option<String> {
        let c: Vec<usize> = (0..self.a.lists.len())
            .filter(|&l| {
                let x = &self.a.lists[l];
                x.reliable
                    && (x.kind == ListKind::Program
                        || (x.kind == ListKind::FunctionBody && x.ctx.simple_params))
            })
            .collect();
        // Prefer the program (half the time).
        let l = if rng.chance(1, 2) && c.first() == Some(&0) {
            0
        } else {
            c[pick_idx(rng, c.len())?]
        };
        let list = &self.a.lists[l];
        if let Some(&s0) = list.stmts.first() {
            let s = &self.a.stmts[s0];
            let w = self.text(s.t0);
            if self.kind(s.t0) == Some(Kind::Str)
                && (w == "'use strict'" || w == "\"use strict\"")
                && s.t1 - s.t0 <= 2
            {
                return Some(splice(self.src, s.start, s.end, ""));
            }
        }
        Some(splice(self.src, list.open, list.open, "\n'use strict';\n"))
    }

    fn apply(&self, op: Op, rng: &mut Rng) -> Option<String> {
        match op {
            Op::Literal => self.literal(rng),
            Op::Operator => self.operator(rng),
            Op::Ident => self.ident(rng),
            Op::ExprSplice => self.expr_splice(rng),
            Op::StmtSplice => self.stmt_splice(rng),
            Op::StmtRemove => self.stmt_remove(rng),
            Op::StmtDup => self.stmt_dup(rng),
            Op::StmtSwap => self.stmt_swap(rng),
            Op::Wrap => self.wrap(rng),
            Op::Strict => self.strict(rng),
        }
    }
}

/// A mutant and the operators applied to make it, in order.
#[derive(Clone, Debug)]
pub struct Mutant {
    pub text: String,
    pub ops: Vec<Op>,
}

fn pick_op(rng: &mut Rng) -> Op {
    let total: u64 = Op::ALL.iter().map(|o| o.weight()).sum();
    let mut r = rng.below(total);
    for o in Op::ALL {
        if r < o.weight() {
            return o;
        }
        r -= o.weight();
    }
    Op::Literal
}

/// Apply `n` mutations to `src`, splicing from `donors` (or from `src` itself when `None`).
/// Each candidate edit is re-analyzed; one that breaks the token/bracket structure or adds
/// a parser anomaly is discarded and another is drawn (at most `8 * n` draws).
pub fn mutate_with(src: &str, donors: Option<&Donors>, seed: u64, n: u32) -> Mutant {
    let own;
    let donors = match donors {
        Some(d) => d,
        None => {
            own = Donors::from_sources(&[src]);
            &own
        }
    };
    let mut rng = Rng::new(seed);
    let mut cur = src.to_string();
    let mut ops = Vec::new();
    let mut a = analyze(&cur);
    let mut draws = 0;
    while (ops.len() as u32) < n && draws < 8 * n.max(1) {
        draws += 1;
        let op = pick_op(&mut rng);
        let cx = Cx {
            src: &cur,
            a: &a,
            donors,
        };
        let Some(next) = cx.apply(op, &mut rng) else {
            continue;
        };
        if next == cur {
            continue;
        }
        let b = analyze(&next);
        if !b.lexed.ok || !b.balanced || b.anomalies > a.anomalies {
            continue;
        }
        cur = next;
        a = b;
        ops.push(op);
    }
    Mutant { text: cur, ops }
}

/// Apply `n` mutations, using `src` itself as the donor pool. Pure function of its inputs.
pub fn mutate(src: &str, seed: u64, n: u32) -> String {
    mutate_with(src, None, seed, n).text
}

/// Visible single-input D2 inputs (no shims, no extra flags) of at most `max_bytes`, in
/// `cases.jsonl` order. The same filter as the driver's `D2Inputs::load`.
pub fn visible_d2_inputs(root: &Path, max_bytes: u64) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(root.join("corpus/d2/cases.jsonl")) else {
        return vec![];
    };
    let mut files = vec![];
    for line in text.lines() {
        let Ok(c) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ins = c["inputs"].as_array().map(|a| a.len()).unwrap_or(0);
        let shims = c["shims"].as_array().map(|a| a.len()).unwrap_or(0);
        let flags = c["extra_flags"].as_array().map(|a| a.len()).unwrap_or(1);
        if ins != 1 || shims != 0 || flags != 0 {
            continue;
        }
        let Some(p) = c["inputs"][0].as_str() else {
            continue;
        };
        let p = root.join(p);
        if std::fs::metadata(&p).is_ok_and(|m| m.len() <= max_bytes) {
            files.push(p);
        }
    }
    files
}

/// The subset of `files` (from [`visible_d2_inputs`]) whose D2 golden ADVANCED result
/// (`corpus-cache/d2/_golden/<golden tag>/<case>/advanced.json`, D-012; the tag is the
/// [`references::reference`] row's, `ref-4ef5a893` by default) exited 0 with
/// non-trivial output (at least 40 bytes once comments and whitespace are removed).
/// Most single-input D2 files are CJS/UMD npm files whose free names are
/// JSC_UNDEFINED_VARIABLE errors under ADVANCED, so mutants of them never reach the
/// optimisation passes the ADVANCED-family profiles exist for.
pub fn advanced_viable(root: &Path, files: &[PathBuf]) -> Vec<PathBuf> {
    let Some(tag) = references::reference().ok().and_then(|r| r.golden_tag()) else {
        return vec![];
    };
    let Ok(text) = std::fs::read_to_string(root.join("corpus/d2/cases.jsonl")) else {
        return vec![];
    };
    let mut ok = std::collections::BTreeSet::new();
    for line in text.lines() {
        let Ok(c) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let (Some(id), Some(inp)) = (c["id"].as_str(), c["inputs"][0].as_str()) else {
            continue;
        };
        if c["inputs"].as_array().map(|a| a.len()) != Some(1) {
            continue;
        }
        let g = root.join(format!("corpus-cache/d2/_golden/{tag}/{id}/advanced.json"));
        let Ok(gt) = std::fs::read_to_string(&g) else {
            continue;
        };
        let Ok(r) = serde_json::from_str::<Value>(&gt) else {
            continue;
        };
        if r["exit_code"].as_i64() != Some(0) {
            continue;
        }
        let out = r["outputs"]["out.js"].as_str().unwrap_or("");
        if meaningful_len(out) >= 40 {
            ok.insert(root.join(inp));
        }
    }
    files.iter().filter(|f| ok.contains(*f)).cloned().collect()
}

/// Length of `js` without `/* */` and `//` comments and whitespace (rough, for filtering).
fn meaningful_len(js: &str) -> usize {
    let b = js.as_bytes();
    let (mut i, mut n) = (0, 0);
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            i = js[i + 2..]
                .find("*/")
                .map(|k| i + 2 + k + 2)
                .unwrap_or(b.len());
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'/') && (i == 0 || b[i - 1] == b'\n') {
            i = js[i..].find('\n').map(|k| i + k).unwrap_or(b.len());
        } else {
            if !b[i].is_ascii_whitespace() {
                n += 1;
            }
            i += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donor_free_names_and_fit() {
        assert_eq!(
            donor_free_names("trackSelf(target);"),
            vec!["target", "trackSelf"]
        );
        assert!(donor_free_names("var a = 1, b = a + 2; console.log(a, b);").is_empty());
        assert!(donor_free_names("function f(x, y) { return x + y; }").is_empty());
        assert!(donor_free_names("let z = [1, 2].map(v => v + 1);").is_empty());
        assert!(donor_free_names("const { p, q: r } = o; r(p);") == vec!["o"]);
        assert!(donor_free_names("try { g(); } catch (e) { h(e); }") == vec!["g", "h"]);
        assert!(donor_free_names("x = { k: 1 }.k;") == vec!["x"]);
        let host: std::collections::BTreeSet<String> = ["target", "trackSelf"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(donor_fits("trackSelf(target);", &host));
        assert!(!donor_fits("trackOther(target);", &host));
    }

    const SRC: &str = "'use strict';\nvar a = 1 + 2;\nfunction f(x) {\n  if (x > a) { return x * 'q'; }\n  for (var i = 0; i < 3; i++) { if (i) continue; a += i; }\n  return /re/g.test(String(x)) ? -1 : 0;\n}\nconsole.log(f(a), true);\n";

    #[test]
    fn deterministic_and_changes() {
        assert_eq!(mutate(SRC, 5, 3), mutate(SRC, 5, 3));
        let changed = (0..40).filter(|&s| mutate(SRC, s, 2) != SRC).count();
        assert!(changed > 35, "{changed}");
    }

    #[test]
    fn every_operator_fires_and_keeps_structure() {
        let donors =
            Donors::from_sources(&[SRC, "while (y) { y--; }\nlet z = [1, 2].map(v => v + 1);"]);
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..600 {
            let m = mutate_with(SRC, Some(&donors), seed, 1);
            let a = analyze(&m.text);
            assert!(a.clean(), "seed {seed} ops {:?}:\n{}", m.ops, m.text);
            seen.extend(m.ops);
        }
        assert_eq!(seen.len(), Op::ALL.len(), "{seen:?}");
    }

    #[test]
    fn strict_toggle_removes_directive() {
        let mut removed = false;
        for seed in 0..200 {
            let m = mutate_with(SRC, None, seed, 1);
            if m.ops == [Op::Strict] && !m.text.contains("'use strict'") {
                removed = true;
            }
        }
        assert!(removed);
    }

    #[test]
    fn non_portable_statements_are_not_donated() {
        let d = Donors::from_sources(&[
            "function* g() { yield 1; }\nclass A extends B { m() { super.m(); } }\nfor (;;) { break; }\nx: while (1) { break x; }\nlabel: if (q) { break label; }",
        ]);
        assert!(
            d.stmts
                .iter()
                .all(|s| !s.text.contains("yield") && !s.text.contains("super"))
        );
        assert!(d.stmts.iter().any(|s| s.text.starts_with("for")));
        assert!(!d.stmts.iter().any(|s| s.text.contains("break label")));
    }
}
