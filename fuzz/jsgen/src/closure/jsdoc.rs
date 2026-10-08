//! Closure JSDoc type expressions (`{...}` contents).

use crate::engine::Gen;

const PRIMS: &[&str] = &[
    "number",
    "string",
    "boolean",
    "*",
    "?",
    "undefined",
    "null",
    "Object",
    "symbol",
    "bigint",
];

/// Emit a random type expression of bounded size.
pub fn type_expr(g: &mut Gen, budget: u32) {
    if budget == 0 || g.rng.chance(1, 2) {
        let p = *g.rng.pick(PRIMS);
        g.w(p);
        return;
    }
    match g.rng.below(6) {
        0 => {
            g.w("!Array<");
            type_expr(g, budget - 1);
            g.w(">");
        }
        1 => {
            g.w("(");
            type_expr(g, budget - 1);
            g.w("|");
            type_expr(g, budget - 1);
            g.w(")");
        }
        2 => {
            g.w("?");
            let p = *g.rng.pick(&["number", "string", "Object", "Function"]);
            g.w(p);
        }
        3 => {
            g.w("function(");
            type_expr(g, budget - 1);
            g.w("): ");
            type_expr(g, budget - 1);
        }
        4 => {
            g.w("{a: ");
            type_expr(g, budget - 1);
            g.w(", b: ");
            type_expr(g, budget - 1);
            g.w("}");
        }
        _ => {
            g.w("!Object<string, ");
            type_expr(g, budget - 1);
            g.w(">");
        }
    }
}

/// `/** @type {T} */ ` prefix.
pub fn type_annotation(g: &mut Gen, tag: &str) {
    g.w("/** @");
    g.w(tag);
    g.w(" {");
    type_expr(g, 2);
    g.w("} */ ");
}
