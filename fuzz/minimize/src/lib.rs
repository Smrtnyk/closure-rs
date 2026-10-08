//! AST-level delta debugging (docs/PORTING.md §4.6), with comment and line-level fallbacks.
//!
//! The tree is not parsed here: a [`TreeSource`] supplies it (the driver uses the Java
//! oracle's `parse_dump`, whose nodes carry `source_offset`/`length` in UTF-16 units; see
//! [`Tree::from_parse_dump`]). `TreeSource::tree` returning `None` means "does not parse",
//! so the tree source is also the **parse-validity check**: every candidate must parse
//! before the (expensive) [`Predicate`] is asked. Rounds repeat until a fixpoint:
//!
//!   1. **Hierarchical ddmin** (HDD): for each tree level, top-down, ddmin over the nodes of
//!      that level: chunks of nodes are deleted together, smaller chunks when that fails.
//!   2. **Replacement**: each remaining node, largest first, is replaced by one of its
//!      children (hoisting) or by `0`.
//!   3. **Comments** (not AST nodes): ddmin over comment ranges.
//!   4. **Lines**: ddmin over lines (Zeller & Hildebrandt), then blank-line cleanup.
//!
//! Limits: a predicate-call budget (deterministic) and an optional wall-clock deadline
//! (a safety net; the result is deterministic whenever the deadline is not reached, which
//! [`Stats::deadline_hit`] reports). Iteration orders never depend on hashing. Every
//! candidate is tested at most once; results are cached.

#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub trait Predicate {
    fn holds(&mut self, src: &str) -> bool;
}

impl<F: FnMut(&str) -> bool> Predicate for F {
    fn holds(&mut self, src: &str) -> bool {
        self(src)
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    /// Byte range in the source.
    pub start: usize,
    pub end: usize,
    pub token: String,
    pub children: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

/// Supplies a tree for a source text (`None` = does not parse).
pub trait TreeSource {
    fn tree(&mut self, src: &str) -> Option<Tree>;
}

impl Tree {
    /// Build from an oracle `parse_dump` response (`ast` with `source_offset`/`length` in
    /// UTF-16 code units). Nodes without a usable range (offset `$threw`, synthetic nodes
    /// with length 0) keep their children but are not themselves reduction candidates.
    pub fn from_parse_dump(dump: &Value, src: &str) -> Option<Tree> {
        let ast = dump.get("ast")?;
        let mut u16_to_byte = Vec::with_capacity(src.len() + 1);
        for (b, ch) in src.char_indices() {
            for _ in 0..ch.len_utf16() {
                u16_to_byte.push(b);
            }
        }
        u16_to_byte.push(src.len());
        let mut t = Tree::default();
        t.root = add(&mut t, ast, &u16_to_byte)?;
        Some(t)
    }

    /// Nodes (except the root) ordered by decreasing size, then source order.
    pub fn candidates(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| i != self.root && self.nodes[i].end > self.nodes[i].start)
            .collect();
        v.sort_by_key(|&i| {
            (
                std::cmp::Reverse(self.nodes[i].end - self.nodes[i].start),
                self.nodes[i].start,
            )
        });
        v
    }

    /// Nodes by depth (root = level 0), each level in source order.
    pub fn levels(&self) -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = vec![];
        let mut cur = vec![self.root];
        while !cur.is_empty() {
            let mut next = vec![];
            for &n in &cur {
                next.extend(self.nodes[n].children.iter().copied());
            }
            out.push(cur);
            cur = next;
        }
        out
    }
}

fn add(t: &mut Tree, n: &Value, map: &[usize]) -> Option<usize> {
    let off = n.get("source_offset").and_then(|v| v.as_u64());
    let len = n.get("length").and_then(|v| v.as_u64()).unwrap_or(0);
    let (start, end) = match off {
        Some(o) if (o + len) < map.len() as u64 => (map[o as usize], map[(o + len) as usize]),
        _ => (0, 0),
    };
    let idx = t.nodes.len();
    t.nodes.push(Node {
        start,
        end,
        token: n
            .get("token")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        children: Vec::new(),
    });
    if let Some(cs) = n.get("children").and_then(|c| c.as_array()) {
        for c in cs {
            if let Some(ci) = add(t, c, map) {
                t.nodes[idx].children.push(ci);
            }
        }
    }
    Some(idx)
}

/// Reduction limits.
#[derive(Clone, Debug)]
pub struct Config {
    /// Maximum predicate calls (deterministic budget).
    pub max_tests: usize,
    /// Maximum parse-validity checks (`TreeSource::tree` calls).
    pub max_parses: usize,
    /// Wall-clock cap; reaching it stops the reduction (then the result may depend on speed).
    pub deadline: Option<Duration>,
    /// Run the comment and line passes.
    pub text_passes: bool,
}

impl Config {
    pub fn calls(max_tests: usize) -> Config {
        Config {
            max_tests,
            max_parses: max_tests.saturating_mul(20).max(1000),
            deadline: None,
            text_passes: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// Predicate calls.
    pub tests: usize,
    /// Parse-validity checks.
    pub parses: usize,
    /// Candidates rejected because they did not parse.
    pub invalid: usize,
    pub ast_steps: usize,
    pub replace_steps: usize,
    pub comment_steps: usize,
    pub line_steps: usize,
    pub rounds: usize,
    pub budget_exhausted: bool,
    pub deadline_hit: bool,
}

struct Tester<'p> {
    pred: &'p mut dyn Predicate,
    trees: Option<&'p mut dyn TreeSource>,
    results: HashMap<String, bool>,
    valid: HashMap<String, bool>,
    cfg: Config,
    t0: Instant,
    stats: Stats,
}

impl Tester<'_> {
    fn out_of_budget(&mut self) -> bool {
        if self.stats.tests >= self.cfg.max_tests || self.stats.parses >= self.cfg.max_parses {
            self.stats.budget_exhausted = true;
            return true;
        }
        if self.cfg.deadline.is_some_and(|d| self.t0.elapsed() >= d) {
            self.stats.deadline_hit = true;
            return true;
        }
        false
    }

    fn parses(&mut self, cand: &str) -> bool {
        let Some(trees) = self.trees.as_mut() else {
            return true;
        };
        if let Some(&v) = self.valid.get(cand) {
            return v;
        }
        self.stats.parses += 1;
        let v = trees.tree(cand).is_some();
        self.valid.insert(cand.to_string(), v);
        v
    }

    fn test(&mut self, cand: &str) -> bool {
        if let Some(&r) = self.results.get(cand) {
            return r;
        }
        if self.out_of_budget() {
            return false;
        }
        if !self.parses(cand) {
            self.stats.invalid += 1;
            return false;
        }
        self.stats.tests += 1;
        let r = self.pred.holds(cand);
        self.results.insert(cand.to_string(), r);
        r
    }

    fn tree(&mut self, src: &str) -> Option<Tree> {
        let trees = self.trees.as_mut()?;
        self.stats.parses += 1;
        trees.tree(src)
    }
}

fn splice(src: &str, start: usize, end: usize, with: &str) -> String {
    let mut s = String::with_capacity(src.len());
    s.push_str(&src[..start]);
    s.push_str(with);
    s.push_str(&src[end..]);
    s
}

/// `base` with every range of `drop` (sorted, disjoint) replaced by its filler.
fn remove_ranges(base: &str, drop: &[(usize, usize, &str)]) -> String {
    let mut s = String::with_capacity(base.len());
    let mut at = 0;
    for &(a, b, fill) in drop {
        s.push_str(&base[at..a]);
        s.push_str(fill);
        at = b;
    }
    s.push_str(&base[at..]);
    s
}

/// ddmin over removable items of `base` (disjoint sorted byte ranges with fillers). Returns
/// the indices of the items removed in the end.
fn ddmin_ranges(base: &str, items: &[(usize, usize, &str)], t: &mut Tester) -> Vec<usize> {
    let mut kept: Vec<usize> = (0..items.len()).collect();
    let mut removed: Vec<usize> = vec![];
    let build = |removed: &[usize]| {
        let mut r: Vec<(usize, usize, &str)> = removed.iter().map(|&i| items[i]).collect();
        r.sort_by_key(|x| x.0);
        remove_ranges(base, &r)
    };
    let mut n = 2usize.min(kept.len().max(1));
    while !kept.is_empty() {
        let chunk = kept.len().div_ceil(n);
        let mut progressed = false;
        let mut start = 0;
        while start < kept.len() {
            let end = (start + chunk).min(kept.len());
            let mut cand_removed = removed.clone();
            cand_removed.extend_from_slice(&kept[start..end]);
            if t.test(&build(&cand_removed)) {
                removed = cand_removed;
                kept.drain(start..end);
                n = (n - 1).max(2).min(kept.len().max(1));
                progressed = true;
                break;
            }
            start = end;
        }
        if t.stats.budget_exhausted || t.stats.deadline_hit {
            break;
        }
        if !progressed {
            if chunk <= 1 {
                break;
            }
            n = (n * 2).min(kept.len());
        }
    }
    removed
}

/// AST-level reduction plus fallbacks; `budget` caps predicate calls.
pub fn reduce(
    src: &str,
    trees: &mut dyn TreeSource,
    pred: &mut dyn Predicate,
    budget: usize,
) -> (String, Stats) {
    reduce_with(src, trees, pred, &Config::calls(budget))
}

/// AST-level reduction plus fallbacks under `cfg`.
pub fn reduce_with(
    src: &str,
    trees: &mut dyn TreeSource,
    pred: &mut dyn Predicate,
    cfg: &Config,
) -> (String, Stats) {
    let original_parses = trees.tree(src).is_some();
    let mut t = Tester {
        pred,
        trees: if original_parses { Some(trees) } else { None },
        results: HashMap::new(),
        valid: HashMap::new(),
        cfg: cfg.clone(),
        t0: Instant::now(),
        stats: Stats::default(),
    };
    t.stats.parses = 1;
    t.results.insert(src.to_string(), true);
    let mut cur = src.to_string();
    loop {
        t.stats.rounds += 1;
        let before = cur.clone();
        if original_parses {
            cur = hdd(&cur, &mut t);
            cur = replace_pass(&cur, &mut t);
        }
        if cfg.text_passes {
            cur = comment_pass(&cur, &mut t);
            let b = t.stats.tests;
            cur = ddmin_lines_with(&cur, &mut t);
            t.stats.line_steps += t.stats.tests - b;
            cur = cleanup(&cur, &mut t);
        }
        if cur == before || t.stats.budget_exhausted || t.stats.deadline_hit {
            break;
        }
    }
    (cur, t.stats)
}

/// Hierarchical delta debugging over the tree levels.
fn hdd(src: &str, t: &mut Tester) -> String {
    let mut cur = src.to_string();
    let mut level = 1;
    loop {
        if t.out_of_budget() {
            break;
        }
        let Some(tree) = t.tree(&cur) else { break };
        let levels = tree.levels();
        if level >= levels.len() {
            break;
        }
        // Disjoint, non-empty ranges of this level, in source order.
        let mut nodes: Vec<(usize, usize)> = levels[level]
            .iter()
            .map(|&i| (tree.nodes[i].start, tree.nodes[i].end))
            .filter(|&(a, b)| {
                b > a && b <= cur.len() && cur.is_char_boundary(a) && cur.is_char_boundary(b)
            })
            .collect();
        nodes.sort();
        nodes.dedup();
        let mut items: Vec<(usize, usize, &str)> = vec![];
        for (a, b) in nodes {
            if items.last().is_none_or(|l| a >= l.1) {
                items.push((a, b, ""));
            }
        }
        let removed = ddmin_ranges(&cur, &items, t);
        if removed.is_empty() {
            level += 1;
        } else {
            let mut r: Vec<(usize, usize, &str)> = removed.iter().map(|&i| items[i]).collect();
            r.sort_by_key(|x| x.0);
            cur = remove_ranges(&cur, &r);
            t.stats.ast_steps += removed.len();
            // Same level again: the shifted tree may expose more.
        }
    }
    cur
}

/// Replace nodes, largest first, by a child (hoist) or by `0`.
fn replace_pass(src: &str, t: &mut Tester) -> String {
    let mut cur = src.to_string();
    'outer: loop {
        if t.out_of_budget() {
            break;
        }
        let Some(tree) = t.tree(&cur) else { break };
        for i in tree.candidates() {
            let n = &tree.nodes[i];
            if n.end > cur.len() || !cur.is_char_boundary(n.start) || !cur.is_char_boundary(n.end) {
                continue;
            }
            for &c in &n.children {
                let cn = &tree.nodes[c];
                if cn.end <= cn.start
                    || cn.start < n.start
                    || cn.end > n.end
                    || (cn.start == n.start && cn.end == n.end)
                {
                    continue;
                }
                let cand = splice(&cur, n.start, n.end, &cur[cn.start..cn.end]);
                if t.test(&cand) {
                    cur = cand;
                    t.stats.replace_steps += 1;
                    continue 'outer;
                }
            }
            if n.end - n.start > 1 {
                let cand = splice(&cur, n.start, n.end, "0");
                if t.test(&cand) {
                    cur = cand;
                    t.stats.replace_steps += 1;
                    continue 'outer;
                }
            }
            if t.out_of_budget() {
                break 'outer;
            }
        }
        break;
    }
    cur
}

/// Byte ranges of comments (a small scanner aware of strings, templates and regexps).
pub fn comments(src: &str) -> Vec<(usize, usize)> {
    let b = src.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    let mut prev_operand = false;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let s = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push((s, i));
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let s = i;
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push((s, i));
        } else if c == b'\'' || c == b'"' || c == b'`' {
            i += 1;
            while i < b.len() && b[i] != c {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            prev_operand = true;
        } else if c == b'/' && !prev_operand {
            i += 1;
            let mut class = false;
            while i < b.len() && b[i] != b'\n' && (class || b[i] != b'/') {
                match b[i] {
                    b'\\' => i += 1,
                    b'[' => class = true,
                    b']' => class = false,
                    _ => {}
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            prev_operand = true;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else {
            prev_operand = c.is_ascii_alphanumeric()
                || matches!(c, b'_' | b'$' | b')' | b']' | b'}')
                || c >= 0x80;
            if prev_operand && (c.is_ascii_alphabetic() || c == b'_' || c == b'$') {
                let s = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$')
                {
                    i += 1;
                }
                // Keywords after which `/` starts a regexp.
                if matches!(
                    &src[s..i],
                    "return"
                        | "typeof"
                        | "case"
                        | "do"
                        | "else"
                        | "in"
                        | "of"
                        | "new"
                        | "delete"
                        | "void"
                        | "throw"
                        | "yield"
                        | "await"
                        | "instanceof"
                ) {
                    prev_operand = false;
                }
                continue;
            }
            i += 1;
        }
        while i < b.len() && !src.is_char_boundary(i) {
            i += 1;
        }
    }
    out
}

fn comment_pass(src: &str, t: &mut Tester) -> String {
    let cs = comments(src);
    if cs.is_empty() {
        return src.to_string();
    }
    // A block comment spanning lines becomes a newline (ASI), any other one a space.
    let items: Vec<(usize, usize, &str)> = cs
        .iter()
        .map(|&(a, b)| {
            let fill = if src[a..b].starts_with("//") {
                ""
            } else if src[a..b].contains('\n') {
                "\n"
            } else {
                " "
            };
            (a, b, fill)
        })
        .collect();
    let removed = ddmin_ranges(src, &items, t);
    t.stats.comment_steps += removed.len();
    let mut r: Vec<(usize, usize, &str)> = removed.iter().map(|&i| items[i]).collect();
    r.sort_by_key(|x| x.0);
    remove_ranges(src, &r)
}

/// Trim trailing spaces and drop blank lines (one test).
fn cleanup(src: &str, t: &mut Tester) -> String {
    let cand = join(
        &src.lines()
            .map(|l| l.trim_end())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>(),
    );
    if cand != src && t.test(&cand) {
        cand
    } else {
        src.to_string()
    }
}

/// Line-level ddmin only (no tree, no parse-validity check).
pub fn ddmin_lines(src: &str, pred: &mut dyn Predicate, budget: usize) -> (String, Stats) {
    let mut t = Tester {
        pred,
        trees: None,
        results: HashMap::new(),
        valid: HashMap::new(),
        cfg: Config::calls(budget),
        t0: Instant::now(),
        stats: Stats::default(),
    };
    let out = ddmin_lines_with(src, &mut t);
    t.stats.line_steps = t.stats.tests;
    (out, t.stats)
}

fn join(lines: &[&str]) -> String {
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

fn ddmin_lines_with(src: &str, t: &mut Tester) -> String {
    // Lines as byte ranges of the ORIGINAL text, each with its own terminator: a `\r\n`
    // stays `\r\n` and a missing final newline stays missing, so a candidate differs from
    // `src` only by the removed lines and every adopted text is one the predicate accepted
    // (with nothing removed the result is `src` itself, already known to satisfy it).
    let mut items = vec![];
    let mut at = 0;
    for l in src.split_inclusive('\n') {
        items.push((at, at + l.len(), ""));
        at += l.len();
    }
    if items.len() < 2 {
        return src.to_string();
    }
    let removed = ddmin_ranges(src, &items, t);
    let mut r: Vec<(usize, usize, &str)> = removed.iter().map(|&i| items[i]).collect();
    r.sort_by_key(|x| x.0);
    let out = remove_ranges(src, &r);
    if out.is_empty() { src.to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy tree source: every `{...}` group is a node under the root, nested properly;
    /// unbalanced braces = "does not parse".
    struct Braces;
    impl TreeSource for Braces {
        fn tree(&mut self, src: &str) -> Option<Tree> {
            let mut t = Tree::default();
            t.nodes.push(Node {
                start: 0,
                end: src.len(),
                token: "SCRIPT".into(),
                children: vec![],
            });
            let mut stack: Vec<(usize, usize)> = vec![];
            for (i, c) in src.char_indices() {
                if c == '{' {
                    let k = t.nodes.len();
                    t.nodes.push(Node {
                        start: i,
                        end: i,
                        token: "BLOCK".into(),
                        children: vec![],
                    });
                    let parent = stack.last().map(|s| s.1).unwrap_or(0);
                    t.nodes[parent].children.push(k);
                    stack.push((i, k));
                } else if c == '}' {
                    let (_, k) = stack.pop()?;
                    t.nodes[k].end = i + 1;
                }
            }
            stack.is_empty().then_some(t)
        }
    }

    #[test]
    fn ddmin_keeps_marker() {
        let src = (0..40)
            .map(|i| format!("line{i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut p = |s: &str| s.contains("line17;") && s.contains("line3;");
        let (out, st) = ddmin_lines(&src, &mut p, 10_000);
        assert_eq!(out, "line3;\nline17;\n");
        assert!(st.tests > 0);
    }

    #[test]
    fn hdd_removes_siblings_and_keeps_validity() {
        let src = "a;\nif (x) { b; { X; } c; { d; { e; } } }\n{ f; }\nd;\n";
        let mut calls = vec![];
        let mut p = |s: &str| {
            calls.push(s.to_string());
            s.contains('X')
        };
        let (out, st) = reduce(src, &mut Braces, &mut p, 1000);
        assert!(out.contains('X'));
        assert!(!out.contains("f;") && !out.contains("e;"), "{out}");
        // Every predicate call saw balanced braces (the validity check ran first).
        assert!(calls.iter().all(|c| Braces.tree(c).is_some()));
        assert!(st.invalid > 0 || st.parses > 0);
    }

    #[test]
    fn deterministic() {
        let src = "{ a { b } c { X { y } } }\n// note\nz;\n/* q */ w;\n";
        let run = || {
            let mut p = |s: &str| s.contains('X') && s.contains('w');
            reduce(src, &mut Braces, &mut p, 500).0
        };
        assert_eq!(run(), run());
        assert!(!run().contains("note"));
    }

    #[test]
    fn budget_and_deadline() {
        let src = (0..200).map(|i| format!("s{i};\n")).collect::<String>();
        let mut p = |s: &str| s.contains("s7;");
        let (_, st) = reduce(&src, &mut Braces, &mut p, 10);
        assert!(st.budget_exhausted && st.tests <= 10);
        let cfg = Config {
            deadline: Some(Duration::from_millis(0)),
            ..Config::calls(10_000)
        };
        let mut p = |s: &str| s.contains("s7;");
        let (out, st) = reduce_with(&src, &mut Braces, &mut p, &cfg);
        assert!(st.deadline_hit);
        assert!(out.contains("s7;"));
    }

    #[test]
    fn comments_scanner() {
        let src = "a = b / c; // x\nr = /\\/\\*/g; /* y */ s = '//';";
        let cs = comments(src);
        let texts: Vec<&str> = cs.iter().map(|&(a, b)| &src[a..b]).collect();
        assert_eq!(texts, vec!["// x", "/* y */"]);
    }

    #[test]
    fn utf16_offsets() {
        let src = "'\u{1F600}';x;";
        let dump: Value = serde_json::json!({"ast": {"token": "SCRIPT", "source_offset": 0, "length": 7,
            "children": [{"token": "EXPR_RESULT", "source_offset": 5, "length": 2, "children": []}]}});
        let t = Tree::from_parse_dump(&dump, src).unwrap();
        let n = &t.nodes[1];
        assert_eq!(&src[n.start..n.end], "x;");
    }

    /// Regression: CRLF and a missing final newline survive line ddmin,
    /// and the result always satisfies the predicate.
    #[test]
    fn line_ddmin_keeps_crlf_and_predicate() {
        struct P(fn(&str) -> bool);
        impl Predicate for P {
            fn holds(&mut self, s: &str) -> bool {
                (self.0)(s)
            }
        }
        struct NoTree;
        impl TreeSource for NoTree {
            fn tree(&mut self, _: &str) -> Option<Tree> {
                None
            }
        }
        let src = "var a = 1;\r\nvar b = 2;\r\n";
        let mut p = P(|s| s.contains("\r\n"));
        let (out, _) = ddmin_lines(src, &mut p, 100);
        assert!(out.contains("\r\n"), "{out:?}");
        assert!(out.len() < src.len(), "{out:?}");
        let mut p = P(|s| s.contains("\r\n"));
        let (out, _) = reduce(src, &mut NoTree, &mut p, 100);
        assert!(out.contains("\r\n"), "{out:?}");
        let mut p = P(|s| !s.ends_with('\n') && s.contains("b()"));
        let (out, _) = ddmin_lines("a();\nb();", &mut p, 100);
        assert_eq!(out, "b();");
        let mut p = P(|s| s == "a();\r\nb();");
        let (out, _) = reduce("a();\r\nb();", &mut NoTree, &mut p, 100);
        assert_eq!(out, "a();\r\nb();");
    }
}
