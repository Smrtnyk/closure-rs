//! Well-typed values for Closure type annotations.
//!
//! Random `g.expr()` output almost never matches a declared JSDoc type, so annotated code
//! built from it fails the type checker and ADVANCED output degenerates. The idioms in this
//! directory use [`Ty`] instead: a small closed set of types, each with a JSDoc spelling and
//! a generator of values that the Closure type checker accepts for it.

use crate::engine::Gen;

/// A JSDoc type with a known value generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Num,
    Str,
    Bool,
    NumArray,
    StrNumMap,
    /// `function(number): number`
    NumFn,
    /// `?number` (nullable)
    NullableNum,
}

pub const ALL: &[Ty] = &[
    Ty::Num,
    Ty::Str,
    Ty::Bool,
    Ty::NumArray,
    Ty::StrNumMap,
    Ty::NumFn,
    Ty::NullableNum,
];

/// Scalar types (weighted toward `number`, which most arithmetic idioms use).
pub const SCALARS: &[Ty] = &[Ty::Num, Ty::Num, Ty::Num, Ty::Str, Ty::Bool];

impl Ty {
    /// The JSDoc type expression (without braces).
    pub fn jsdoc(self) -> &'static str {
        match self {
            Ty::Num => "number",
            Ty::Str => "string",
            Ty::Bool => "boolean",
            Ty::NumArray => "!Array<number>",
            Ty::StrNumMap => "!Object<string, number>",
            Ty::NumFn => "function(number): number",
            Ty::NullableNum => "?number",
        }
    }
}

pub fn pick(g: &mut Gen) -> Ty {
    *g.rng.pick(ALL)
}

pub fn pick_scalar(g: &mut Gen) -> Ty {
    *g.rng.pick(SCALARS)
}

/// A literal of type `ty`; never recurses into the engine.
pub fn literal(g: &mut Gen, ty: Ty) {
    match ty {
        Ty::Num => {
            let v = *g
                .rng
                .pick(&["0", "1", "2", "3", "7", "10", "42", "0.5", "-1", "255"]);
            g.w(v);
        }
        Ty::Str => {
            let v = *g
                .rng
                .pick(&["''", "'a'", "'xy'", "'hello'", "'k1'", "'\\u00e9'"]);
            g.w(v);
        }
        Ty::Bool => {
            let v = *g.rng.pick(&["true", "false"]);
            g.w(v);
        }
        Ty::NumArray => {
            let n = g.rng.range(0, 3);
            g.w("[");
            for i in 0..n {
                if i > 0 {
                    g.w(", ");
                }
                literal(g, Ty::Num);
            }
            g.w("]");
        }
        Ty::StrNumMap => {
            let n = g.rng.range(0, 2);
            g.w("{");
            for i in 0..n {
                if i > 0 {
                    g.w(", ");
                }
                g.w(&format!("'q{i}': "));
                literal(g, Ty::Num);
            }
            g.w("}");
        }
        Ty::NumFn => {
            let op = *g.rng.pick(&["+", "*", "-"]);
            g.w("(v) => v ");
            g.w(op);
            g.w(" ");
            literal(g, Ty::Num);
        }
        Ty::NullableNum => {
            if g.rng.chance(1, 3) {
                g.w("null");
            } else {
                literal(g, Ty::Num);
            }
        }
    }
}

/// A value of type `ty`, possibly built from typed names in `env` (name, type).
pub fn value(g: &mut Gen, ty: Ty, env: &[(String, Ty)]) {
    let same: Vec<&String> = env
        .iter()
        .filter(|(_, t)| *t == ty)
        .map(|(n, _)| n)
        .collect();
    let nums: Vec<&String> = env
        .iter()
        .filter(|(_, t)| *t == Ty::Num)
        .map(|(n, _)| n)
        .collect();
    match g.rng.below(4) {
        0 if !same.is_empty() => {
            let n = (*g.rng.pick(&same)).clone();
            g.w(&n);
        }
        1 if ty == Ty::Num && !nums.is_empty() => {
            let a = (*g.rng.pick(&nums)).clone();
            let op = *g.rng.pick(&["+", "*", "-", "%", "|", "<<"]);
            g.w(&format!("({a} {op} "));
            literal(g, Ty::Num);
            g.w(")");
        }
        1 if ty == Ty::Str && !nums.is_empty() => {
            let a = (*g.rng.pick(&nums)).clone();
            g.w(&format!("('s' + {a})"));
        }
        1 if ty == Ty::Bool && !nums.is_empty() => {
            let a = (*g.rng.pick(&nums)).clone();
            let op = *g.rng.pick(&["<", ">", "===", "!=="]);
            g.w(&format!("({a} {op} "));
            literal(g, Ty::Num);
            g.w(")");
        }
        _ => literal(g, ty),
    }
}

/// An expression of type `number` that uses a value `v` of type `ty`.
pub fn use_as_number(g: &mut Gen, v: &str, ty: Ty) {
    match ty {
        Ty::Num => g.w(v),
        Ty::Str => g.w(&format!("{v}.length")),
        Ty::Bool => g.w(&format!("({v} ? 1 : 0)")),
        Ty::NumArray => g.w(&format!("{v}.length")),
        Ty::StrNumMap => g.w(&format!("Object.keys({v}).length")),
        Ty::NumFn => {
            g.w(&format!("{v}("));
            literal(g, Ty::Num);
            g.w(")");
        }
        Ty::NullableNum => g.w(&format!("({v} === null ? 0 : {v})")),
    }
}
