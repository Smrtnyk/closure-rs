//! Statements: declarations, control flow, loops (always bounded), labelled loops with
//! labelled `break`/`continue`, `switch`, exceptions (incl. optional catch binding and
//! destructured catch parameters), `for await`, `debugger`, empty statements, dead /
//! foldable control flow, and sloppy-only `with` and strict-reserved identifiers.

use super::{dialect, util};
use crate::engine::{Binding, Gen, Production, always};

fn decl(g: &mut Gen) {
    let (kw, b) = *g.rng.pick(&[
        ("var", Binding::Var),
        ("let", Binding::Let),
        ("let", Binding::Let),
        ("const", Binding::Const),
        ("const", Binding::Const),
    ]);
    g.w(kw);
    let n = if g.rng.chance(1, 5) { 2 } else { 1 };
    let mut names = vec![];
    for i in 0..n {
        let name = g.fresh("v");
        g.w(if i == 0 { " " } else { ", " });
        g.w(&name);
        if b == Binding::Const || g.rng.chance(4, 5) {
            g.w(" = ");
            g.expr();
        }
        names.push(name);
    }
    g.w(";");
    for name in names {
        g.declare(&name, b);
    }
}

/// `const o = {a: .., b: ..};` followed by uses of its properties (property-level passes).
fn object_decl(g: &mut Gen) {
    let o = g.fresh("o");
    g.w(&format!("const {o} = {{a: "));
    g.expr();
    g.w(", b: ");
    g.expr();
    g.w(", m() { return this.a; }};");
    g.nl();
    match g.rng.below(3) {
        0 => g.w(&format!("{o}.a = {o}.b;")),
        1 => g.w(&format!("console.log({o}.a, {o}.m());")),
        _ => {
            g.w(&format!("{o}.c = "));
            g.expr();
            g.w(";");
        }
    }
    g.declare(&o, Binding::Const);
}

fn expr_stmt(g: &mut Gen) {
    // A leading `(` keeps `{`/`function`/`class`/`let [` from starting the statement.
    g.pexpr();
    g.w(";");
}

fn log(g: &mut Gen) {
    g.w("console.log(");
    if g.rng.chance(2, 3) {
        util::reference(g);
    } else {
        g.expr();
    }
    g.w(");");
}

fn if_stmt(g: &mut Gen) {
    g.w("if ");
    g.pexpr();
    g.w(" ");
    g.block(3);
    match g.rng.below(4) {
        0 | 1 => {
            g.w(" else ");
            g.block(3);
        }
        2 => {
            g.w(" else if ");
            g.pexpr();
            g.w(" ");
            g.block(2);
        }
        _ => {}
    }
}

/// Dead or foldable control flow: `if (true)`, `while (false)`, `if (0) .. else ..`.
fn dead_flow(g: &mut Gen) {
    let t = *g.rng.pick(&[
        "if (true) $B",
        "if (false) $B else $B",
        "if (0) $B",
        "if (!1) $B else $B",
        "if ('') $B",
        "if (typeof $r === 'number') $B",
        "if ($r) ; else $B",
        "if (1 + 1 === 2) $B",
        "while (false) $B",
        "for (; false; ) $B",
        "do ; while (false);",
        "$r && $f($r);",
        "$r || console.log($n);",
        "void 0;",
        "0;",
    ]);
    util::expand(g, t);
}

fn loop_body(g: &mut Gen) {
    g.with_ctx(|c| c.in_loop = true, |g| g.block(3));
}

/// Optionally label a loop (`LLn:`); loop labels start with `LL` so `continue LLn` knows they
/// label an iteration statement.
fn loop_label(g: &mut Gen) -> Option<String> {
    if g.rng.chance(1, 5) {
        let l = g.fresh("LL");
        g.w(&l);
        g.w(": ");
        Some(l)
    } else {
        None
    }
}

fn labelled_loop_body(g: &mut Gen, label: Option<String>) {
    g.with_ctx(
        move |c| {
            c.in_loop = true;
            if let Some(l) = label {
                c.labels.push(l);
            }
        },
        |g| g.block(3),
    );
}

fn for_stmt(g: &mut Gen) {
    let label = loop_label(g);
    let i = g.fresh("i");
    let n = g.rng.range(0, 5);
    match g.rng.below(4) {
        0 => g.w(&format!("for (var {i} = 0; {i} < {n}; {i}++) ")),
        1 => {
            let j = g.fresh("j");
            g.w(&format!(
                "for (let {i} = 0, {j} = {n}; {i} < {j}; {i}++, {j}--) "
            ));
        }
        2 => g.w(&format!("for (let {i} = {n}; {i}--; ) ")),
        _ => g.w(&format!("for (let {i} = 0; {i} < {n}; {i} += 1) ")),
    }
    g.push_scope();
    g.declare(&i, Binding::Const); // never reassigned in the body: keeps the loop bounded
    labelled_loop_body(g, label);
    g.pop_scope();
}

/// `for (;;) { ...; break; }`
fn for_ever(g: &mut Gen) {
    g.w("for (;;) ");
    g.with_ctx(|c| c.in_loop = true, |g| util::body(g, 2, "", "break;"));
}

fn while_stmt(g: &mut Gen) {
    let k = g.fresh("k");
    let n = g.rng.range(0, 4);
    g.w(&format!("let {k} = {n};"));
    g.nl();
    let label = loop_label(g);
    g.w(&format!("while ({k}-- > 0) "));
    labelled_loop_body(g, label);
}

fn do_while(g: &mut Gen) {
    let k = g.fresh("k");
    g.w(&format!("let {k} = 0;"));
    g.nl();
    let label = loop_label(g);
    g.w("do ");
    labelled_loop_body(g, label);
    g.w(&format!(" while (++{k} < 2);"));
}

fn for_of(g: &mut Gen) {
    let label = loop_label(g);
    let x = g.fresh("e");
    let kw = *g.rng.pick(&["const", "let", "var"]);
    g.w(&format!("for ({kw} {x} of "));
    match g.rng.below(3) {
        0 => {
            g.w("[");
            g.expr();
            g.w(", ");
            g.expr();
            g.w("]");
        }
        1 => {
            super::literal::string_lit(g);
        }
        _ => {
            g.w("new Set([");
            g.expr();
            g.w("])");
        }
    }
    g.w(") ");
    g.push_scope();
    g.declare(&x, Binding::Const);
    labelled_loop_body(g, label);
    g.pop_scope();
}

fn for_in(g: &mut Gen) {
    let label = loop_label(g);
    let x = g.fresh("p");
    let kw = *g.rng.pick(&["const", "let", "var"]);
    g.w(&format!("for ({kw} {x} in "));
    if g.rng.chance(1, 2) {
        g.w("{a: 1, b: ");
        g.expr();
        g.w("}");
    } else {
        g.pexpr();
    }
    g.w(") ");
    g.push_scope();
    g.declare(&x, Binding::Const);
    labelled_loop_body(g, label);
    g.pop_scope();
}

/// `for await (const x of [...])` inside async functions.
fn for_await(g: &mut Gen) {
    let x = g.fresh("aw");
    g.w(&format!("for await (const {x} of ["));
    g.expr();
    g.w(", Promise.resolve(");
    g.expr();
    g.w(")]) ");
    g.push_scope();
    g.declare(&x, Binding::Const);
    loop_body(g);
    g.pop_scope();
}

fn switch_stmt(g: &mut Gen) {
    g.w("switch ");
    g.pexpr();
    g.w(" {");
    let n = g.rng.range(1, 4);
    let default_at = g.rng.range(0, n);
    let strings = g.rng.chance(1, 3);
    g.with_ctx(
        |c| c.in_switch = true,
        |g| {
            for i in 0..=n {
                g.nl();
                if i == default_at {
                    g.w("default:");
                } else if strings {
                    g.w(&format!("case 'c{i}':"));
                } else if g.rng.chance(1, 6) {
                    g.w("case ");
                    g.pexpr();
                    g.w(":");
                } else {
                    g.w(&format!("case {i}:"));
                }
                match g.rng.below(4) {
                    0 => {} // fall through, empty clause
                    _ => {
                        g.w(" ");
                        g.block(2);
                        if g.rng.chance(1, 2) {
                            g.w(" break;");
                        }
                    }
                }
            }
        },
    );
    g.nl();
    g.w("}");
}

fn try_stmt(g: &mut Gen) {
    g.w("try ");
    g.block(3);
    let form = g.rng.below(3);
    if form != 1 {
        match g.rng.below(5) {
            0 => {
                // Optional catch binding (ES2019).
                g.w(" catch ");
                g.block(2);
            }
            1 => {
                let m = g.fresh("msg");
                g.w(&format!(" catch ({{message: {m}}}) "));
                g.push_scope();
                g.declare(&m, Binding::Let);
                g.block(2);
                g.pop_scope();
            }
            _ => {
                let e = g.fresh("err");
                g.w(&format!(" catch ({e}) "));
                g.push_scope();
                g.declare(&e, Binding::Let);
                g.block(2);
                g.pop_scope();
            }
        }
    }
    if form != 0 {
        g.w(" finally ");
        g.block(2);
    }
}

fn throw_stmt(g: &mut Gen) {
    g.ctx.terminated = true;
    if g.rng.chance(1, 3) {
        g.w("throw ");
        g.expr();
        g.w(";");
    } else {
        g.w("throw new Error(");
        g.expr();
        g.w(");");
    }
}

fn labeled(g: &mut Gen) {
    let l = g.fresh("L");
    g.w(&l);
    g.w(": ");
    let l2 = l.clone();
    g.with_ctx(move |c| c.labels.push(l2), |g| g.block(3));
}

fn break_stmt(g: &mut Gen) {
    g.ctx.terminated = true;
    let unlabeled_ok = g.ctx.in_loop || g.ctx.in_switch;
    if !g.ctx.labels.is_empty() && (!unlabeled_ok || g.rng.chance(1, 3)) {
        let l = g.rng.pick(&g.ctx.labels.clone()).clone();
        g.w(&format!("break {l};"));
    } else {
        g.w("break;");
    }
}

fn continue_stmt(g: &mut Gen) {
    g.ctx.terminated = true;
    let loop_labels: Vec<String> = g
        .ctx
        .labels
        .iter()
        .filter(|l| l.starts_with("LL"))
        .cloned()
        .collect();
    if !loop_labels.is_empty() && g.rng.chance(1, 2) {
        let l = g.rng.pick(&loop_labels).clone();
        g.w(&format!("continue {l};"));
    } else {
        g.w("continue;");
    }
}

fn return_stmt(g: &mut Gen) {
    g.ctx.terminated = true;
    if g.rng.chance(1, 5) {
        g.w("return;");
    } else {
        g.w("return ");
        g.expr();
        g.w(";");
    }
}

fn block_stmt(g: &mut Gen) {
    g.block(3);
}

fn empty_or_debugger(g: &mut Gen) {
    let s = *g.rng.pick(&[";", ";", "debugger;"]);
    g.w(s);
}

/// Sloppy only: `with (obj) { ... }`.
fn with_stmt(g: &mut Gen) {
    g.w("with (");
    if g.rng.chance(1, 2) {
        g.w("{x: 1, y: ");
        g.expr();
        g.w("}");
    } else {
        util::reference(g);
    }
    g.w(") ");
    g.block(2);
}

/// Contextual keywords are ordinary identifiers (self-contained; `var` tolerates repeats).
/// Strict-mode reserved words (`implements`, `package`, `yield`, `let`, ...) are not used:
/// Closure's parser rejects them as identifier references even with
/// `--strict_mode_input=false`.
fn contextual_keywords(g: &mut Gen) {
    let t = *g.rng.pick(&[
        "var of = $n, get = 2, set = 3; console.log(of + get + set);",
        "var async = $n, target = 1; console.log(async(0) || target);",
        "var o = {static: 1, private: 2, let: 3, yield: 4}; console.log(o.static + o.private + o.let, o.yield);",
    ]);
    util::expand(g, t);
}

/// Sloppy only: Annex B function declaration as an `if` body (self-contained). Closure
/// rejects it in every non-WHITESPACE mode (JSC_DECLARATION_NOT_DIRECTLY_IN_BLOCK), so the
/// bare form is kept at a small explicit rate (1 in 8) for the error path and the braced
/// form is emitted otherwise (it caused most sloppy compile errors).
fn annex_b_if_function(g: &mut Gen) {
    let f = g.fresh("fb");
    g.w("if ");
    g.pexpr();
    if g.rng.chance(1, 8) {
        g.w(&format!(" function {f}() {{ return 1; }}"));
    } else {
        g.w(&format!(" {{ function {f}() {{ return 1; }} }}"));
    }
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "decl",
        weight: 30,
        leaf: false,
        when: always,
        emit: decl,
    },
    Production {
        name: "object_decl",
        weight: 3,
        leaf: false,
        when: always,
        emit: object_decl,
    },
    Production {
        name: "expr_stmt",
        weight: 14,
        leaf: false,
        when: always,
        emit: expr_stmt,
    },
    Production {
        name: "log",
        weight: 10,
        leaf: false,
        when: always,
        emit: log,
    },
    Production {
        name: "if",
        weight: 10,
        leaf: false,
        when: always,
        emit: if_stmt,
    },
    Production {
        name: "dead_flow",
        weight: 3,
        leaf: false,
        when: always,
        emit: dead_flow,
    },
    Production {
        name: "for",
        weight: 5,
        leaf: false,
        when: always,
        emit: for_stmt,
    },
    Production {
        name: "for_ever",
        weight: 1,
        leaf: false,
        when: always,
        emit: for_ever,
    },
    Production {
        name: "while",
        weight: 3,
        leaf: false,
        when: always,
        emit: while_stmt,
    },
    Production {
        name: "do_while",
        weight: 2,
        leaf: false,
        when: always,
        emit: do_while,
    },
    Production {
        name: "for_of",
        weight: 3,
        leaf: false,
        when: always,
        emit: for_of,
    },
    Production {
        name: "for_in",
        weight: 2,
        leaf: false,
        when: always,
        emit: for_in,
    },
    Production {
        name: "for_await",
        weight: 3,
        leaf: false,
        when: |c| c.in_async,
        emit: for_await,
    },
    Production {
        name: "switch",
        weight: 3,
        leaf: false,
        when: always,
        emit: switch_stmt,
    },
    Production {
        name: "try",
        weight: 4,
        leaf: false,
        when: always,
        emit: try_stmt,
    },
    Production {
        name: "throw",
        weight: 1,
        leaf: false,
        when: always,
        emit: throw_stmt,
    },
    Production {
        name: "labeled",
        weight: 2,
        leaf: false,
        when: always,
        emit: labeled,
    },
    Production {
        name: "break",
        weight: 3,
        leaf: true,
        when: |c| c.in_loop || c.in_switch || !c.labels.is_empty(),
        emit: break_stmt,
    },
    Production {
        name: "continue",
        weight: 2,
        leaf: true,
        when: |c| c.in_loop,
        emit: continue_stmt,
    },
    Production {
        name: "return",
        weight: 6,
        leaf: false,
        when: |c| c.in_function,
        emit: return_stmt,
    },
    Production {
        name: "block",
        weight: 2,
        leaf: false,
        when: always,
        emit: block_stmt,
    },
    Production {
        name: "empty_or_debugger",
        weight: 1,
        leaf: true,
        when: always,
        emit: empty_or_debugger,
    },
    Production {
        name: "with",
        weight: 2,
        leaf: false,
        when: |c| dialect::sloppy() && !c.in_class_method,
        emit: with_stmt,
    },
    Production {
        name: "contextual_keywords",
        weight: 1,
        leaf: false,
        when: |c| !c.in_class_method,
        emit: contextual_keywords,
    },
    Production {
        name: "annex_b_if_function",
        weight: 1,
        leaf: false,
        when: |c| dialect::sloppy() && !c.in_class_method,
        emit: annex_b_if_function,
    },
];
