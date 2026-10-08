//! Shared helpers for `lang/` productions: function-context resets, bodies with directives
//! and a tiny template expander used for constant-foldable and idiomatic shapes.

use super::literal;
use crate::engine::{Binding, Ctx, Gen};

/// Context of a fresh function body (not an arrow: arrows keep nothing of this either, as
/// `new.target`/`arguments` are only emitted by self-contained productions).
pub fn enter_fn(c: &mut Ctx, generator: bool, is_async: bool) {
    c.in_function = true;
    c.in_generator = generator;
    c.in_async = is_async;
    c.in_loop = false;
    c.in_switch = false;
    c.labels.clear();
    c.terminated = false;
}

/// Context of an expression evaluated outside any function body that could hold
/// `yield`/`await` (parameter defaults, class field initialisers, computed keys of classes).
pub fn no_yield_await(c: &mut Ctx) {
    c.in_generator = false;
    c.in_async = false;
}

/// `{ [prefix] stmt* [suffix] }` in a fresh scope. `prefix` is written first (e.g. a
/// directive); `suffix` (e.g. `break;`) is written last unless a jump already ended the body.
pub fn body(g: &mut Gen, max: u32, prefix: &str, suffix: &str) {
    g.w("{");
    g.push_scope();
    if !prefix.is_empty() {
        g.w(" ");
        g.w(prefix);
    }
    let n = if g.at_cap() { 0 } else { g.rng.range(0, max) };
    let outer = std::mem::take(&mut g.ctx.terminated);
    for _ in 0..n {
        g.nl();
        g.w("  ");
        g.stmt();
        if g.ctx.terminated {
            break;
        }
    }
    if !suffix.is_empty() && !g.ctx.terminated {
        g.nl();
        g.w("  ");
        g.w(suffix);
    }
    g.ctx.terminated = outer;
    g.pop_scope();
    g.nl();
    g.w("}");
}

/// A visible non-class name, or a global fallback.
pub fn reference(g: &mut Gen) {
    match g.visible(|b| !matches!(b, Binding::Class)) {
        Some(n) => g.w(&n),
        None => {
            let s = *g
                .rng
                .pick(&["globalThis", "undefined", "NaN", "Math.PI", "this"]);
            g.w(s);
        }
    }
}

/// A visible mutable name, if any.
pub fn mutable(g: &mut Gen) -> Option<String> {
    g.visible(Binding::mutable)
}

/// Expand a shape template:
/// `$n` number literal, `$s` string literal, `$i` small integer, `$r` reference,
/// `$e` parenthesised sub-expression, `$x` bare sub-expression (only where an
/// AssignmentExpression is allowed), `$f` callable name, `$B` block, `$$` a literal `$`.
pub fn expand(g: &mut Gen, t: &str) {
    let mut it = t.chars().peekable();
    let mut buf = String::new();
    while let Some(c) = it.next() {
        if c != '$' {
            buf.push(c);
            continue;
        }
        let k = it.next().unwrap_or('$');
        if k == '$' {
            buf.push('$');
            continue;
        }
        g.w(&buf);
        buf.clear();
        match k {
            'n' => literal::number_lit(g),
            's' => literal::string_lit(g),
            'i' => {
                let v = g.rng.range(0, 9);
                g.w(&v.to_string());
            }
            'r' => reference(g),
            'e' => g.pexpr(),
            'x' => g.expr(),
            'f' => match g.visible(|b| b == Binding::Function) {
                Some(f) => g.w(&f),
                None => g.w("String"),
            },
            'B' => g.block(2),
            other => {
                g.w("$");
                g.w(&other.to_string());
            }
        }
    }
    g.w(&buf);
}
