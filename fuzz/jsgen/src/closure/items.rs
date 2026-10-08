//! Typed, JSDoc-annotated top-level items (functions, prototype classes, ES classes with
//! interfaces, enums, constants, generic functions) and their typed uses.
//!
//! An item is declared in one of several [`Style`]s, so the same generator feeds the
//! single-file productions (`Style::Script`) and the multi-file programs in `program.rs`
//! (goog.module, goog.provide, ES modules, CommonJS). Every use of an item is an expression
//! of type `number`, so importers can combine uses freely and stay type-correct.
//!
//! Property and method names come from small shared pools on purpose: unrelated classes
//! then share property names, which is what disambiguateProperties, ambiguateProperties,
//! devirtualizeMethods and inlineSimpleMethods act on.

use super::typed::{self, Ty};
use crate::engine::Gen;
use std::cell::RefCell;

thread_local! {
    /// Items imported into the module being generated, with the prefix each is reached
    /// through (`m3.`, `gen.m3.`, or empty for a named import). Function and method bodies
    /// call into them, so values flow across modules and chunks (multi-file
    /// bodies were trivial and folded to constants).
    static IMPORTS: RefCell<Vec<(String, Item)>> = const { RefCell::new(Vec::new()) };
}

/// Set the imports visible to the bodies generated next (see [`IMPORTS`]).
pub fn set_imports(v: Vec<(String, Item)>) {
    IMPORTS.with(|c| *c.borrow_mut() = v);
}

thread_local! {
    /// goog.module / CommonJS: declarations stay local and `program.rs` writes one whole-object
    /// or whole-value export (`exports = {..}`, `module.exports = X`) at the end of the file
    /// (only per-name exports were generated).
    static COLLECT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// ES module: the next function or class declaration is written `export default ...`.
    static DEFAULT_SLOT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The name that took [`DEFAULT_SLOT`], if any.
    static DEFAULTED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Configure how the next declarations export themselves (see [`COLLECT`], [`DEFAULT_SLOT`]).
pub fn set_export_mode(collect: bool, default_slot: bool) {
    COLLECT.with(|c| c.set(collect));
    DEFAULT_SLOT.with(|c| c.set(default_slot));
    DEFAULTED.with(|c| *c.borrow_mut() = None);
}

/// Whether module declarations currently leave their export to the end of the file.
pub fn collecting() -> bool {
    COLLECT.with(|c| c.get())
}

/// The item name written as `export default function|class <name>` since the last
/// [`set_export_mode`], if any.
pub fn take_defaulted() -> Option<String> {
    DEFAULTED.with(|c| c.borrow_mut().take())
}

/// `item` reached under another name (a renamed import, a whole-value export, a re-export
/// alias). `Ctors` declares two names and cannot be renamed as one value.
pub fn renamed(item: &Item, new: &str) -> Option<Item> {
    let mut it = item.clone();
    match &mut it {
        Item::Func { name, .. }
        | Item::Generic { name }
        | Item::Const { name, .. }
        | Item::Class { name, .. }
        | Item::Enum { name, .. }
        | Item::State { name }
        | Item::Wide { name, .. } => *name = new.to_string(),
        Item::Ctors { .. } => return None,
    }
    Some(it)
}

/// The declaration keyword for a mutable-looking binding: `const`, or in ES modules sometimes
/// `let` / `var` (`export let`, `export var`).
fn var_kw(g: &mut Gen, style: &Style) -> &'static str {
    if matches!(style, Style::EsModule) && g.rng.chance(1, 4) {
        if g.rng.chance(1, 4) { "var" } else { "let" }
    } else {
        "const"
    }
}

fn imports() -> Vec<(String, Item)> {
    IMPORTS.with(|c| c.borrow().clone())
}

/// A `number` expression: an imported item's use (cross-module) or a typed value over `env`.
fn num_operand(g: &mut Gen, env: &[(String, Ty)]) {
    let imp = imports();
    let nums: Vec<String> = env
        .iter()
        .filter(|(_, t)| *t == Ty::Num)
        .map(|(n, _)| n.clone())
        .collect();
    if !nums.is_empty() && g.rng.chance(1, 3) {
        // A parameter or local: keeps the computation input-dependent (not foldable).
        let n = g.rng.pick(&nums).clone();
        g.w(&n);
    } else if !imp.is_empty() && g.rng.chance(1, 2) {
        let (p, it) = g.rng.pick(&imp).clone();
        use_number(g, &p, &it);
    } else {
        typed::value(g, Ty::Num, env);
    }
}

/// Typed statements that compute a number into a fresh `let` (returned, and pushed to
/// `env`): loops, branches, switch, try/catch, for-of, object state and cross-module calls,
/// all type-correct, so bodies stay `--jscomp_error=*`-clean and are not folded away.
pub fn typed_stmts(g: &mut Gen, env: &mut Vec<(String, Ty)>, init: Option<&str>) -> String {
    let acc = g.fresh("acc");
    g.w(&format!(" let {acc} = "));
    match init {
        Some(v) => g.w(v),
        None => num_operand(g, env),
    }
    g.w(";");
    let n = g.rng.range(1, 4);
    for _ in 0..n {
        g.w(" ");
        match g.rng.below(9) {
            0 => {
                let i = g.fresh("i");
                let k = g.rng.range(1, 5);
                g.w(&format!("for (let {i} = 0; {i} < {k}; {i}++) {{ {acc} += "));
                let mut e2 = env.clone();
                e2.push((i, Ty::Num));
                typed::value(g, Ty::Num, &e2);
                g.w("; }");
            }
            1 => {
                g.w(&format!("if ({acc} > "));
                typed::literal(g, Ty::Num);
                g.w(&format!(") {{ {acc} = {acc} * 2 + "));
                num_operand(g, env);
                g.w(&format!("; }} else {{ {acc} -= "));
                typed::literal(g, Ty::Num);
                g.w("; }");
            }
            2 => {
                g.w(&format!(
                    "switch ({acc} % 3) {{ case 0: {acc} += 1; break; case 1: {acc} *= 2; break; default: {acc} = "
                ));
                num_operand(g, env);
                g.w("; }");
            }
            3 => {
                let e = g.fresh("e");
                g.w(&format!("try {{ {acc} = {acc} + "));
                num_operand(g, env);
                g.w(&format!("; }} catch ({e}) {{ {acc} = 0; }}"));
            }
            4 => {
                let v = g.fresh("v");
                g.w(&format!("for (const {v} of ["));
                typed::literal(g, Ty::Num);
                g.w(", ");
                num_operand(g, env);
                g.w(&format!("]) {{ {acc} = {acc} - {v}; }}"));
            }
            5 => {
                let o = g.fresh("o");
                g.w(&format!("const {o} = {{n: {acc}, k: "));
                num_operand(g, env);
                g.w(&format!("}}; {o}.n += {o}.k; {acc} = {o}.n;"));
            }
            6 => {
                g.w(&format!("{acc} = {acc} > 0 ? {acc} : -{acc};"));
            }
            _ => {
                g.w(&format!("{acc} += "));
                num_operand(g, env);
                g.w(";");
            }
        }
    }
    env.push((acc.clone(), Ty::Num));
    acc
}

/// How a declaration is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Style {
    /// Plain global script declarations.
    Script,
    /// Inside a `goog.module`: local declaration, then `exports.X = X;`.
    GoogModule,
    /// ES module: `export function X`, `export const X`, `export class X`.
    EsModule,
    /// CommonJS: local declaration, then `module.exports.X = X;` or `exports.X = X;`.
    CommonJs,
    /// `goog.provide('ns')` file: `ns.X = ...`.
    Provide(String),
}

/// Shared method-name pool (see module docs).
pub const METHODS: &[&str] = &["get", "size", "run", "val", "calc"];
/// Shared field-name pool.
pub const FIELDS: &[&str] = &["x", "y", "count", "w"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// `function name(params...): ret`
    Func {
        name: String,
        params: Vec<Ty>,
        ret: Ty,
    },
    /// `@template T` identity-like function over numbers.
    Generic { name: String },
    /// A constant of a known type.
    Const { name: String, ty: Ty },
    /// A class (ES5 constructor or ES6 class) whose constructor takes one number and whose
    /// methods take no arguments and return numbers.
    Class { name: String, methods: Vec<String> },
    /// `@enum {number}` with `keys` keys `K0..`.
    Enum { name: String, keys: u32 },
    /// Mutable module state `{n: number, hits: number}`; importers update `n` (cross-module
    /// mutation).
    State { name: String },
    /// ES5 class with 7..=10 uniquely named prototype methods; every use calls all of them
    /// (`passes::wide_class`: extractPrototypeMemberDeclarations, crossChunkMethodMotion).
    Wide { name: String, methods: Vec<String> },
    /// ES class plus a subclass whose constructor only forwards to `super`
    /// (`passes::trivial_ctor_classes`: optimizeConstructors). `empty`: the base constructor
    /// is empty and takes no arguments.
    Ctors {
        base: String,
        sub: String,
        base_method: String,
        sub_method: String,
        empty: bool,
    },
}

impl Item {
    /// Every top-level name the item declares (and a module exports): `Ctors` declares two.
    pub fn names(&self) -> Vec<&str> {
        match self {
            Item::Ctors { base, sub, .. } => vec![base, sub],
            _ => vec![self.name()],
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Item::Func { name, .. }
            | Item::Generic { name }
            | Item::Const { name, .. }
            | Item::Class { name, .. }
            | Item::Enum { name, .. }
            | Item::State { name }
            | Item::Wide { name, .. } => name,
            Item::Ctors { sub, .. } => sub,
        }
    }
}

/// The left-hand side under which a declaration is written, and the trailing export line.
fn binding(g: &mut Gen, style: &Style, name: &str, kw: &str) -> (String, String) {
    match style {
        Style::Script | Style::GoogModule | Style::CommonJs => {
            let tail = match style {
                Style::GoogModule | Style::CommonJs if collecting() => String::new(),
                Style::GoogModule => format!("exports.{name} = {name};"),
                Style::CommonJs => {
                    if g.rng.chance(1, 2) {
                        format!("module.exports.{name} = {name};")
                    } else {
                        format!("exports.{name} = {name};")
                    }
                }
                _ => String::new(),
            };
            (format!("{kw} {name}"), tail)
        }
        Style::EsModule => {
            if matches!(kw, "function" | "class") && DEFAULT_SLOT.with(|c| c.replace(false)) {
                // `export default function X(..) {..}` / `export default class X {..}`: also
                // binds X locally.
                DEFAULTED.with(|c| *c.borrow_mut() = Some(name.to_string()));
                (format!("export default {kw} {name}"), String::new())
            } else if g.rng.chance(1, 4) {
                // Local declaration, exported through a specifier list.
                (format!("{kw} {name}"), format!("export {{{name}}};"))
            } else {
                (format!("export {kw} {name}"), String::new())
            }
        }
        Style::Provide(ns) => (format!("{ns}.{name}"), String::new()),
    }
}

fn finish(g: &mut Gen, tail: &str) {
    if !tail.is_empty() {
        g.nl();
        g.w(tail);
    }
}

fn param_doc(g: &mut Gen, params: &[(String, Ty)]) {
    for (p, t) in params {
        g.w(&format!("\n * @param {{{}}} {p}", t.jsdoc()));
    }
}

/// Declare a random item in `style`. Returns the item for later typed uses.
pub fn declare(g: &mut Gen, style: &Style) -> Item {
    match g.rng.below(15) {
        0..=3 => func(g, style),
        4..=6 => class(g, style),
        7 => generic(g, style),
        8 | 9 => constant(g, style),
        10 => state(g, style),
        11 => enumeration(g, style),
        12 | 13 => super::passes::wide_class(g, style),
        _ => super::passes::trivial_ctor_classes(g, style),
    }
}

/// `/** @const */ X = {n: <num>, hits: 0};` (mutable module state).
pub fn state(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("S");
    g.w("/** @const */");
    g.nl();
    let kw = var_kw(g, style);
    let (lhs, tail) = binding(g, style, &name, kw);
    g.w(&format!("{lhs} = {{n: "));
    typed::literal(g, Ty::Num);
    g.w(", hits: 0};");
    finish(g, &tail);
    Item::State { name }
}

/// Whether the module being generated is a goog.module / goog.provide file (Closure
/// primitives such as goog.inherits are then available from base.js).
/// Single-file programs also use `Style::Provide` for plain namespace objects, without
/// base.js, so the file kind decides (@export without goog.exportSymbol is
/// JSC_MISSING_EXPORT_SYMBOL_DEFINITION).
fn goog_style(style: &Style) -> bool {
    use super::program::{FileKind, current_kind};
    matches!(style, Style::GoogModule | Style::Provide(_))
        && matches!(current_kind(), FileKind::GoogModule | FileKind::GoogProvide)
}

pub fn func(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("fn");
    let np = g.rng.range(0, 3);
    let mut params = vec![];
    for _ in 0..np {
        let p = g.fresh("a");
        let t = if g.rng.chance(2, 3) {
            Ty::Num
        } else {
            typed::pick(g)
        };
        params.push((p, t));
    }
    let ret = typed::pick_scalar(g);
    g.w("/**");
    if g.rng.chance(1, 8) {
        g.w("\n * @suppress {checkTypes}");
    }
    if matches!(style, Style::Provide(_)) && goog_style(style) && g.rng.chance(1, 4) {
        g.w("\n * @export");
    }
    param_doc(g, &params);
    g.w(&format!("\n * @return {{{}}}\n */", ret.jsdoc()));
    g.nl();
    let (lhs, tail) = binding(g, style, &name, "function");
    let plist: Vec<&str> = params.iter().map(|(p, _)| p.as_str()).collect();
    let plist = plist.join(", ");
    if matches!(style, Style::Provide(_)) {
        g.w(&format!("{lhs} = function({plist}) {{"));
    } else {
        g.w(&format!("{lhs}({plist}) {{"));
    }
    // Only some parameters are used: unused parameters feed optimizeCalls/removeUnusedCode.
    let mut env: Vec<(String, Ty)> = params
        .iter()
        .filter(|_| g.rng.chance(3, 4))
        .cloned()
        .collect();
    if g.rng.chance(1, 3) {
        let t = g.fresh("t");
        g.w(&format!(" const {t} = "));
        typed::value(g, Ty::Num, &env);
        g.w(";");
        env.push((t, Ty::Num));
    }
    let acc = if g.rng.chance(3, 4) {
        Some(typed_stmts(g, &mut env, None))
    } else {
        None
    };
    if g.rng.chance(1, 4) {
        // Dead branch for removeUnreachableCode / peephole folding.
        g.w(" if (false) { return ");
        typed::literal(g, ret);
        g.w("; }");
    }
    g.w(" return ");
    match (&acc, ret) {
        (Some(a), Ty::Num) if g.rng.chance(2, 3) => g.w(a),
        _ => typed::value(g, ret, &env),
    }
    g.w(if matches!(style, Style::Provide(_)) {
        "; };"
    } else {
        "; }"
    });
    finish(g, &tail);
    Item::Func {
        name,
        params: params.into_iter().map(|(_, t)| t).collect(),
        ret,
    }
}

pub fn generic(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("id");
    g.w("/**\n * @template T\n * @param {T} v\n * @return {T}\n */");
    g.nl();
    let (lhs, tail) = binding(g, style, &name, "function");
    if matches!(style, Style::Provide(_)) {
        g.w(&format!("{lhs} = function(v) {{ return v; }};"));
    } else {
        g.w(&format!("{lhs}(v) {{ return v; }}"));
    }
    finish(g, &tail);
    Item::Generic { name }
}

pub fn constant(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("K");
    let ty = typed::pick(g);
    g.w(&format!("/** @const {{{}}} */", ty.jsdoc()));
    g.nl();
    let kw = var_kw(g, style);
    let (lhs, tail) = binding(g, style, &name, kw);
    g.w(&format!("{lhs} = "));
    typed::literal(g, ty);
    g.w(";");
    finish(g, &tail);
    Item::Const { name, ty }
}

pub fn enumeration(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("E");
    let keys = g.rng.range(1, 4);
    g.w("/** @enum {number} */");
    g.nl();
    let (lhs, tail) = binding(g, style, &name, "const");
    g.w(&format!("{lhs} = {{"));
    for i in 0..keys {
        if i > 0 {
            g.w(", ");
        }
        g.w(&format!("K{i}: {}", i * 3 + 1));
    }
    g.w("};");
    finish(g, &tail);
    Item::Enum { name, keys }
}

fn pick_methods(g: &mut Gen) -> Vec<String> {
    let n = g.rng.range(1, 3);
    let mut ms: Vec<String> = vec![];
    for _ in 0..n {
        let m = (*g.rng.pick(METHODS)).to_string();
        if !ms.contains(&m) {
            ms.push(m);
        }
    }
    ms
}

pub fn class(g: &mut Gen, style: &Style) -> Item {
    if g.rng.chance(1, 2) {
        es5_class(g, style)
    } else {
        es6_class(g, style)
    }
}

/// Prototype-based class: `@constructor` function plus `X.prototype.m = function` members.
fn es5_class(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("C");
    let methods = pick_methods(g);
    let f1 = *g.rng.pick(FIELDS);
    let sub = goog_style(style) && g.rng.chance(1, 3);
    let tag = if sub {
        *g.rng.pick(&["\n * @struct", "", "\n * @unrestricted"])
    } else {
        *g.rng
            .pick(&["\n * @struct", "", "\n * @final", "\n * @unrestricted"])
    };
    g.w(&format!(
        "/**\n * @constructor{tag}\n * @param {{number}} v\n */"
    ));
    g.nl();
    let (lhs, tail) = binding(g, style, &name, "function");
    let open = if matches!(style, Style::Provide(_)) {
        format!("{lhs} = function(v) {{")
    } else {
        format!("{lhs}(v) {{")
    };
    g.w(&open);
    g.w(&format!(
        "\n  /** @type {{number}} */\n  this.{f1} = v;\n  /** @private {{number}} */\n  this.p_ = v + 1;\n}}"
    ));
    if matches!(style, Style::Provide(_)) {
        g.w(";");
    }
    // In module styles the prototype is reached through the local name.
    let local = match style {
        Style::Provide(ns) => format!("{ns}.{name}"),
        _ => name.clone(),
    };
    for m in &methods {
        g.nl();
        g.w(&format!(
            "/** @return {{number}} */\n{local}.prototype.{m} = function() {{"
        ));
        if g.rng.chance(1, 2) {
            let mut env = vec![];
            let a = typed_stmts(g, &mut env, Some(&format!("this.{f1}")));
            g.w(&format!(" this.p_ = {a}; return {a};"));
        } else {
            g.w(" return ");
            match g.rng.below(3) {
                0 => g.w(&format!("this.{f1}")),
                1 => g.w(&format!("this.{f1} + this.p_")),
                _ => g.w(&format!("this.{f1} * 2")),
            }
            g.w(";");
        }
        g.w(" };");
    }
    if goog_style(style) && g.rng.chance(1, 4) {
        // Removed by closureCodeRemoval.
        let a = g.fresh("abs");
        g.nl();
        g.w(&format!(
            "/** @return {{number}} */\n{local}.prototype.{a} = goog.abstractMethod;"
        ));
    }
    finish(g, &tail);
    if !sub {
        return Item::Class { name, methods };
    }
    // A goog.inherits subclass (closurePrimitives, superClass_ calls); importers use it.
    let sname = g.fresh("Sub");
    g.nl();
    g.w("/**\n * @constructor\n * @extends {");
    g.w(&local);
    g.w("}\n * @param {number} v\n */");
    g.nl();
    let (slhs, stail) = binding(g, style, &sname, "function");
    let slocal = match style {
        Style::Provide(ns) => format!("{ns}.{sname}"),
        _ => sname.clone(),
    };
    // `Sub.base(this, 'constructor', v)` and `Sub.base(this, 'm')` are rewritten by
    // closurePrimitives (ProcessClosurePrimitives.maybeProcessClassBaseCall) into
    // `Parent.call(this, v)` and `Sub.superClass_.m.call(this)`.
    let use_base = g.rng.chance(1, 2);
    let super_ctor = if use_base {
        format!("{slocal}.base(this, 'constructor', v);")
    } else {
        format!("{local}.call(this, v);")
    };
    if matches!(style, Style::Provide(_)) {
        g.w(&format!("{slhs} = function(v) {{ {super_ctor} }};"));
    } else {
        g.w(&format!("{slhs}(v) {{ {super_ctor} }}"));
    }
    g.nl();
    g.w(&format!("goog.inherits({slocal}, {local});"));
    let m = g.rng.pick(&methods).clone();
    g.nl();
    let super_call = if use_base && g.rng.chance(2, 3) {
        format!("/** @type {{number}} */ ({slocal}.base(this, '{m}'))")
    } else {
        format!("{slocal}.superClass_.{m}.call(this)")
    };
    g.w(&format!(
        "/** @override */\n{slocal}.prototype.{m} = function() {{ return {super_call} + "
    ));
    typed::literal(g, Ty::Num);
    g.w("; };");
    finish(g, &stail);
    Item::Class {
        name: sname,
        methods,
    }
}

/// ES6 class, sometimes implementing a freshly declared `@interface`.
fn es6_class(g: &mut Gen, style: &Style) -> Item {
    let name = g.fresh("C");
    let methods = pick_methods(g);
    let f1 = *g.rng.pick(FIELDS);
    let iface = if g.rng.chance(1, 3) && !matches!(style, Style::Provide(_)) {
        let i = g.fresh("I");
        let kind = *g.rng.pick(&["interface", "record"]);
        g.w(&format!("/** @{kind} */\nclass {i} {{"));
        for m in &methods {
            g.w(&format!(" /** @return {{number}} */ {m}() {{}}"));
        }
        g.w(" }");
        g.nl();
        Some(i)
    } else {
        None
    };
    let (lhs, tail) = match style {
        Style::Provide(ns) => (format!("{ns}.{name} = class"), String::new()),
        _ => binding(g, style, &name, "class"),
    };
    if let Some(i) = &iface {
        g.w(&format!("/** @implements {{{i}}} */"));
        g.nl();
    }
    g.w(&format!(
        "{lhs} {{\n  /** @param {{number}} v */\n  constructor(v) {{\n    /** @const {{number}} */\n    this.{f1} = v;\n  }}"
    ));
    for m in &methods {
        let ann = if iface.is_some() {
            "/** @override */"
        } else {
            "/** @return {number} */"
        };
        if g.rng.chance(1, 2) {
            g.w(&format!("\n  {ann}\n  {m}() {{"));
            let mut env = vec![];
            let a = typed_stmts(g, &mut env, Some(&format!("this.{f1}")));
            g.w(&format!(" return {a}; }}"));
        } else {
            g.w(&format!("\n  {ann}\n  {m}() {{ return this.{f1} + "));
            typed::literal(g, Ty::Num);
            g.w("; }");
        }
    }
    if g.rng.chance(1, 3) {
        g.w("\n  /** @nocollapse @return {number} */\n  static make() { return 1; }");
    }
    g.w("\n}");
    if matches!(style, Style::Provide(_)) {
        g.w(";");
    }
    finish(g, &tail);
    Item::Class { name, methods }
}

/// A `number`-typed expression that uses `item`, reached through `prefix` (e.g. `m3.` for a
/// goog.module import, `ns.p1.` for a provide, empty for a local or named import).
pub fn use_number(g: &mut Gen, prefix: &str, item: &Item) {
    match item {
        Item::Func { name, params, ret } => {
            let mut call = format!("{prefix}{name}(");
            g.w("");
            let mut args = String::new();
            for (i, t) in params.iter().enumerate() {
                if i > 0 {
                    args.push_str(", ");
                }
                let save = std::mem::take(&mut g.out);
                typed::literal(g, *t);
                args.push_str(&std::mem::replace(&mut g.out, save));
            }
            call.push_str(&args);
            call.push(')');
            typed::use_as_number(g, &call, *ret);
        }
        Item::Generic { name } => {
            g.w(&format!("{prefix}{name}("));
            typed::literal(g, Ty::Num);
            g.w(")");
        }
        Item::Const { name, ty } => typed::use_as_number(g, &format!("{prefix}{name}"), *ty),
        Item::Class { name, methods } => {
            let m = g.rng.pick(methods).clone();
            g.w(&format!("new {prefix}{name}("));
            typed::literal(g, Ty::Num);
            g.w(&format!(").{m}()"));
        }
        Item::Enum { name, keys } => {
            let k = g.rng.below(u64::from(*keys));
            g.w(&format!("{prefix}{name}.K{k}"));
        }
        Item::Wide { .. } | Item::Ctors { .. } => super::passes::use_number(g, prefix, item),
        Item::State { name } => match g.rng.below(3) {
            0 => g.w(&format!("{prefix}{name}.n")),
            1 => g.w(&format!("++{prefix}{name}.hits")),
            _ => {
                g.w(&format!("({prefix}{name}.n += "));
                typed::literal(g, Ty::Num);
                g.w(")");
            }
        },
    }
}

/// `console.log(use, use, ...);` over a random subset of `items` (at least one).
pub fn observe(g: &mut Gen, prefix_of: &dyn Fn(&Item) -> String, items: &[Item]) {
    if items.is_empty() {
        return;
    }
    let n = g.rng.range(1, items.len().min(3) as u32);
    g.w("console.log(");
    for i in 0..n {
        if i > 0 {
            g.w(", ");
        }
        let it = g.rng.pick(items).clone();
        let p = prefix_of(&it);
        use_number(g, &p, &it);
    }
    g.w(");");
}
