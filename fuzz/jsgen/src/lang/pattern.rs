//! Destructuring: binding patterns (object/array, nested, defaults, holes, rest) in
//! declarations, parameters, `for-of` heads and `catch` clauses.

use super::util;
use crate::engine::{Binding, Gen, Production, always};

/// Write a binding pattern. Bound names are pushed to `names`; the caller declares them
/// (after the pattern, so defaults never see names from the same pattern too early).
pub fn binding_pattern(g: &mut Gen, depth: u32, defaults: bool, names: &mut Vec<String>) {
    if g.rng.chance(1, 2) {
        // Object pattern.
        g.w("{");
        let n = g.rng.range(1, 3);
        for i in 0..n {
            if i > 0 {
                g.w(", ");
            }
            match g.rng.below(6) {
                0 if depth > 0 => {
                    let k = *g.rng.pick(&["x", "y", "a"]);
                    g.w(&format!("{k}: "));
                    binding_pattern(g, depth - 1, defaults, names);
                }
                1 => {
                    // Shorthand: the property name is the binding name.
                    let v = g.fresh("s");
                    g.w(&v);
                    maybe_default(g, defaults);
                    names.push(v);
                }
                2 => {
                    let v = g.fresh("d");
                    g.w("[");
                    g.expr();
                    g.w(&format!("]: {v}"));
                    names.push(v);
                }
                3 => {
                    let v = g.fresh("d");
                    let k = *g.rng.pick(&["'q-k'", "0", "length"]);
                    g.w(&format!("{k}: {v}"));
                    maybe_default(g, defaults);
                    names.push(v);
                }
                _ => {
                    let v = g.fresh("d");
                    let k = *g.rng.pick(&["x", "y", "a", "b"]);
                    g.w(&format!("{k}: {v}"));
                    maybe_default(g, defaults);
                    names.push(v);
                }
            }
        }
        if g.rng.chance(1, 5) {
            let r = g.fresh("r");
            g.w(&format!(", ...{r}"));
            names.push(r);
        }
        g.w("}");
    } else {
        // Array pattern.
        g.w("[");
        let n = g.rng.range(1, 3);
        for i in 0..n {
            if i > 0 {
                g.w(", ");
            }
            match g.rng.below(6) {
                0 if depth > 0 => binding_pattern(g, depth - 1, defaults, names),
                1 => {} // elision
                _ => {
                    let v = g.fresh("d");
                    g.w(&v);
                    maybe_default(g, defaults);
                    names.push(v);
                }
            }
        }
        if g.rng.chance(1, 5) {
            g.w(", ...");
            if depth > 0 && g.rng.chance(1, 3) {
                binding_pattern(g, 0, false, names);
            } else {
                let r = g.fresh("r");
                g.w(&r);
                names.push(r);
            }
        }
        g.w("]");
    }
}

fn maybe_default(g: &mut Gen, defaults: bool) {
    if defaults && g.rng.chance(1, 3) {
        g.w(" = ");
        g.pexpr();
    }
}

/// An initializer that usually matches the pattern shape (so defaults and rest do work).
fn initializer(g: &mut Gen) {
    match g.rng.below(4) {
        0 => {
            g.w("{x: ");
            g.expr();
            g.w(", y: [");
            g.expr();
            g.w(", 2], a: {x: 1, y: 2}, b: void 0, length: 3, 'q-k': 0}");
        }
        1 => {
            g.w("[");
            g.expr();
            g.w(", [1, 2], {x: 3}, ");
            g.expr();
            g.w("]");
        }
        2 => util::reference(g),
        _ => g.pexpr(),
    }
}

fn destructuring_decl(g: &mut Gen) {
    let kw = *g.rng.pick(&["let", "const", "var"]);
    let b = match kw {
        "let" => Binding::Let,
        "const" => Binding::Const,
        _ => Binding::Var,
    };
    let mut names = vec![];
    g.w(kw);
    g.w(" ");
    g.with_ctx(util::no_yield_await, |g| {
        binding_pattern(g, 2, true, &mut names)
    });
    g.w(" = ");
    initializer(g);
    g.w(";");
    for n in names {
        g.declare(&n, b);
    }
}

/// `for (const {x, y} of [...])` / `for (let [k, v] of Object.entries(o))`.
fn for_of_pattern(g: &mut Gen) {
    let mut names = vec![];
    let kw = *g.rng.pick(&["const", "let", "var"]);
    g.w(&format!("for ({kw} "));
    // Closure reports "`await` is illegal in parameter default value" for an `await` in a
    // for-of pattern default, so patterns never contain yield/await.
    g.with_ctx(util::no_yield_await, |g| {
        binding_pattern(g, 1, true, &mut names)
    });
    g.w(" of ");
    if g.rng.chance(1, 2) {
        g.w("Object.entries(");
        g.expr();
        g.w(")");
    } else {
        g.w("[");
        initializer(g);
        g.w("]");
    }
    g.w(") ");
    g.push_scope();
    for n in names {
        g.declare(
            &n,
            if kw == "var" {
                Binding::Var
            } else {
                Binding::Const
            },
        );
    }
    g.with_ctx(|c| c.in_loop = true, |g| g.block(3));
    g.pop_scope();
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "destructuring_decl",
        weight: 5,
        leaf: false,
        when: always,
        emit: destructuring_decl,
    },
    Production {
        name: "for_of_pattern",
        weight: 2,
        leaf: false,
        when: always,
        emit: for_of_pattern,
    },
];
