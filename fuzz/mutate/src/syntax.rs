//! Statement-level structure of a JavaScript program, recovered from tokens.
//!
//! This is a tolerant recursive-descent parser for statements. Expressions are skipped as
//! bracket-balanced token runs, with the ASI rules for line terminators, but the parser
//! descends into every function body it meets inside them (`) {` and `=> {`), so the
//! statements of IIFEs, callbacks and methods are found too. Every statement list carries
//! the context the mutator needs (inside a function? a loop? simple parameters?). Anything
//! surprising counts as an anomaly and marks the list unreliable; the mutator only edits
//! reliable lists, and rejects any edit that adds anomalies.

use crate::lex::{Kind, Lexed, Tok, ends_expr, lex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Program,
    FunctionBody,
    Block,
    Case,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    pub in_function: bool,
    pub in_loop: bool,
    pub in_switch: bool,
    /// Function body whose parameters are plain identifiers (a `'use strict'` directive is
    /// then allowed).
    pub simple_params: bool,
    pub fn_depth: u32,
}

#[derive(Clone, Debug)]
pub struct List {
    pub kind: ListKind,
    /// Byte offset where a statement can be inserted before every other statement.
    pub open: usize,
    pub stmts: Vec<usize>,
    pub ctx: Ctx,
    pub reliable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SKind {
    Expr,
    Var,
    Lexical,
    Function,
    Class,
    If,
    Loop,
    Switch,
    Try,
    Block,
    Return,
    Throw,
    Jump,
    Import,
    Export,
    Labelled,
    Empty,
    Other,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    /// Token range `[t0, t1)`.
    pub t0: usize,
    pub t1: usize,
    pub start: usize,
    pub end: usize,
    pub kind: SKind,
    pub fn_depth: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub lexed: Lexed,
    /// For each bracket token, the index of its partner; `usize::MAX` otherwise.
    pub partner: Vec<usize>,
    pub balanced: bool,
    pub lists: Vec<List>,
    pub stmts: Vec<Stmt>,
    /// Token index pairs `(open, close)` of if/while/switch/do-while conditions.
    pub conds: Vec<(usize, usize)>,
    /// Token ranges `[a, b)` of expressions: return/throw arguments and expression statements.
    pub exprs: Vec<(usize, usize)>,
    /// Token index pairs `(open, close)` of function bodies.
    pub fn_bodies: Vec<(usize, usize)>,
    pub anomalies: usize,
}

impl Analysis {
    pub fn toks(&self) -> &[Tok] {
        &self.lexed.toks
    }

    /// The program is lexically sane and the parser saw nothing it did not understand.
    pub fn clean(&self) -> bool {
        self.lexed.ok && self.balanced && self.anomalies == 0
    }

    /// Number of function bodies enclosing token `i`.
    pub fn fn_depth_at(&self, i: usize) -> u32 {
        self.fn_bodies
            .iter()
            .filter(|(a, b)| *a < i && i < *b)
            .count() as u32
    }
}

pub fn analyze(src: &str) -> Analysis {
    let lexed = lex(src);
    let n = lexed.toks.len();
    let mut partner = vec![usize::MAX; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut balanced = true;
    for (i, t) in lexed.toks.iter().enumerate() {
        if t.kind != Kind::Punct {
            continue;
        }
        match &src[t.start..t.end] {
            "(" | "[" | "{" => stack.push(i),
            c @ (")" | "]" | "}") => {
                let want = match c {
                    ")" => "(",
                    "]" => "[",
                    _ => "{",
                };
                match stack.pop() {
                    Some(o) if &src[lexed.toks[o].start..lexed.toks[o].end] == want => {
                        partner[o] = i;
                        partner[i] = o;
                    }
                    _ => {
                        balanced = false;
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        balanced = false;
    }
    let mut a = Analysis {
        lexed,
        partner,
        balanced,
        ..Analysis::default()
    };
    if !a.balanced {
        return a;
    }
    let mut p = Parser {
        src,
        a: &mut a,
        i: 0,
    };
    p.list(Ctx::default(), n, ListKind::Program, 0);
    a
}

struct Parser<'s, 'a> {
    src: &'s str,
    a: &'a mut Analysis,
    i: usize,
}

const CONTINUES: &[&str] = &[
    "(", "[", ".", "?.", ",", "?", ":", "=", "=>", "+", "-", "*", "/", "%", "**", "<", ">", "<=",
    ">=", "==", "!=", "===", "!==", "&&", "||", "??", "&", "|", "^", "<<", ">>", ">>>", "+=", "-=",
    "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=", "??=",
];

impl Parser<'_, '_> {
    fn n(&self) -> usize {
        self.a.lexed.toks.len()
    }

    fn t(&self, i: usize) -> &str {
        match self.a.lexed.toks.get(i) {
            Some(t) => &self.src[t.start..t.end],
            None => "",
        }
    }

    fn kind(&self, i: usize) -> Option<Kind> {
        self.a.lexed.toks.get(i).map(|t| t.kind)
    }

    fn nl(&self, i: usize) -> bool {
        self.a.lexed.toks.get(i).is_some_and(|t| t.nl_before)
    }

    fn is_punct(&self, i: usize, s: &str) -> bool {
        self.kind(i) == Some(Kind::Punct) && self.t(i) == s
    }

    fn is_kw(&self, i: usize, s: &str) -> bool {
        matches!(self.kind(i), Some(Kind::Keyword | Kind::Ident)) && self.t(i) == s
    }

    fn anomaly(&mut self) {
        self.a.anomalies += 1;
    }

    /// Statement list up to token `end` (exclusive).
    fn list(&mut self, ctx: Ctx, end: usize, kind: ListKind, open: usize) -> usize {
        let li = self.a.lists.len();
        self.a.lists.push(List {
            kind,
            open,
            stmts: vec![],
            ctx,
            reliable: true,
        });
        let before = self.a.anomalies;
        while self.i < end {
            if kind == ListKind::Case
                && (self.is_kw(self.i, "case")
                    || (self.is_kw(self.i, "default") && self.is_punct(self.i + 1, ":")))
            {
                break;
            }
            let at = self.i;
            let s = self.stmt(ctx, end);
            if self.i == at {
                self.anomaly();
                self.i += 1;
            } else if let Some(s) = s {
                self.a.lists[li].stmts.push(s);
            }
        }
        if self.i > end {
            self.anomaly();
            self.i = end;
        }
        if self.a.anomalies != before {
            self.a.lists[li].reliable = false;
        }
        li
    }

    fn push_stmt(&mut self, t0: usize, kind: SKind, ctx: Ctx) -> Option<usize> {
        let t1 = self.i.min(self.n());
        if t1 <= t0 {
            return None;
        }
        let toks = &self.a.lexed.toks;
        let s = Stmt {
            t0,
            t1,
            start: toks[t0].start,
            end: toks[t1 - 1].end,
            kind,
            fn_depth: ctx.fn_depth,
        };
        self.a.stmts.push(s);
        Some(self.a.stmts.len() - 1)
    }

    fn group(&mut self, ctx: Ctx) -> Option<usize> {
        let p = self.a.partner.get(self.i).copied().unwrap_or(usize::MAX);
        if p == usize::MAX || !matches!(self.t(self.i), "(" | "[" | "{") {
            self.anomaly();
            return None;
        }
        let open = self.i;
        self.region(open + 1, p, ctx);
        self.i = p + 1;
        Some(open)
    }

    fn opt_semi(&mut self) {
        if self.is_punct(self.i, ";") {
            self.i += 1;
        }
    }

    /// A `{ ... }` block or body at `self.i`, parsed as a statement list.
    fn body(&mut self, ctx: Ctx, kind: ListKind) {
        if !self.is_punct(self.i, "{") {
            self.anomaly();
            return;
        }
        let open = self.i;
        let close = self.a.partner[open];
        if kind == ListKind::FunctionBody {
            self.a.fn_bodies.push((open, close));
        }
        let at = self.a.lexed.toks[open].end;
        self.i = open + 1;
        self.list(ctx, close, kind, at);
        self.i = close + 1;
    }

    fn fn_ctx(&self, ctx: Ctx, simple: bool) -> Ctx {
        Ctx {
            in_function: true,
            in_loop: false,
            in_switch: false,
            simple_params: simple,
            fn_depth: ctx.fn_depth + 1,
        }
    }

    fn params_simple(&self, open: usize) -> bool {
        let close = self.a.partner[open];
        (open + 1..close)
            .all(|k| self.kind(k) == Some(Kind::Ident) || (self.is_punct(k, ",") && k + 1 < close))
    }

    fn stmt(&mut self, ctx: Ctx, end: usize) -> Option<usize> {
        let t0 = self.i;
        let w = self.t(t0).to_string();
        let k = self.kind(t0);
        let kind = match (k, w.as_str()) {
            (Some(Kind::Punct), "{") => {
                self.body(ctx, ListKind::Block);
                SKind::Block
            }
            (Some(Kind::Punct), ";") => {
                self.i += 1;
                SKind::Empty
            }
            (Some(Kind::Keyword), "var") => {
                self.expr(ctx, end, false);
                SKind::Var
            }
            (Some(Kind::Keyword), "const") => {
                self.expr(ctx, end, false);
                SKind::Lexical
            }
            (Some(Kind::Keyword), "let")
                if matches!(self.kind(t0 + 1), Some(Kind::Ident | Kind::Keyword))
                    || matches!(self.t(t0 + 1), "[" | "{") =>
            {
                self.expr(ctx, end, false);
                SKind::Lexical
            }
            (Some(Kind::Keyword), "if") => {
                self.i += 1;
                self.cond(ctx);
                self.sub_stmt(ctx, end);
                if self.is_kw(self.i, "else") {
                    self.i += 1;
                    self.sub_stmt(ctx, end);
                }
                SKind::If
            }
            (Some(Kind::Keyword), "for") => {
                self.i += 1;
                if self.is_kw(self.i, "await") {
                    self.i += 1;
                }
                self.group(ctx);
                let lc = Ctx {
                    in_loop: true,
                    ..ctx
                };
                self.sub_stmt(lc, end);
                SKind::Loop
            }
            (Some(Kind::Keyword), "while") => {
                self.i += 1;
                self.cond(ctx);
                let lc = Ctx {
                    in_loop: true,
                    ..ctx
                };
                self.sub_stmt(lc, end);
                SKind::Loop
            }
            (Some(Kind::Keyword), "do") => {
                self.i += 1;
                let lc = Ctx {
                    in_loop: true,
                    ..ctx
                };
                self.sub_stmt(lc, end);
                if !self.is_kw(self.i, "while") {
                    self.anomaly();
                } else {
                    self.i += 1;
                    self.cond(ctx);
                    self.opt_semi();
                }
                SKind::Loop
            }
            (Some(Kind::Keyword), "with") => {
                self.i += 1;
                self.group(ctx);
                self.sub_stmt(ctx, end);
                SKind::Other
            }
            (Some(Kind::Keyword), "function") => {
                self.function(ctx);
                SKind::Function
            }
            (Some(Kind::Ident), "async") if self.is_kw(t0 + 1, "function") && !self.nl(t0 + 1) => {
                self.i += 1;
                self.function(ctx);
                SKind::Function
            }
            (Some(Kind::Keyword), "class") => {
                self.class(ctx);
                SKind::Class
            }
            (Some(Kind::Keyword), "try") => {
                self.i += 1;
                self.body(ctx, ListKind::Block);
                if self.is_kw(self.i, "catch") {
                    self.i += 1;
                    if self.is_punct(self.i, "(") {
                        self.group(ctx);
                    }
                    self.body(ctx, ListKind::Block);
                }
                if self.is_kw(self.i, "finally") {
                    self.i += 1;
                    self.body(ctx, ListKind::Block);
                }
                SKind::Try
            }
            (Some(Kind::Keyword), "switch") => {
                self.i += 1;
                self.cond(ctx);
                self.switch_body(ctx);
                SKind::Switch
            }
            (Some(Kind::Keyword), "return" | "throw") => {
                self.i += 1;
                if !self.is_punct(self.i, ";")
                    && !self.is_punct(self.i, "}")
                    && self.i < end
                    && !self.nl(self.i)
                {
                    let a = self.i;
                    self.expr(ctx, end, false);
                    let b = if self.is_punct(self.i - 1, ";") {
                        self.i - 1
                    } else {
                        self.i
                    };
                    if b > a {
                        self.a.exprs.push((a, b));
                    }
                } else {
                    self.opt_semi();
                }
                if w == "return" {
                    SKind::Return
                } else {
                    SKind::Throw
                }
            }
            (Some(Kind::Keyword), "break" | "continue") => {
                self.i += 1;
                if self.kind(self.i) == Some(Kind::Ident) && !self.nl(self.i) {
                    self.i += 1;
                }
                self.opt_semi();
                SKind::Jump
            }
            (Some(Kind::Keyword), "debugger") => {
                self.i += 1;
                self.opt_semi();
                SKind::Other
            }
            (Some(Kind::Keyword), "import")
                if !self.is_punct(t0 + 1, "(") && !self.is_punct(t0 + 1, ".") =>
            {
                self.expr(ctx, end, false);
                SKind::Import
            }
            (Some(Kind::Keyword), "export") => {
                self.i += 1;
                if self.is_kw(self.i, "default") {
                    self.i += 1;
                }
                match self.t(self.i) {
                    "function" => self.function(ctx),
                    "async" if self.is_kw(self.i + 1, "function") => {
                        self.i += 1;
                        self.function(ctx)
                    }
                    "class" => self.class(ctx),
                    _ => self.expr(ctx, end, false),
                }
                SKind::Export
            }
            (Some(Kind::Ident | Kind::Keyword), _)
                if self.is_punct(t0 + 1, ":") && !matches!(w.as_str(), "case" | "default") =>
            {
                self.i += 2;
                self.sub_stmt(ctx, end);
                SKind::Labelled
            }
            (Some(Kind::Keyword), "case" | "default" | "else" | "catch" | "finally") => {
                self.anomaly();
                self.i += 1;
                SKind::Other
            }
            _ => {
                let a = self.i;
                self.expr(ctx, end, false);
                let b = if self.is_punct(self.i - 1, ";") {
                    self.i - 1
                } else {
                    self.i
                };
                if b > a && !matches!(w.as_str(), "function" | "class" | "{" | "let") {
                    self.a.exprs.push((a, b));
                }
                SKind::Expr
            }
        };
        self.push_stmt(t0, kind, ctx)
    }

    /// A statement nested in a compound statement (not itself in a list).
    fn sub_stmt(&mut self, ctx: Ctx, end: usize) {
        let at = self.i;
        if self.i >= end {
            self.anomaly();
            return;
        }
        self.stmt(ctx, end);
        if self.i == at {
            self.anomaly();
            self.i += 1;
        }
    }

    fn cond(&mut self, ctx: Ctx) {
        if !self.is_punct(self.i, "(") {
            self.anomaly();
            return;
        }
        let open = self.i;
        self.group(ctx);
        let close = self.a.partner[open];
        if close > open + 1 {
            self.a.conds.push((open, close));
        }
    }

    fn function(&mut self, ctx: Ctx) {
        // at `function`
        self.i += 1;
        if self.is_punct(self.i, "*") {
            self.i += 1;
        }
        if matches!(self.kind(self.i), Some(Kind::Ident | Kind::Keyword)) {
            self.i += 1;
        }
        if !self.is_punct(self.i, "(") {
            self.anomaly();
            return;
        }
        let open = self.i;
        let simple = self.params_simple(open);
        self.group(ctx);
        self.body(self.fn_ctx(ctx, simple), ListKind::FunctionBody);
    }

    fn class(&mut self, ctx: Ctx) {
        // at `class`; heritage expression then the body (scanned as a region).
        self.i += 1;
        while self.i < self.n() && !self.is_punct(self.i, "{") {
            if matches!(self.t(self.i), "(" | "[") {
                self.group(ctx);
            } else {
                self.i += 1;
            }
        }
        if self.is_punct(self.i, "{") {
            self.group(ctx);
        } else {
            self.anomaly();
        }
    }

    fn switch_body(&mut self, ctx: Ctx) {
        if !self.is_punct(self.i, "{") {
            self.anomaly();
            return;
        }
        let close = self.a.partner[self.i];
        self.i += 1;
        let sc = Ctx {
            in_switch: true,
            ..ctx
        };
        while self.i < close {
            if self.is_kw(self.i, "case") {
                self.i += 1;
                self.expr(ctx, close, true);
            } else if self.is_kw(self.i, "default") && self.is_punct(self.i + 1, ":") {
                self.i += 2;
            } else {
                self.anomaly();
                self.i += 1;
                continue;
            }
            let open = self.a.lexed.toks[self.i - 1].end;
            self.list(sc, close, ListKind::Case, open);
        }
        self.i = close + 1;
    }

    /// Scans an expression (or declaration list) at statement level up to its end: `;`
    /// (consumed), a closer of an enclosing construct, `end`, or an ASI line break. With
    /// `case_colon`, also stops after a `:` that is not part of a `?:`.
    fn expr(&mut self, ctx: Ctx, end: usize, case_colon: bool) {
        let mut ternary = 0usize;
        let mut first = true;
        while self.i < end {
            let i = self.i;
            let w = self.t(i);
            let k = self.kind(i);
            if k == Some(Kind::Punct) {
                if w == ";" {
                    self.i += 1;
                    return;
                }
                if matches!(w, ")" | "]" | "}") {
                    return;
                }
                if case_colon && w == ":" {
                    if ternary == 0 {
                        self.i += 1;
                        return;
                    }
                    ternary -= 1;
                }
                if w == "?" {
                    ternary += 1;
                }
            }
            if !first && self.nl(i) {
                let pt = &self.a.lexed.toks[i - 1];
                let prev_end = ends_expr(pt.kind, &self.src[pt.start..pt.end])
                    || matches!(&self.src[pt.start..pt.end], "++" | "--");
                let cont =
                    (k == Some(Kind::Punct) && CONTINUES.contains(&w) && w != "++" && w != "--")
                        || k == Some(Kind::Template)
                        || matches!(w, "in" | "instanceof" | "of");
                if prev_end && !cont {
                    return;
                }
            }
            first = false;
            if k == Some(Kind::Punct) && matches!(w, "(" | "[") {
                self.group(ctx);
            } else if k == Some(Kind::Punct) && w == "{" {
                self.brace(ctx);
            } else {
                self.i += 1;
            }
        }
    }

    /// Scans tokens `[from, to)` inside brackets, descending into function bodies.
    fn region(&mut self, from: usize, to: usize, ctx: Ctx) {
        let save = self.i;
        self.i = from;
        while self.i < to {
            let w = self.t(self.i);
            if self.kind(self.i) == Some(Kind::Punct) && matches!(w, "(" | "[") {
                self.group(ctx);
            } else if self.kind(self.i) == Some(Kind::Punct) && w == "{" {
                self.brace(ctx);
            } else {
                self.i += 1;
            }
        }
        self.i = save;
    }

    /// `{` inside an expression: a function body after `)` or `=>`, else an object literal
    /// or class body.
    fn brace(&mut self, ctx: Ctx) {
        let i = self.i;
        let prev = if i > 0 { self.t(i - 1) } else { "" };
        if i > 0 && (prev == ")" || prev == "=>") && self.kind(i - 1) == Some(Kind::Punct) {
            let simple = if prev == ")" {
                self.params_simple(self.a.partner[i - 1])
            } else if i >= 2 && self.is_punct(i - 2, ")") {
                self.params_simple(self.a.partner[i - 2])
            } else {
                true
            };
            self.body(self.fn_ctx(ctx, simple), ListKind::FunctionBody);
        } else {
            self.group(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stmt_texts(src: &str, li: usize) -> Vec<String> {
        let a = analyze(src);
        a.lists[li]
            .stmts
            .iter()
            .map(|&s| src[a.stmts[s].start..a.stmts[s].end].to_string())
            .collect()
    }

    #[test]
    fn top_level_with_asi() {
        let src = "var a = 1\nlet b = a\n(c)\nfoo()\nx++\ny\nif (a) { b } else c;\nreturn_ = 2";
        let a = analyze(src);
        assert!(a.clean(), "{a:?}");
        assert_eq!(
            stmt_texts(src, 0),
            vec![
                "var a = 1",
                "let b = a\n(c)",
                "foo()",
                "x++",
                "y",
                "if (a) { b } else c;",
                "return_ = 2"
            ]
        );
    }

    #[test]
    fn nested_function_bodies() {
        let src = "(function () { 'use strict'; var x = 1; return x; })();\nfoo(a => { a(); b(); }, class { m(p) { q(); } });";
        let a = analyze(src);
        assert!(a.clean());
        let fb: Vec<usize> = (0..a.lists.len())
            .filter(|&l| a.lists[l].kind == ListKind::FunctionBody)
            .collect();
        assert_eq!(fb.len(), 3);
        assert_eq!(a.lists[fb[0]].stmts.len(), 3);
        assert!(a.lists[fb[0]].ctx.in_function && a.lists[fb[0]].ctx.simple_params);
        assert_eq!(a.lists[fb[1]].stmts.len(), 2);
    }

    #[test]
    fn switch_cases_and_loops() {
        let src = "switch (x) { case a ? 1 : 2: f(); break; default: g(); }\nfor (;;) { if (y) continue; }";
        let a = analyze(src);
        assert!(a.clean(), "{a:?}");
        let cases: Vec<&List> = a
            .lists
            .iter()
            .filter(|l| l.kind == ListKind::Case)
            .collect();
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].stmts.len(), 2);
        assert!(cases[0].ctx.in_switch);
        assert!(a.lists.iter().any(|l| l.ctx.in_loop));
        assert_eq!(a.conds.len(), 2);
    }

    #[test]
    fn unbalanced_is_not_clean() {
        assert!(!analyze("if (a { }").clean());
        assert!(!analyze("else x;").clean());
    }
}
