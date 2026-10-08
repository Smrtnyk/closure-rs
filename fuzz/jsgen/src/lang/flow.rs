//! Live data flow. Generated code used to write most locals once and never
//! read them, so ADVANCED deleted it early and the data-flow passes (flowSensitiveInlineVariables,
//! coalesceVariableNames, deadAssignmentsElimination, collapseVariableDeclarations, ...) had
//! nothing to work on. These productions build values that are read later:
//! - locals computed from parameters and earlier locals (not constant-foldable);
//! - reassignment in both arms of a branch, and plain read-modify-write;
//! - loop-carried accumulators (`for` and `while`), with an early `break`;
//! - several short-lived locals with disjoint live ranges in one function (coalescing);
//! - small inner helpers and `Math` calls whose results are used;
//! - every flow function is called at least twice with different arguments and the results are
//!   logged, so neither inlining nor dead-code removal can drop the body.
//!
//! All names are fresh, so the productions never depend on (or disturb) other bindings, except
//! that `flow_block` may read visible value bindings as its inputs.

use crate::engine::{Binding, Gen, Production, always};

/// Readable values in the function being built: `(name, mutable)`.
struct Vals(Vec<(String, bool)>);

impl Vals {
    fn operand(&self, g: &mut Gen) -> String {
        if self.0.is_empty() || g.rng.chance(1, 5) {
            return format!("{}", g.rng.range(0, 9));
        }
        let i = g.rng.below(self.0.len() as u64) as usize;
        self.0[i].0.clone()
    }

    fn mutable(&self, g: &mut Gen) -> Option<String> {
        let m: Vec<&String> = self.0.iter().filter(|(_, m)| *m).map(|(n, _)| n).collect();
        if m.is_empty() {
            return None;
        }
        Some(m[g.rng.below(m.len() as u64) as usize].clone())
    }
}

const OPS: &[&str] = &["+", "-", "*", "|", "&", "^", "<<", ">>", "%", "+", "-", "*"];
const CMPS: &[&str] = &["<", ">", "<=", ">=", "===", "!=="];

fn arith(g: &mut Gen, v: &Vals) -> String {
    let a = v.operand(g);
    let b = v.operand(g);
    let op = *g.rng.pick(OPS);
    if g.rng.chance(1, 4) {
        let c = v.operand(g);
        let op2 = *g.rng.pick(OPS);
        format!("({a} {op} {b}) {op2} {c}")
    } else {
        format!("{a} {op} {b}")
    }
}

fn cond(g: &mut Gen, v: &Vals) -> String {
    let a = v.operand(g);
    let b = v.operand(g);
    let c = *g.rng.pick(CMPS);
    format!("{a} {c} {b}")
}

/// One data-flow step at the current output position (`ind` = indentation prefix).
fn step(g: &mut Gen, v: &mut Vals, ind: &str, sink: &str) {
    match g.rng.below(11) {
        // New local from earlier values.
        0..=2 => {
            let kw = *g.rng.pick(&["let", "var", "const", "let"]);
            let x = g.fresh("x");
            let e = arith(g, v);
            g.w(&format!("{ind}{kw} {x} = {e};"));
            v.0.push((x, kw != "const"));
        }
        // Reassignment in both arms of a branch.
        3 => {
            let Some(m) = v.mutable(g) else {
                return step(g, v, ind, sink);
            };
            let c = cond(g, v);
            let e1 = arith(g, v);
            let op = *g.rng.pick(&["+", "-", "*", "|"]);
            let e2 = v.operand(g);
            g.w(&format!(
                "{ind}if ({c}) {{ {m} = {e1}; }} else {{ {m} {op}= {e2}; }}"
            ));
        }
        // Read-modify-write.
        4 => {
            let Some(m) = v.mutable(g) else {
                return step(g, v, ind, sink);
            };
            let e = arith(g, v);
            g.w(&format!("{ind}{m} = {m} + ({e});"));
        }
        // Loop-carried accumulator (`for`).
        5 => {
            let acc = g.fresh("acc");
            let i = g.fresh("i");
            let n = v.operand(g);
            let e = v.operand(g);
            let lim = g.rng.range(20, 200);
            g.w(&format!(
                "{ind}let {acc} = 0;\n{ind}for (let {i} = 0; {i} < {n}; {i}++) {{ {acc} += {e} * {i}; if ({acc} > {lim}) {{ break; }} }}"
            ));
            v.0.push((acc, true));
        }
        // Loop-carried accumulator (`while`, counter consumed).
        6 => {
            let k = g.fresh("k");
            let s = g.fresh("s");
            let n = v.operand(g);
            let e = arith(g, v);
            g.w(&format!(
                "{ind}var {k} = {n} & 7, {s} = 1;\n{ind}while ({k} > 0) {{ {s} = {s} * 2 + ({e}); {k}--; }}"
            ));
            v.0.push((s, true));
        }
        // Short-lived locals with disjoint live ranges (coalescing candidates).
        7 => {
            for _ in 0..g.rng.range(2, 3) {
                let t = g.fresh("t");
                let e = arith(g, v);
                g.w(&format!("{ind}var {t} = {e};\n{ind}{sink}({t});\n"));
            }
            // Drop the trailing newline: the caller adds one per step.
            g.out.pop();
        }
        // Inner helper whose result is used.
        8 => {
            let h = g.fresh("h");
            let q = g.fresh("q");
            let a = v.operand(g);
            let op = *g.rng.pick(OPS);
            let x = g.fresh("x");
            let arg = v.operand(g);
            g.w(&format!(
                "{ind}const {h} = ({q}) => {q} {op} {a};\n{ind}let {x} = {h}({arg}) + {h}({arg} + 1);"
            ));
            v.0.push((x, true));
        }
        // Conditional value / Math call.
        9 => {
            let x = g.fresh("x");
            let a = v.operand(g);
            let b = v.operand(g);
            let e = if g.rng.chance(1, 2) {
                let c = cond(g, v);
                format!("{c} ? {a} - {b} : {b} - {a}")
            } else {
                let f = *g.rng.pick(&["max", "min", "abs", "floor"]);
                if f == "abs" || f == "floor" {
                    format!("Math.{f}({a} - {b})")
                } else {
                    format!("Math.{f}({a}, {b})")
                }
            };
            g.w(&format!("{ind}let {x} = {e};"));
            v.0.push((x, true));
        }
        // Observe a value mid-function.
        _ => {
            let a = v.operand(g);
            g.w(&format!("{ind}{sink}({a});"));
        }
    }
}

fn steps(g: &mut Gen, v: &mut Vals, ind: &str, sink: &str) {
    let n = g.rng.range(3, 9);
    for _ in 0..n {
        g.nl();
        step(g, v, ind, sink);
    }
}

fn args(g: &mut Gen, n: usize) -> String {
    (0..n)
        .map(|_| format!("{}", g.rng.range(0, 12)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `function f(p..) { <data flow> return <value>; }` called at least twice, results logged.
fn flow_fn(g: &mut Gen) {
    let f = g.fresh("flow");
    let np = g.rng.range(1, 3) as usize;
    let ps: Vec<String> = (0..np).map(|_| g.fresh("p")).collect();
    let mut v = Vals(ps.iter().map(|p| (p.clone(), true)).collect());
    g.w(&format!("function {f}({}) {{", ps.join(", ")));
    steps(g, &mut v, "  ", "console.log");
    let r = arith(g, &v);
    g.nl();
    g.w(&format!("  return {r};"));
    g.nl();
    g.w("}");
    g.declare(&f, Binding::Function);
    g.nl();
    let calls = g.rng.range(2, 3);
    let cs: Vec<String> = (0..calls)
        .map(|_| {
            let a = args(g, np);
            format!("{f}({a})")
        })
        .collect();
    if g.rng.chance(1, 3) {
        // Results flow into a further computation before being observed.
        let r = g.fresh("r");
        g.w(&format!("let {r} = {} + {};", cs[0], cs[1]));
        g.nl();
        g.w(&format!(
            "console.log({r}{});",
            cs[2..].iter().map(|c| format!(", {c}")).collect::<String>()
        ));
    } else {
        g.w(&format!("console.log({});", cs.join(", ")));
    }
}

/// `{ <data flow over visible values> console.log(..); }` in place.
fn flow_block(g: &mut Gen) {
    let mut v = Vals(vec![]);
    g.w("{");
    for _ in 0..2 {
        if let Some(n) = g.visible(|b| {
            matches!(
                b,
                Binding::Var | Binding::Let | Binding::Const | Binding::Param
            )
        }) {
            // Copy into a fresh local (the outer binding may be const, or hold a BigInt, which
            // must not meet number operators).
            let x = g.fresh("x");
            g.nl();
            g.w(&format!("  let {x} = typeof {n} === 'number' ? {n} : 3;"));
            v.0.push((x, true));
        }
    }
    steps(g, &mut v, "  ", "console.log");
    let a = v.operand(g);
    let b = v.operand(g);
    g.nl();
    g.w(&format!("  console.log({a}, {b});"));
    g.nl();
    g.w("}");
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "flow_fn",
        weight: 8,
        leaf: false,
        when: always,
        emit: flow_fn,
    },
    Production {
        name: "flow_block",
        weight: 4,
        leaf: false,
        when: always,
        emit: flow_block,
    },
];
