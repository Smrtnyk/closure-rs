//! Classes: declarations and expressions, `extends` (classes, builtins, expressions),
//! constructors with `super(...)`, methods (plain, static, generator, async, async
//! generator, computed / string / numeric names), getters and setters, public and static
//! fields (incl. computed), static blocks, `super.m()`, and, in the unsupported dialect,
//! private fields, methods, accessors and `#x in o`.

use super::{dialect, util};
use crate::engine::{Binding, Ctx, Gen, Production, always};

fn method_ctx(c: &mut Ctx, generator: bool, is_async: bool) {
    util::enter_fn(c, generator, is_async);
    c.in_class_method = true;
}

/// Class field initialisers and static blocks: no `yield`/`await`/`return`/`arguments`.
fn init_ctx(c: &mut Ctx) {
    c.in_function = false;
    c.in_generator = false;
    c.in_async = false;
    c.in_loop = false;
    c.in_switch = false;
    c.labels.clear();
    c.in_class_method = true;
}

/// A method name. The fixed string and numeric keys are used at most once per class body
/// (`used`): Closure rejects a repeated member name (JSC_DUPLICATE_MEMBER), which would stop
/// every non-WHITESPACE compile before optimization.
fn method_name(g: &mut Gen, used: &mut Vec<String>) -> String {
    let k = match g.rng.below(10) {
        0 => "'str key'".to_string(),
        1 => "42".to_string(),
        2 => "[Symbol.iterator]".to_string(),
        3 => "['comp' + 'uted']".to_string(),
        _ => return g.fresh("m"),
    };
    if k.starts_with('[') {
        return k;
    }
    if used.contains(&k) {
        return g.fresh("m");
    }
    used.push(k.clone());
    k
}

fn class_body(g: &mut Gen, derived: bool) {
    g.w(" {");
    let n = g.rng.range(0, 5);
    let mut have_ctor = false;
    let mut privates: Vec<String> = vec![];
    let mut used: Vec<String> = vec![];
    for _ in 0..n {
        g.nl();
        g.w("  ");
        let k = if dialect::unsupported() {
            g.rng.below(14)
        } else {
            g.rng.below(11)
        };
        match k {
            0 if !have_ctor => {
                have_ctor = true;
                g.w("constructor");
                g.push_scope();
                super::func::params(g, 2);
                g.w(" ");
                let prefix = if derived {
                    "super(...arguments); this.x = 1;"
                } else {
                    "this.x = 1;"
                };
                g.with_ctx(
                    |c| method_ctx(c, false, false),
                    |g| util::body(g, 2, prefix, ""),
                );
                g.pop_scope();
            }
            1 => {
                let m = g.fresh("p");
                let st = if g.rng.chance(1, 3) { "static " } else { "" };
                g.w(&format!("{st}get {m}() {{ return "));
                g.with_ctx(|c| method_ctx(c, false, false), |g| g.expr());
                let v = g.fresh("v");
                g.w(&format!("; }} {st}set {m}({v}) {{ this._{m} = {v}; }}"));
            }
            2 => {
                let m = method_name(g, &mut used);
                g.w(&format!("static {m}() "));
                g.push_scope();
                g.with_ctx(|c| method_ctx(c, false, false), |g| g.block(2));
                g.pop_scope();
            }
            3 | 4 => {
                // Public field (instance or static), optionally computed / uninitialised.
                let st = if g.rng.chance(1, 3) { "static " } else { "" };
                let m = if g.rng.chance(1, 6) {
                    "['f' + 1]".to_string()
                } else {
                    g.fresh("fld")
                };
                g.w(&format!("{st}{m}"));
                if g.rng.chance(4, 5) {
                    g.w(" = ");
                    g.with_ctx(init_ctx, |g| g.expr());
                }
                g.w(";");
            }
            5 => {
                // Static initialisation block (ES2022).
                g.w("static ");
                g.with_ctx(init_ctx, |g| util::body(g, 3, "", ""));
            }
            6 => {
                let (kw, gen_, asy) = *g.rng.pick(&[
                    ("*", true, false),
                    ("async ", false, true),
                    ("async *", true, true),
                    ("static *", true, false),
                    ("static async ", false, true),
                ]);
                let m = method_name(g, &mut used);
                g.w(&format!("{kw}{m}() "));
                g.push_scope();
                g.with_ctx(|c| method_ctx(c, gen_, asy), |g| g.block(2));
                g.pop_scope();
            }
            7 if derived => {
                let m = g.fresh("m");
                g.w(&format!("{m}() {{ return super.toString() + "));
                g.with_ctx(|c| method_ctx(c, false, false), |g| g.pexpr());
                g.w("; }");
            }
            11 => {
                let p = g.fresh("#q");
                let st = if g.rng.chance(1, 4) { "static " } else { "" };
                g.w(&format!("{st}{p} = "));
                g.with_ctx(init_ctx, |g| g.expr());
                g.w(";");
                if st.is_empty() {
                    privates.push(p);
                }
            }
            12 => {
                let p = g.fresh("#pm");
                g.w(&format!("{p}() {{ return this; }}"));
                let a = g.fresh("#acc");
                g.w(&format!(" get {a}() {{ return 1; }} set {a}(v) {{}}"));
            }
            13 if !privates.is_empty() => {
                let p = g.rng.pick(&privates).clone();
                let m = g.fresh("m");
                let o = g.fresh("o");
                g.w(&format!(
                    "{m}({o}) {{ return {p} in {o} ? this.{p} + {o}.{p} : this?.{p}; }}"
                ));
            }
            _ => {
                let m = method_name(g, &mut used);
                g.w(&m);
                g.push_scope();
                super::func::params(g, 2);
                g.w(" ");
                g.with_ctx(|c| method_ctx(c, false, false), |g| g.block(3));
                g.pop_scope();
            }
        }
    }
    g.nl();
    g.w("}");
}

fn heritage(g: &mut Gen) -> bool {
    if !g.rng.chance(1, 3) {
        return false;
    }
    g.w(" extends ");
    match g.visible(|b| b == Binding::Class) {
        Some(b) if g.rng.chance(2, 3) => g.w(&b),
        _ => {
            let b = *g
                .rng
                .pick(&["Object", "Error", "Array", "Map", "(class {})"]);
            g.w(b);
        }
    }
    true
}

fn class_decl(g: &mut Gen) {
    let name = g.fresh("C");
    g.w("class ");
    g.w(&name);
    let derived = heritage(g);
    g.with_ctx(util::no_yield_await, |g| class_body(g, derived));
    g.declare(&name, Binding::Class);
    if g.rng.chance(1, 2) {
        // Use the class: instantiate and call into it.
        g.nl();
        let i = g.fresh("inst");
        g.w(&format!("const {i} = new {name}();"));
        g.declare(&i, Binding::Const);
    }
}

fn class_expr(g: &mut Gen) {
    g.w("(class");
    if g.rng.chance(1, 3) {
        let n = g.fresh("CE");
        g.w(" ");
        g.w(&n);
    }
    let derived = heritage(g);
    g.with_ctx(util::no_yield_await, |g| class_body(g, derived));
    g.w(")");
}

pub static STATEMENTS: &[Production] = &[Production {
    name: "class_decl",
    weight: 4,
    leaf: false,
    when: always,
    emit: class_decl,
}];

pub static EXPRESSIONS: &[Production] = &[Production {
    name: "class_expr",
    weight: 1,
    leaf: false,
    when: always,
    emit: class_expr,
}];
