//! Single-file Closure statement productions: typed items with uses, namespaces for
//! property collapsing, `@define`, `window[...]` exports, `@typedef`/`@record` values,
//! dead code and unused parameters.

use super::items::{self, Item, Style};
use super::program::{FileKind, current_kind};
use super::typed::{self, Ty};
use crate::engine::{Binding, Gen, Production, always};

/// True when the production is being emitted as a top-level statement of a script file.
fn top_level_script(g: &Gen) -> bool {
    g.depth() == 1 && current_kind() == FileKind::Script
}

fn bind_item(g: &mut Gen, it: &Item) {
    let b = match it {
        Item::Func { .. } | Item::Generic { .. } | Item::Wide { .. } => Binding::Function,
        Item::Class { .. } | Item::Ctors { .. } => Binding::Class,
        Item::Const { .. } | Item::Enum { .. } | Item::State { .. } => Binding::Const,
    };
    g.declare(it.name(), b);
}

/// A typed item, then a typed use of it (keeps ADVANCED output non-trivial).
fn typed_item(g: &mut Gen) {
    let it = items::declare(g, &Style::Script);
    g.nl();
    items::observe(g, &|_| String::new(), std::slice::from_ref(&it));
    bind_item(g, &it);
}

/// Namespace object with nested members (collapseProperties, inlineProperties).
fn namespace(g: &mut Gen) {
    let ns = g.fresh("ns");
    g.w(&format!("/** @const */\nvar {ns} = {{}};"));
    g.nl();
    g.w(&format!("/** @const */\n{ns}.sub = {{}};"));
    let mut members: Vec<Item> = vec![];
    let mut prefixes: Vec<(String, String)> = vec![];
    let k = g.rng.range(1, 3);
    for _ in 0..k {
        g.nl();
        let pfx = if g.rng.chance(1, 2) {
            format!("{ns}.sub")
        } else {
            ns.clone()
        };
        let it = items::declare(g, &Style::Provide(pfx.clone()));
        prefixes.push((it.name().to_string(), format!("{pfx}.")));
        members.push(it);
    }
    if g.rng.chance(1, 2) {
        g.nl();
        g.w(&format!("/** @type {{number}} */\n{ns}.counter = 0;"));
        g.nl();
        g.w(&format!("{ns}.counter = {ns}.counter + 1;"));
    }
    g.nl();
    let lookup = move |it: &Item| {
        prefixes
            .iter()
            .find(|(n, _)| n == it.name())
            .map(|(_, p)| p.clone())
            .unwrap_or_default()
    };
    items::observe(g, &lookup, &members);
    g.declare(&ns, Binding::Var);
}

/// `@define` (global only), then a branch on it (processDefines, dead-code folding).
fn define(g: &mut Gen) {
    if !top_level_script(g) {
        return typed_item(g);
    }
    let d = g.fresh("DEF");
    let (ty, v) = *g.rng.pick(&[
        ("boolean", "false"),
        ("boolean", "true"),
        ("number", "2"),
        ("string", "'en'"),
    ]);
    let kw = *g.rng.pick(&["const", "var"]);
    g.w(&format!("/** @define {{{ty}}} */\n{kw} {d} = {v};"));
    g.nl();
    let cond = match ty {
        "boolean" => d.clone(),
        "number" => format!("{d} > 1"),
        _ => format!("{d} === 'de'"),
    };
    g.w(&format!(
        "if ({cond}) {{ console.log('on'); }} else {{ console.log('off'); }}"
    ));
    g.declare(&d, Binding::Const);
}

/// Exported item: `window['name'] = item;` (ADVANCED keeps it alive).
fn window_export(g: &mut Gen) {
    let it = items::declare(g, &Style::Script);
    g.nl();
    g.w(&format!("window['{0}'] = {0};", it.name()));
    bind_item(g, &it);
}

/// `@record` type plus a typed value and use (structural typing).
fn record_value(g: &mut Gen) {
    let r = g.fresh("Rec");
    let f = *g.rng.pick(items::FIELDS);
    g.w(&format!(
        "/** @record */\nfunction {r}() {{}}\n/** @type {{number}} */\n{r}.prototype.{f};"
    ));
    let v = g.fresh("rv");
    g.nl();
    g.w(&format!("/** @type {{!{r}}} */\nconst {v} = {{{f}: "));
    typed::literal(g, Ty::Num);
    g.w("};");
    g.nl();
    g.w(&format!("console.log({v}.{f});"));
    g.declare(&v, Binding::Const);
}

/// `@typedef` record type, used by an annotated function.
fn typedef_use(g: &mut Gen) {
    let t = g.fresh("T");
    let f = g.fresh("tf");
    g.w(&format!(
        "/** @typedef {{{{a: number, b: string}}}} */\nlet {t};\n/**\n * @param {{{t}}} o\n * @return {{number}}\n */\nfunction {f}(o) {{ return o.a + o.b.length; }}"
    ));
    g.nl();
    g.w(&format!("console.log({f}({{a: "));
    typed::literal(g, Ty::Num);
    g.w(", b: ");
    typed::literal(g, Ty::Str);
    g.w("}));");
    g.declare(&f, Binding::Function);
}

/// Unused function, unused parameters, and a constant-false branch.
fn dead_code(g: &mut Gen) {
    let f = g.fresh("unused");
    g.w(&format!(
        "/** @param {{number}} a @param {{number}} b @return {{number}} */\nfunction {f}(a, b) {{ return a; }}"
    ));
    g.nl();
    match g.rng.below(3) {
        0 => {
            g.w(&format!("console.log({f}(1, 2));"));
        }
        1 => {
            g.w("if (false) { console.log('dead'); }");
        }
        _ => {
            g.w(&format!("const {f}v = {f}(3, 4) * 0;"));
        }
    }
}

/// Subclass with `@extends`, `@override` and `super` calls (ES6 class over an item class).
fn subclass(g: &mut Gen) {
    let it = items::class(g, &Style::Script);
    let Item::Class { name, methods } = &it else {
        return;
    };
    let sub = g.fresh("D");
    let m = g.rng.pick(methods).clone();
    g.nl();
    g.w(&format!(
        "/** @extends {{{name}}} */\nclass {sub} extends {name} {{\n  /** @param {{number}} v */\n  constructor(v) {{ super(v); /** @protected {{number}} */ this.extra = v; }}\n  /** @override */\n  {m}() {{ return this.extra; }}\n}}"
    ));
    g.nl();
    g.w(&format!(
        "/** @type {{!{name}}} */\nconst {sub}o = Math.random() > 0.5 ? new {sub}(2) : new {name}(1);"
    ));
    g.nl();
    g.w(&format!("console.log({sub}o.{m}());"));
    g.declare(name, Binding::Class);
    g.declare(&sub, Binding::Class);
}

pub static STATEMENTS: &[Production] = &[
    Production {
        name: "typed_item",
        weight: 8,
        leaf: true,
        when: always,
        emit: typed_item,
    },
    Production {
        name: "namespace",
        weight: 3,
        leaf: true,
        when: always,
        emit: namespace,
    },
    Production {
        name: "define",
        weight: 2,
        leaf: true,
        when: always,
        emit: define,
    },
    Production {
        name: "window_export",
        weight: 2,
        leaf: true,
        when: always,
        emit: window_export,
    },
    Production {
        name: "record_value",
        weight: 1,
        leaf: true,
        when: always,
        emit: record_value,
    },
    Production {
        name: "typedef_use",
        weight: 1,
        leaf: true,
        when: always,
        emit: typedef_use,
    },
    Production {
        name: "dead_code",
        weight: 2,
        leaf: true,
        when: always,
        emit: dead_code,
    },
    Production {
        name: "subclass",
        weight: 2,
        leaf: true,
        when: always,
        emit: subclass,
    },
];
