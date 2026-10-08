//! Inputs for the D2-effective passes that no fuzz run reached before (D-016 item 5):
//!
//! - `extractPrototypeMemberDeclarations` needs a run of consecutive `X.prototype.m = ...`
//!   statements that survives to the end of the optimization loop. Under USE_GLOBAL_TEMP an
//!   extraction pays off only from about seven members of one class
//!   (ExtractPrototypeMemberDeclarations.Pattern: 14 + 3·n per instance, 6 global), so
//!   [`wide_class`] declares 7..=10 methods and every use calls all of them.
//! - `crossChunkMethodMotion` moves a prototype method whose property name is referenced only
//!   in a deeper chunk. [`wide_class`] uses fresh method names, and `program.rs` never uses a
//!   wide class in the file that declares it, so importers in c1 are its only users.
//! - Both need the methods to stay prototype methods: DevirtualizeMethods skips functions that
//!   reference `arguments` (isEligibleDefinitionFunction), and InlineSimpleMethods skips
//!   bodies that are not a single `return this.x`, so the method bodies read
//!   `arguments.length`.
//! - `deadPropertyAssignmentElimination` removes a property write that is overwritten in the
//!   same function before any read, call, nested block or nested function
//!   (DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal). Constructors are
//!   never inlined, so the pattern is placed in constructor bodies.
//! - `optimizeConstructors` drops an ES class constructor that is empty or only forwards its
//!   parameters to an equivalent super constructor (OptimizeConstructors).
//! - `@nosideeffects` (accepted outside externs by this compiler version: CheckJSDoc defines
//!   JSC_INVALID_NO_SIDE_EFFECT_ANNOTATION but never reports it) and `@dict`.
//!
//! Property names are fresh (`w12a`, `dp7`), never names that the default externs declare:
//! DeadPropertyAssignmentElimination skips extern property names.

use super::items::{self, Item, Style};
use super::typed::{self, Ty};
use crate::engine::{Binding, Gen, Production, always};

/// Method-body suffix that keeps a method out of DevirtualizeMethods and InlineSimpleMethods.
const ARGS: &str = "arguments.length";

/// Declaration prefix and local reference for `name` in `style` (see `items::binding`).
fn lhs_and_local(g: &mut Gen, style: &Style, name: &str, kw: &str) -> (String, String, String) {
    match style {
        Style::Provide(ns) => (
            format!("{ns}.{name}"),
            String::new(),
            format!("{ns}.{name}"),
        ),
        Style::GoogModule | Style::CommonJs if items::collecting() => {
            (format!("{kw} {name}"), String::new(), name.into())
        }
        Style::GoogModule => (
            format!("{kw} {name}"),
            format!("exports.{name} = {name};"),
            name.into(),
        ),
        Style::CommonJs => {
            let tail = if g.rng.chance(1, 2) {
                format!("module.exports.{name} = {name};")
            } else {
                format!("exports.{name} = {name};")
            };
            (format!("{kw} {name}"), tail, name.into())
        }
        Style::EsModule => {
            if g.rng.chance(1, 4) {
                (
                    format!("{kw} {name}"),
                    format!("export {{{name}}};"),
                    name.into(),
                )
            } else {
                (format!("export {kw} {name}"), String::new(), name.into())
            }
        }
        Style::Script => (format!("{kw} {name}"), String::new(), name.into()),
    }
}

/// An ES5 `@constructor` with 7..=10 prototype methods under fresh names. The constructor
/// writes one field twice in a row (a dead property write). Returns `Item::Wide`.
pub fn wide_class(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("W");
    let f = format!("{}fld", name.to_lowercase());
    let n = g.rng.range(7, 10);
    let methods: Vec<String> = (0..n)
        .map(|k| format!("{}{}", name.to_lowercase(), char::from(b'a' + k as u8)))
        .collect();
    let tag = *g.rng.pick(&["", "\n * @struct", "\n * @final"]);
    g.w(&format!(
        "/**\n * @constructor{tag}\n * @param {{number}} v\n */"
    ));
    g.nl();
    let (lhs, tail, local) = lhs_and_local(g, style, &name, "function");
    let open = if matches!(style, Style::Provide(_)) {
        format!("{lhs} = function(v) {{")
    } else {
        format!("{lhs}(v) {{")
    };
    g.w(&open);
    // Dead write, then the live one (no read, call or block in between).
    g.w(&format!("\n  /** @type {{number}} */\n  this.{f} = v + "));
    typed::literal(g, Ty::Num);
    g.w(&format!(";\n  this.{f} = v * 2;\n}}"));
    if matches!(style, Style::Provide(_)) {
        g.w(";");
    }
    for m in &methods {
        g.nl();
        g.w(&format!(
            "/** @return {{number}} */\n{local}.prototype.{m} = function() {{ return "
        ));
        match g.rng.below(3) {
            0 => g.w(&format!("this.{f} + {ARGS}")),
            1 => {
                g.w(&format!("this.{f} * "));
                typed::literal(g, Ty::Num);
                g.w(&format!(" - {ARGS}"));
            }
            _ => g.w(&format!("({ARGS} ? 0 : this.{f}) + 1")),
        }
        g.w("; };");
    }
    if !tail.is_empty() {
        g.nl();
        g.w(&tail);
    }
    Item::Wide { name, methods }
}

/// An ES class whose subclass constructor only forwards to `super`, and sometimes a class
/// with an empty constructor (both removable by OptimizeConstructors). Returns the subclass
/// as an `Item::Class` (its uses call base and subclass methods).
pub fn trivial_ctor_classes(g: &mut Gen, style: &Style) -> Item {
    let base = g.fresh("Tb");
    let sub = g.fresh("Td");
    let fb = format!("{}v", base.to_lowercase());
    let mb = format!("{}m", base.to_lowercase());
    let md = format!("{}m", sub.to_lowercase());
    let (blhs, btail, blocal) = match style {
        Style::Provide(ns) => (
            format!("{ns}.{base} = class"),
            String::new(),
            format!("{ns}.{base}"),
        ),
        _ => lhs_and_local(g, style, &base, "class"),
    };
    let empty = g.rng.chance(1, 3);
    if empty {
        // Empty constructor, no parameters, no extends clause.
        g.w(&format!(
            "{blhs} {{\n  constructor() {{}}\n  /** @return {{number}} */\n  {mb}() {{ return {ARGS} + "
        ));
        typed::literal(g, Ty::Num);
        g.w("; }\n}");
    } else {
        g.w(&format!(
            "{blhs} {{\n  /** @param {{number}} v */\n  constructor(v) {{\n    /** @const {{number}} */\n    this.{fb} = v;\n  }}\n  /** @return {{number}} */\n  {mb}() {{ return this.{fb} + {ARGS}; }}\n}}"
        ));
    }
    if matches!(style, Style::Provide(_)) {
        g.w(";");
    }
    if !btail.is_empty() {
        g.nl();
        g.w(&btail);
    }
    g.nl();
    let (slhs, stail, _) = match style {
        Style::Provide(ns) => (format!("{ns}.{sub} = class"), String::new(), String::new()),
        _ => lhs_and_local(g, style, &sub, "class"),
    };
    let (params, call) = if empty {
        ("", "")
    } else if g.rng.chance(1, 4) {
        ("...args", "...args")
    } else {
        ("v", "v")
    };
    let pdoc = match params {
        "v" => "/** @param {number} v */\n  ",
        "...args" => "/** @param {...number} args */\n  ",
        _ => "",
    };
    let read = if empty {
        format!("{ARGS} + 2")
    } else {
        format!("this.{fb} * 2 + {ARGS}")
    };
    g.w(&format!(
        "{slhs} extends {blocal} {{\n  {pdoc}constructor({params}) {{ super({call}); }}\n  /** @return {{number}} */\n  {md}() {{ return {read}; }}\n}}"
    ));
    if matches!(style, Style::Provide(_)) {
        g.w(";");
    }
    if !stail.is_empty() {
        g.nl();
        g.w(&stail);
    }
    Item::Ctors {
        base,
        sub,
        base_method: mb,
        sub_method: md,
        empty,
    }
}

/// `new X(v).a() + new X(v).b() + ...` over every method of a wide class, or base and
/// subclass method calls of [`trivial_ctor_classes`].
pub fn use_number(g: &mut Gen, prefix: &str, item: &Item) {
    match item {
        Item::Wide { name, methods } => {
            g.w("(");
            for (k, m) in methods.iter().enumerate() {
                if k > 0 {
                    g.w(" + ");
                }
                g.w(&format!("new {prefix}{name}("));
                typed::literal(g, Ty::Num);
                g.w(&format!(").{m}()"));
            }
            g.w(")");
        }
        Item::Ctors {
            base,
            sub,
            base_method,
            sub_method,
            empty,
        } => {
            let arg = |g: &mut Gen| {
                if *empty {
                    String::new()
                } else {
                    let save = std::mem::take(&mut g.out);
                    typed::literal(g, Ty::Num);
                    std::mem::replace(&mut g.out, save)
                }
            };
            let (a1, a2, a3) = (arg(g), arg(g), arg(g));
            g.w(&format!(
                "(new {prefix}{sub}({a1}).{sub_method}() + new {prefix}{sub}({a2}).{base_method}() + new {prefix}{base}({a3}).{base_method}())"
            ));
        }
        _ => items::use_number(g, prefix, item),
    }
}

// ---- single-file productions ---------------------------------------------------------

fn bind(g: &mut Gen, it: &Item) {
    match it {
        Item::Wide { name, .. } => g.declare(name, Binding::Function),
        Item::Ctors { base, sub, .. } => {
            g.declare(base, Binding::Class);
            g.declare(sub, Binding::Class);
        }
        _ => {}
    }
}

fn wide_class_stmt(g: &mut Gen) {
    let it = wide_class(g, &Style::Script);
    g.nl();
    g.w("console.log(");
    use_number(g, "", &it);
    g.w(");");
    bind(g, &it);
}

fn trivial_ctor_stmt(g: &mut Gen) {
    let it = trivial_ctor_classes(g, &Style::Script);
    g.nl();
    g.w("console.log(");
    use_number(g, "", &it);
    g.w(");");
    bind(g, &it);
}

/// A constructor that overwrites a field before reading it, and a method that does the same
/// on an object parameter (deadPropertyAssignmentElimination).
fn dead_property_stmt(g: &mut Gen) {
    let c = g.fresh("Dp");
    let f = format!("{}a", c.to_lowercase());
    let h = format!("{}b", c.to_lowercase());
    let m = format!("{}m", c.to_lowercase());
    g.w(&format!(
        "/**\n * @constructor\n * @param {{number}} v\n */\nfunction {c}(v) {{\n  /** @type {{number}} */\n  this.{f} = "
    ));
    typed::literal(g, Ty::Num);
    g.w(&format!(
        ";\n  /** @type {{number}} */\n  this.{h} = v;\n  this.{f} = v + "
    ));
    typed::literal(g, Ty::Num);
    g.w(&format!(";\n  this.{h} = v * 3;\n}}"));
    g.nl();
    g.w(&format!(
        "/** @param {{!{c}}} o @return {{number}} */\n{c}.prototype.{m} = function(o) {{ o.{f} = 1; o.{f} = this.{h} + {ARGS}; return o.{f}; }};"
    ));
    g.nl();
    let o = g.fresh("dpo");
    g.w(&format!("const {o} = new {c}("));
    typed::literal(g, Ty::Num);
    g.w(");");
    g.nl();
    g.w(&format!("console.log({o}.{m}({o}) + {o}.{f} + {o}.{h});"));
    g.declare(&c, Binding::Function);
    g.declare(&o, Binding::Const);
}

/// `@nosideeffects` functions; one call's result is unused (removable as pure).
fn nosideeffects_stmt(g: &mut Gen) {
    let f = g.fresh("pure");
    let p = g.fresh("a");
    g.w(&format!(
        "/**\n * @nosideeffects\n * @param {{number}} {p}\n * @return {{number}}\n */\nfunction {f}({p}) {{ return {p} * "
    ));
    typed::literal(g, Ty::Num);
    g.w(" + 1; }");
    g.nl();
    g.w(&format!("{f}("));
    typed::literal(g, Ty::Num);
    g.w(");");
    g.nl();
    g.w(&format!("console.log({f}("));
    typed::literal(g, Ty::Num);
    g.w("));");
    g.declare(&f, Binding::Function);
}

/// `@dict` object literal and `@dict` constructor, accessed only with brackets.
fn dict_stmt(g: &mut Gen) {
    let d = g.fresh("dict");
    let k1 = format!("{d}a");
    let k2 = format!("{d}b");
    g.w(&format!("/** @dict */\nconst {d} = {{'{k1}': "));
    typed::literal(g, Ty::Num);
    g.w(&format!(", '{k2}': 2}};"));
    g.nl();
    g.w(&format!("{d}['{k1}'] = {d}['{k2}'] + 1;"));
    g.nl();
    if g.rng.chance(1, 2) {
        let c = g.fresh("Dict");
        g.w(&format!(
            "/**\n * @constructor\n * @dict\n */\nfunction {c}() {{ this['{k1}'] = 3; }}"
        ));
        g.nl();
        g.w(&format!("console.log({d}['{k1}'], new {c}()['{k1}']);"));
        g.declare(&c, Binding::Function);
    } else {
        g.w(&format!("console.log({d}['{k1}']);"));
    }
    g.declare(&d, Binding::Const);
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "wide_class",
        weight: 2,
        leaf: true,
        when: always,
        emit: wide_class_stmt,
    },
    Production {
        name: "trivial_ctor",
        weight: 2,
        leaf: true,
        when: always,
        emit: trivial_ctor_stmt,
    },
    Production {
        name: "dead_property",
        weight: 2,
        leaf: true,
        when: always,
        emit: dead_property_stmt,
    },
    Production {
        name: "nosideeffects",
        weight: 1,
        leaf: true,
        when: always,
        emit: nosideeffects_stmt,
    },
    Production {
        name: "dict",
        weight: 1,
        leaf: true,
        when: always,
        emit: dict_stmt,
    },
];
