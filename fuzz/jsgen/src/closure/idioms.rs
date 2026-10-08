//! Closure idioms: JSDoc-annotated declarations, `@const`, `@enum`, `@constructor`,
//! `@record`/`@interface`, `@typedef`, `@nocollapse`, `@suppress`.

use super::jsdoc::{type_annotation, type_expr};
use super::typed;
use crate::engine::{Binding, Gen, Production, always};

fn typed_decl(g: &mut Gen) {
    let n = g.fresh("t");
    let kw = *g.rng.pick(&["let", "var"]);
    if g.rng.chance(3, 4) {
        // Type-correct: the value matches the annotation.
        let ty = typed::pick(g);
        g.w(&format!("/** @type {{{}}} */ {kw} {n} = ", ty.jsdoc()));
        typed::literal(g, ty);
    } else {
        type_annotation(g, "type");
        g.w(&format!("{kw} {n} = "));
        g.expr();
    }
    g.w(";");
    g.declare(
        &n,
        if kw == "let" {
            Binding::Let
        } else {
            Binding::Var
        },
    );
}

fn const_decl(g: &mut Gen) {
    let n = g.fresh("K");
    g.w(&format!("/** @const */ var {n} = "));
    g.expr();
    g.w(";");
    g.declare(&n, Binding::Const);
}

fn enum_decl(g: &mut Gen) {
    let n = g.fresh("E");
    let ty = *g.rng.pick(&["number", "string"]);
    g.w(&format!("/** @enum {{{ty}}} */ const {n} = {{"));
    let k = g.rng.range(1, 4);
    for i in 0..k {
        if i > 0 {
            g.w(", ");
        }
        if ty == "number" {
            g.w(&format!("K{i}: {i}"));
        } else {
            g.w(&format!("K{i}: 'k{i}'"));
        }
    }
    g.w("};");
    g.declare(&n, Binding::Const);
}

fn annotated_function(g: &mut Gen) {
    let n = g.fresh("af");
    let p = g.fresh("a");
    if g.rng.chance(2, 3) {
        // Type-correct: typed parameter, typed return built from it.
        let (pt, rt) = (typed::pick(g), typed::pick_scalar(g));
        g.w(&format!(
            "/**\n * @param {{{}}} {p}\n * @return {{{}}}\n */\nfunction {n}({p}) {{ return ",
            pt.jsdoc(),
            rt.jsdoc()
        ));
        typed::value(g, rt, &[(p.clone(), pt)]);
        g.w("; }");
        g.declare(&n, Binding::Function);
        return;
    }
    g.w("/**\n * @param {");
    type_expr(g, 1);
    g.w(&format!("}} {p}\n * @return {{"));
    type_expr(g, 1);
    g.w("}\n */\n");
    g.w(&format!("function {n}({p}) {{ return "));
    g.push_scope();
    g.declare(&p, Binding::Param);
    g.with_ctx(
        |c| {
            c.in_function = true;
            c.in_generator = false;
            c.in_async = false;
        },
        |g| g.expr(),
    );
    g.pop_scope();
    g.w("; }");
    g.declare(&n, Binding::Function);
}

fn es5_constructor(g: &mut Gen) {
    let n = g.fresh("Ctor");
    g.w(&format!("/** @constructor */ function {n}() {{ this.x = "));
    g.with_ctx(
        |c| {
            c.in_generator = false;
            c.in_async = false;
        },
        |g| g.expr(),
    );
    g.w("; }");
    g.nl();
    let m = g.fresh("pm");
    g.w(&format!(
        "{n}.prototype.{m} = function() {{ return this.x; }};"
    ));
    g.declare(&n, Binding::Function);
}

fn record(g: &mut Gen) {
    let n = g.fresh("R");
    let kind = *g.rng.pick(&["record", "interface"]);
    g.w(&format!("/** @{kind} */ function {n}() {{}}"));
    g.nl();
    g.w("/** @type {");
    type_expr(g, 1);
    g.w(&format!("}} */ {n}.prototype.field;"));
}

fn typedef(g: &mut Gen) {
    let n = g.fresh("T");
    g.w("/** @typedef {");
    type_expr(g, 2);
    g.w(&format!("}} */ let {n};"));
}

fn nocollapse_static(g: &mut Gen) {
    let c = g.fresh("S");
    g.w(&format!("class {c} {{}}"));
    g.nl();
    g.w(&format!("/** @nocollapse */ {c}.v = "));
    g.expr();
    g.w(";");
    g.declare(&c, Binding::Class);
}

fn cast(g: &mut Gen) {
    if g.rng.chance(2, 3) {
        // Casts to the unknown/all types are always valid (no JSC_INVALID_CAST).
        let t = *g.rng.pick(&["?", "*"]);
        g.w(&format!("/** @type {{{t}}} */ ("));
    } else {
        g.w("/** @type {");
        type_expr(g, 1);
        g.w("} */ (");
    }
    g.expr();
    g.w(")");
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "typed_decl",
        weight: 6,
        leaf: false,
        when: always,
        emit: typed_decl,
    },
    Production {
        name: "const_decl",
        weight: 4,
        leaf: false,
        when: always,
        emit: const_decl,
    },
    Production {
        name: "enum",
        weight: 2,
        leaf: true,
        when: always,
        emit: enum_decl,
    },
    Production {
        name: "annotated_function",
        weight: 4,
        leaf: false,
        when: always,
        emit: annotated_function,
    },
    Production {
        name: "es5_constructor",
        weight: 2,
        leaf: false,
        when: always,
        emit: es5_constructor,
    },
    Production {
        name: "record",
        weight: 1,
        leaf: true,
        when: always,
        emit: record,
    },
    Production {
        name: "typedef",
        weight: 1,
        leaf: true,
        when: always,
        emit: typedef,
    },
    Production {
        name: "nocollapse_static",
        weight: 1,
        leaf: false,
        when: always,
        emit: nocollapse_static,
    },
];

pub static EXPRESSIONS: &[Production] = &[Production {
    name: "cast",
    weight: 2,
    leaf: false,
    when: always,
    emit: cast,
}];
