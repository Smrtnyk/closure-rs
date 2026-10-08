//! Functions: declarations, expressions (named/anonymous), arrows (expression and block
//! bodies, async), generators, async functions, async generators, IIFEs, default /
//! destructured / rest parameters, directives, `arguments`, `new.target`, prototype methods.

use super::{dialect, pattern, util};
use crate::engine::{Binding, Gen, Production, always};

/// Emit `(params)`; returns true when the parameter list is simple (only identifiers), which
/// is required for a `'use strict'` directive in the body.
pub fn params(g: &mut Gen, max: u32) -> bool {
    g.w("(");
    let n = g.rng.range(0, max);
    let mut simple = true;
    let mut names = vec![];
    for i in 0..n {
        if i > 0 {
            g.w(", ");
        }
        match g.rng.below(10) {
            0 | 1 => {
                simple = false;
                g.with_ctx(util::no_yield_await, |g| {
                    pattern::binding_pattern(g, 2, true, &mut names)
                });
                if g.rng.chance(1, 2) {
                    let d = if g.rng.chance(1, 2) { " = {}" } else { " = []" };
                    g.w(d);
                }
            }
            2 | 3 => {
                simple = false;
                let p = g.fresh("a");
                g.w(&p);
                g.w(" = ");
                g.with_ctx(util::no_yield_await, |g| g.pexpr());
                names.push(p);
            }
            _ => {
                let p = g.fresh("a");
                g.w(&p);
                names.push(p);
            }
        }
        // Earlier parameters are visible in later defaults.
        for p in names.drain(..) {
            g.declare(&p, Binding::Param);
        }
    }
    if g.rng.chance(1, 8) {
        simple = false;
        let r = g.fresh("rest");
        g.w(if n > 0 { ", ..." } else { "..." });
        g.w(&r);
        g.declare(&r, Binding::Param);
    }
    g.w(")");
    simple
}

/// `(params) { body }` with generator/async context. Directive prologues are emitted only in
/// the default dialect (a strict function body would make sloppy-only syntax invalid).
pub fn params_and_body(g: &mut Gen, generator: bool, is_async: bool) {
    g.push_scope();
    let simple = params(g, 3);
    g.w(" ");
    let directive = if simple && !dialect::sloppy() && g.rng.chance(1, 10) {
        *g.rng
            .pick(&["'use strict';", "\"use strict\";", "'use asm-not';"])
    } else {
        ""
    };
    g.with_ctx(
        |c| util::enter_fn(c, generator, is_async),
        |g| util::body(g, 4, directive, ""),
    );
    g.pop_scope();
}

fn kind(g: &mut Gen) -> (bool, bool) {
    match g.rng.below(16) {
        0 | 1 => (true, false),
        2 | 3 => (false, true),
        4 => (true, true),
        _ => (false, false),
    }
}

fn head(g: &mut Gen, generator: bool, is_async: bool) {
    if is_async {
        g.w("async ");
    }
    g.w("function");
    if generator {
        g.w("*");
    }
}

fn func_decl(g: &mut Gen) {
    let (generator, is_async) = kind(g);
    let name = g.fresh("f");
    g.declare(&name, Binding::Function);
    head(g, generator, is_async);
    g.w(" ");
    g.w(&name);
    params_and_body(g, generator, is_async);
}

fn func_expr(g: &mut Gen) {
    let (generator, is_async) = kind(g);
    head(g, generator, is_async);
    g.push_scope();
    if g.rng.chance(1, 3) {
        // Named function expression: the name is visible (and callable) inside.
        let n = g.fresh("fe");
        g.w(" ");
        g.w(&n);
        g.declare(&n, Binding::Function);
    }
    params_and_body(g, generator, is_async);
    g.pop_scope();
}

fn arrow(g: &mut Gen) {
    let is_async = g.rng.chance(1, 6);
    if is_async {
        g.w("async ");
    }
    g.push_scope();
    if g.rng.chance(1, 4) {
        // Single bare parameter: `x => ...`.
        let p = g.fresh("a");
        g.w(&p);
        g.declare(&p, Binding::Param);
    } else {
        params(g, 2);
    }
    g.w(" => ");
    if g.rng.chance(1, 2) {
        g.with_ctx(|c| util::enter_fn(c, false, is_async), |g| g.pexpr());
    } else {
        g.with_ctx(
            |c| util::enter_fn(c, false, is_async),
            |g| util::body(g, 3, "", ""),
        );
    }
    g.pop_scope();
}

fn iife(g: &mut Gen) {
    match g.rng.below(3) {
        0 => {
            g.w("(() => ");
            g.with_ctx(
                |c| util::enter_fn(c, false, false),
                |g| util::body(g, 3, "", ""),
            );
            g.w(")()");
        }
        1 => {
            g.w("(function () ");
            g.with_ctx(
                |c| util::enter_fn(c, false, false),
                |g| util::body(g, 3, "", "return this;"),
            );
            g.w(").call(");
            g.expr();
            g.w(")");
        }
        _ => {
            g.w("(");
            func_expr(g);
            g.w(")(");
            g.expr();
            g.w(")");
        }
    }
}

/// Self-contained uses of function-only meta syntax.
fn meta_function(g: &mut Gen) {
    const ALL: &[&str] = &[
        "function () { return arguments.length; }",
        "function (a) { return arguments[0] === a; }",
        "function () { return new.target === undefined; }",
        "function F() { if (!new.target) return new F(); this.v = 1; }",
        "function (...xs) { return xs.length + arguments.length; }",
        "function () { 'use strict'; return this; }",
    ];
    // `new.target` in a plain function is untranspilable to ES5/ES2015 (dialect::low_target).
    let t = if dialect::low_target() {
        let ok: Vec<&str> = ALL
            .iter()
            .copied()
            .filter(|t| !t.contains("new.target"))
            .collect();
        *g.rng.pick(&ok)
    } else {
        *g.rng.pick(ALL)
    };
    g.w("(");
    g.w(t);
    g.w(")");
}

/// Generator and async-generator drivers that actually consume what they produce.
fn generator_use(g: &mut Gen) {
    let t = *g.rng.pick(&[
        "[...(function* () { yield $x; yield* [$x, $x]; })()]",
        "(function* () { const r = yield $x; return r; })().next()",
        "Array.from((function* () { for (let i = 0; i < 3; i++) yield i * $e; })())",
        "(async function* () { yield await $e; })().next()",
        "(async () => { let s = 0; for await (const v of [$x, Promise.resolve($x)]) s += v; return s; })()",
        "(async function () { try { return await $e; } finally { $e; } })()",
    ]);
    // Inner bodies are sync generators / async functions of their own: no outer yield/await.
    g.with_ctx(|c| util::enter_fn(c, false, false), |g| util::expand(g, t));
}

/// `F.prototype.m = function () {...}` / `F.x = ...` for a visible function.
fn proto_assign(g: &mut Gen) {
    match g.visible(|b| b == Binding::Function) {
        Some(f) => {
            let m = g.fresh("pm");
            if g.rng.chance(2, 3) {
                g.w(&format!("{f}.prototype.{m} = function () "));
                g.with_ctx(
                    |c| util::enter_fn(c, false, false),
                    |g| util::body(g, 2, "", "return this;"),
                );
                g.w(";");
            } else {
                g.w(&format!("{f}.{m} = "));
                g.expr();
                g.w(";");
            }
        }
        None => {
            g.w("console.log(");
            util::reference(g);
            g.w(");");
        }
    }
}

/// `f(args);` as a statement: keeps declared functions used.
fn call_stmt(g: &mut Gen) {
    match g.visible(|b| b == Binding::Function) {
        Some(f) => {
            g.w(&f);
            g.w("(");
            let n = g.rng.range(0, 3);
            for i in 0..n {
                if i > 0 {
                    g.w(", ");
                }
                g.expr();
            }
            g.w(");");
        }
        None => {
            g.w("console.log(");
            util::reference(g);
            g.w(");");
        }
    }
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "function_decl",
        weight: 9,
        leaf: false,
        when: always,
        emit: func_decl,
    },
    Production {
        name: "call_stmt",
        weight: 8,
        leaf: false,
        when: always,
        emit: call_stmt,
    },
    Production {
        name: "proto_assign",
        weight: 1,
        leaf: false,
        when: always,
        emit: proto_assign,
    },
];

pub static EXPRESSIONS: &[Production] = &[
    Production {
        name: "function_expr",
        weight: 3,
        leaf: false,
        when: always,
        emit: func_expr,
    },
    Production {
        name: "arrow",
        weight: 4,
        leaf: false,
        when: always,
        emit: arrow,
    },
    Production {
        name: "iife",
        weight: 2,
        leaf: false,
        when: always,
        emit: iife,
    },
    Production {
        name: "meta_function",
        weight: 1,
        leaf: true,
        when: always,
        emit: meta_function,
    },
    Production {
        name: "generator_use",
        weight: 1,
        leaf: false,
        when: always,
        emit: generator_use,
    },
];
