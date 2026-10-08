//! A JavaScript lexer that finds token boundaries for the mutator.
//!
//! It is not a validating lexer: anything it cannot classify becomes a one-byte `Punct`, and
//! `Lexed::ok` turns false on an unterminated string, comment, template or regexp. Comments
//! are not tokens; they are listed in `Lexed::comments`. A template literal (with all of
//! its `${...}` substitutions) is one opaque `Template` token. Regexp versus division is
//! decided from the previous significant token, as a JS lexer does.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ident,
    Keyword,
    Number,
    BigInt,
    Str,
    Template,
    Regex,
    Private,
    Punct,
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
    /// A line terminator occurs between the previous token and this one.
    pub nl_before: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Lexed {
    pub toks: Vec<Tok>,
    /// Byte ranges of comments (including a leading hashbang line).
    pub comments: Vec<(usize, usize)>,
    pub ok: bool,
}

/// Reserved words (ES2022, strict mode) plus the literals `null`, `true`, `false`.
pub const RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "await",
];

/// Identifiers with a contextual meaning; the mutator never invents or replaces them.
pub const CONTEXTUAL: &[&str] = &[
    "async",
    "get",
    "set",
    "of",
    "from",
    "as",
    "target",
    "meta",
    "accessor",
    "eval",
    "arguments",
    "constructor",
    "prototype",
    "undefined",
];

pub fn is_reserved(s: &str) -> bool {
    RESERVED.contains(&s)
}

const PUNCTS: &[&str] = &[
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "%=", "&=", "|=", "^=", "**",
    "<<", ">>", "/=",
];

/// Does a token of this kind/text end an expression (so a following `/` is division)?
pub fn ends_expr(kind: Kind, text: &str) -> bool {
    match kind {
        Kind::Number
        | Kind::BigInt
        | Kind::Str
        | Kind::Template
        | Kind::Regex
        | Kind::Private
        | Kind::Ident => true,
        Kind::Keyword => matches!(text, "this" | "super" | "null" | "true" | "false"),
        Kind::Punct => matches!(text, ")" | "]" | "}"),
    }
}

struct Lexer<'a> {
    src: &'a str,
    b: &'a [u8],
    i: usize,
    toks: Vec<Tok>,
    comments: Vec<(usize, usize)>,
    ok: bool,
    nl: bool,
}

pub fn lex(src: &str) -> Lexed {
    let mut lx = Lexer {
        src,
        b: src.as_bytes(),
        i: 0,
        toks: Vec::new(),
        comments: Vec::new(),
        ok: true,
        nl: false,
    };
    if src.starts_with("#!") {
        while lx.i < lx.b.len() && lx.b[lx.i] != b'\n' {
            lx.i += 1;
        }
        lx.comments.push((0, lx.i));
    }
    lx.run(false);
    Lexed {
        toks: lx.toks,
        comments: lx.comments,
        ok: lx.ok,
    }
}

fn ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c == b'\\' || c >= 0x80
}

fn ident_part(c: u8) -> bool {
    ident_start(c) || c.is_ascii_digit()
}

impl Lexer<'_> {
    fn ch_at(&self, i: usize) -> Option<char> {
        self.src.get(i..).and_then(|s| s.chars().next())
    }

    fn bump_boundary(&mut self) {
        while self.i < self.b.len() && !self.src.is_char_boundary(self.i) {
            self.i += 1;
        }
    }

    /// Skips whitespace and comments; sets `self.nl` on a line terminator.
    fn skip_trivia(&mut self) {
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'\n' || c == b'\r' {
                self.nl = true;
                self.i += 1;
            } else if c == b' ' || c == b'\t' || c == 0x0b || c == 0x0c {
                self.i += 1;
            } else if c >= 0x80 {
                let ch = self.ch_at(self.i).unwrap_or('\0');
                if ch == '\u{2028}' || ch == '\u{2029}' {
                    self.nl = true;
                } else if !(ch.is_whitespace() || ch == '\u{feff}') {
                    return;
                }
                self.i += ch.len_utf8().max(1);
            } else if c == b'/' && self.b.get(self.i + 1) == Some(&b'/') {
                let s = self.i;
                while self.i < self.b.len() && self.b[self.i] != b'\n' && self.b[self.i] != b'\r' {
                    self.i += 1;
                }
                self.comments.push((s, self.i));
            } else if c == b'/' && self.b.get(self.i + 1) == Some(&b'*') {
                let s = self.i;
                self.i += 2;
                loop {
                    if self.i + 1 >= self.b.len() {
                        self.ok = false;
                        self.i = self.b.len();
                        break;
                    }
                    if self.b[self.i] == b'*' && self.b[self.i + 1] == b'/' {
                        self.i += 2;
                        break;
                    }
                    if self.b[self.i] == b'\n' || self.b[self.i] == b'\r' {
                        self.nl = true;
                    }
                    self.i += 1;
                }
                self.comments.push((s, self.i));
            } else {
                return;
            }
        }
    }

    /// Lexes to EOF, or (in a template substitution) to its closing `}` (not consumed).
    fn run(&mut self, in_subst: bool) {
        let mut depth = 0usize;
        loop {
            self.skip_trivia();
            if self.i >= self.b.len() {
                if in_subst {
                    self.ok = false;
                }
                return;
            }
            if in_subst {
                match self.b[self.i] {
                    b'{' => depth += 1,
                    b'}' if depth == 0 => return,
                    b'}' => depth -= 1,
                    _ => {}
                }
            }
            self.one();
        }
    }

    fn prev_ends_expr(&self) -> bool {
        self.toks
            .last()
            .is_some_and(|t| ends_expr(t.kind, &self.src[t.start..t.end]))
    }

    fn one(&mut self) {
        let b = self.b;
        let start = self.i;
        let c = b[self.i];
        let kind;
        if c == b'\'' || c == b'"' {
            self.i += 1;
            loop {
                if self.i >= b.len() || b[self.i] == b'\n' || b[self.i] == b'\r' {
                    self.ok = false;
                    break;
                }
                if b[self.i] == b'\\' {
                    self.i += 2;
                    // A line continuation `\` CR LF.
                    if self.i < b.len() && b[self.i - 1] == b'\r' && b[self.i] == b'\n' {
                        self.i += 1;
                    }
                    self.bump_boundary();
                    continue;
                }
                if b[self.i] == c {
                    self.i += 1;
                    break;
                }
                self.i += 1;
            }
            self.i = self.i.min(b.len());
            kind = Kind::Str;
        } else if c == b'`' {
            self.template();
            kind = Kind::Template;
        } else if c.is_ascii_digit()
            || (c == b'.' && b.get(self.i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            kind = self.number();
        } else if ident_start(c) {
            self.ident();
            let t = &self.src[start..self.i];
            kind = if is_reserved(t) {
                Kind::Keyword
            } else {
                Kind::Ident
            };
        } else if c == b'#' && b.get(self.i + 1).is_some_and(|&d| ident_start(d)) {
            self.i += 1;
            self.ident();
            kind = Kind::Private;
        } else if c == b'/' && !self.prev_ends_expr() {
            self.regex();
            kind = Kind::Regex;
        } else {
            let rest = &self.src[self.i..];
            let p = PUNCTS.iter().find(|p| rest.starts_with(**p)).filter(|p| {
                // `?.5` is `?` then `.5`.
                **p != "?." || !b.get(self.i + 2).is_some_and(|d| d.is_ascii_digit())
            });
            self.i += p.map(|p| p.len()).unwrap_or(1);
            self.bump_boundary();
            kind = Kind::Punct;
        }
        self.toks.push(Tok {
            start,
            end: self.i,
            kind,
            nl_before: std::mem::take(&mut self.nl),
        });
    }

    fn ident(&mut self) {
        let b = self.b;
        while self.i < b.len() && ident_part(b[self.i]) {
            if b[self.i] == b'\\' {
                self.i += 1;
                if b.get(self.i) == Some(&b'u') {
                    self.i += 1;
                    if b.get(self.i) == Some(&b'{') {
                        while self.i < b.len() && b[self.i] != b'}' {
                            self.i += 1;
                        }
                        self.i += 1;
                    } else {
                        self.i += 4;
                    }
                }
                self.i = self.i.min(b.len());
                continue;
            }
            if b[self.i] >= 0x80 {
                let ch = self.ch_at(self.i).unwrap_or('\0');
                if ch.is_whitespace() || ch == '\u{feff}' {
                    break;
                }
                self.i += ch.len_utf8().max(1);
                continue;
            }
            self.i += 1;
        }
    }

    fn number(&mut self) -> Kind {
        let b = self.b;
        let mut kind = Kind::Number;
        if b[self.i] == b'0' && b.get(self.i + 1).is_some_and(|d| b"xXoObB".contains(d)) {
            self.i += 2;
            while self.i < b.len() && (b[self.i].is_ascii_alphanumeric() || b[self.i] == b'_') {
                self.i += 1;
            }
            if b[self.i - 1] == b'n' {
                kind = Kind::BigInt;
            }
            return kind;
        }
        let digits = |l: &mut Self| {
            while l.i < b.len() && (b[l.i].is_ascii_digit() || b[l.i] == b'_') {
                l.i += 1;
            }
        };
        digits(self);
        if self.i < b.len() && b[self.i] == b'.' {
            self.i += 1;
            digits(self);
        }
        if self.i < b.len() && (b[self.i] == b'e' || b[self.i] == b'E') {
            self.i += 1;
            if self.i < b.len() && (b[self.i] == b'+' || b[self.i] == b'-') {
                self.i += 1;
            }
            digits(self);
        }
        if self.i < b.len() && b[self.i] == b'n' {
            self.i += 1;
            kind = Kind::BigInt;
        }
        kind
    }

    fn regex(&mut self) {
        let b = self.b;
        self.i += 1;
        let mut class = false;
        loop {
            if self.i >= b.len() || b[self.i] == b'\n' || b[self.i] == b'\r' {
                self.ok = false;
                return;
            }
            match b[self.i] {
                b'\\' => self.i += 1,
                b'[' => class = true,
                b']' => class = false,
                b'/' if !class => {
                    self.i += 1;
                    break;
                }
                _ => {}
            }
            self.i += 1;
        }
        while self.i < b.len() && ident_part(b[self.i]) {
            self.i += 1;
        }
        self.bump_boundary();
    }

    fn template(&mut self) {
        let b = self.b;
        self.i += 1;
        loop {
            if self.i >= b.len() {
                self.ok = false;
                return;
            }
            match b[self.i] {
                b'\\' => self.i += 2,
                b'`' => {
                    self.i += 1;
                    return;
                }
                b'$' if b.get(self.i + 1) == Some(&b'{') => {
                    self.i += 2;
                    let saved = self.toks.len();
                    let nl = self.nl;
                    self.run(true);
                    self.toks.truncate(saved);
                    self.nl = nl;
                    if self.i < b.len() {
                        self.i += 1;
                    }
                }
                _ => self.i += 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        lex(src).toks.iter().map(|t| &src[t.start..t.end]).collect()
    }

    #[test]
    fn regex_division_comments() {
        let src = "x = a / b; y = /ab+c/g.test(s); // c\n/* d */ z = '\\'';";
        let l = lex(src);
        assert!(l.ok);
        let t = texts(src);
        assert!(t.contains(&"/ab+c/g"));
        assert!(t.contains(&"'\\''"));
        assert_eq!(l.comments.len(), 2);
    }

    #[test]
    fn templates_are_opaque() {
        let src = "f(`a${ {b: `c${d}`}.b }e`, 1)";
        let l = lex(src);
        assert!(l.ok);
        assert_eq!(
            texts(src),
            vec!["f", "(", "`a${ {b: `c${d}`}.b }e`", ",", "1", ")"]
        );
    }

    #[test]
    fn numbers_puncts_private() {
        let src = "a >>>= 0x1Fn + 1_000.5e-3 ?.5 : b?.c; #p in o;";
        assert_eq!(
            texts(src),
            vec![
                "a",
                ">>>=",
                "0x1Fn",
                "+",
                "1_000.5e-3",
                "?",
                ".5",
                ":",
                "b",
                "?.",
                "c",
                ";",
                "#p",
                "in",
                "o",
                ";"
            ]
        );
        assert_eq!(lex(src).toks[2].kind, Kind::BigInt);
    }

    #[test]
    fn unterminated_is_not_ok() {
        assert!(!lex("'abc").ok);
        assert!(!lex("/* x").ok);
        assert!(!lex("`a${b").ok);
    }

    #[test]
    fn newline_flags() {
        let l = lex("a\n/*\n*/b c");
        assert!(l.toks[1].nl_before);
        assert!(!l.toks[2].nl_before);
    }
}
