//! Multi-file Closure programs: goog.module, goog.provide, ES modules and CommonJS, laid out
//! so that every dependency is legal under all D2 output configurations at once:
//! one output file, `chunks2` (c0 = first ceil(n/2) `--js` files, c1 depends on c0) and
//! `chunks3` (c0 = first ceil(n/3), c1 = next ceil((n-k0)/2), c2 = the rest; c1 and c2 each
//! depend only on c0). See `corpus/d2/profiles.json` and `gates/lib/case_args.py`.
//!
//! Dependency rule: file `i` may import file `j` only if `j < i` and, under `chunks3`,
//! `j` is in c0 or in the same chunk as `i`. Under `chunks2` `j < i` already suffices.
//! Imports across chunk boundaries (c1 or c2 importing c0) are therefore common, which is
//! what crossChunkCodeMotion and the chunk-aware passes need.
//!
//! The driver must write every file under its `name` into one directory, pass them as
//! `--js` in the given order, and add `extra_flags` (CommonJS needs
//! `--process_common_js_modules`). goog.* programs start with a minimal `base.js`
//! (`@provideGoog`), because ADVANCED rejects an undeclared `goog`.

use super::items::{self, Item, Style};
use crate::engine::Gen;
use crate::rng::Rng;
use crate::{Config, rng};
use std::cell::Cell;

/// Which module system a generated file uses. Productions consult [`current_kind`] to stay
/// legal (for example, `@define` is emitted only at the top level of a script).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Script,
    GoogModule,
    GoogProvide,
    EsModule,
    CommonJs,
}

thread_local! {
    static CURRENT: Cell<FileKind> = const { Cell::new(FileKind::Script) };
}

/// The kind of file currently being generated (`Script` for single-file programs).
pub fn current_kind() -> FileKind {
    CURRENT.with(|c| c.get())
}

fn set_kind(k: FileKind) {
    CURRENT.with(|c| c.set(k));
}

/// One generated multi-file program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureProgram {
    /// The module system of the program's files (`base.js` is a script).
    pub kind: FileKind,
    /// `(file name, contents)` in `--js` order. Names are bare (`m0.js`); ES module and
    /// CommonJS imports use `./<name>`.
    pub files: Vec<(String, String)>,
    /// Extra compiler flags the program needs (case `extra_flags`).
    pub extra_flags: Vec<String>,
    /// `deps[i]` = indices of the files that file `i` imports.
    pub deps: Vec<Vec<usize>>,
}

/// Minimal Closure base: declares the primitives the compiler rewrites.
pub const BASE_JS: &str = "/**
 * @fileoverview Minimal Closure base for generated programs.
 * @provideGoog
 */
/** @const */
var goog = goog || {};
/** @const */
goog.global = this || self;
/** @define {boolean} */
goog.DEBUG = true;
/** @param {string} name */
goog.provide = function(name) {};
/** @param {string} name @return {?} */
goog.require = function(name) {};
/** @param {string} name @return {?} */
goog.requireType = function(name) {};
/** @param {string} name */
goog.forwardDeclare = function(name) {};
/**
 * @param {string} path
 * @param {!Array<string>} provides
 * @param {!Array<string>} requires
 * @param {!Object<string, string>=} opt_loadFlags
 */
goog.addDependency = function(path, provides, requires, opt_loadFlags) {};
/** @param {string} name */
goog.module = function(name) {};
/** @return {void} */
goog.module.declareLegacyNamespace = function() {};
/** @param {string} name @return {?} */
goog.module.get = function(name) {};
/** @param {string} name @param {*} v @return {?} */
goog.define = function(name, v) { return v; };
/** @param {string} n @param {*} o */
goog.exportSymbol = function(n, o) {};
/** @param {!Object} o @param {string} n @param {*} v */
goog.exportProperty = function(o, n, v) {};
/** @param {!Function} c @param {!Function} p */
goog.inherits = function(c, p) {
  /** @constructor */
  function T() {}
  T.prototype = p.prototype;
  c.superClass_ = p.prototype;
  c.prototype = new T();
  /** @override */
  c.prototype.constructor = c;
};
/** @type {!Function} */
goog.abstractMethod = function() { throw new Error('unimplemented abstract method'); };
/** @param {string} c @param {string=} m @return {string} */
goog.getCssName = function(c, m) { return m ? c + '-' + m : c; };
/** @param {string} s @param {!Object<string, string>=} v @return {string} */
goog.getMsg = function(s, v) { return s; };
/** @param {function()} fn */
goog.scope = function(fn) { fn.call(goog.global); };
/** @const */
goog.reflect = {};
/** @param {string} p @param {!Object} o @return {string} */
goog.reflect.objectProperty = function(p, o) { return p; };
/** @const */
goog.object = {};
/** @param {...*} var_args @return {!Object} */
goog.object.create = function(var_args) {
  var r = {};
  for (var i = 0; i < arguments.length; i += 2) r[arguments[i]] = arguments[i + 1];
  return r;
};
/** @param {string} n @param {!Object=} o @return {string} */
var JSCompiler_renameProperty = function(n, o) { return n; };
";

/// Chunk index of file `i` (of `n`) under `chunks2`.
pub fn chunk2(i: usize, n: usize) -> usize {
    usize::from(i >= n.div_ceil(2))
}

/// Chunk index of file `i` (of `n`) under `chunks3`.
pub fn chunk3(i: usize, n: usize) -> usize {
    let k0 = n.div_ceil(3);
    let k1 = (n - k0).div_ceil(2);
    if i < k0 {
        0
    } else if i < k0 + k1 {
        1
    } else {
        2
    }
}

/// May file `i` import file `j` in a program of `n` files?
pub fn may_import(i: usize, j: usize, n: usize) -> bool {
    j < i && chunk2(j, n) <= chunk2(i, n) && (chunk3(j, n) == 0 || chunk3(j, n) == chunk3(i, n))
}

/// One core-language statement at module top level. Statements that mention `this`,
/// `arguments`, `await`, `yield` or `throw` are dropped: a goog.module body may not reference
/// `this` or throw at top level (ClosureCheckModule), and the others are not meaningful at
/// module scope. The statement's declarations are scoped to it, so later code never
/// references a name whose declaration was dropped.
fn filler(g: &mut Gen) {
    for _ in 0..3 {
        let saved = std::mem::take(&mut g.out);
        // Own scope: a dropped attempt must not leave its declarations visible.
        g.push_scope();
        g.nl();
        g.stmt();
        g.pop_scope();
        g.ctx.terminated = false;
        let stmt = std::mem::replace(&mut g.out, saved);
        let bad = ["this", "arguments", "await", "yield", "throw"]
            .iter()
            .any(|w| stmt.contains(w));
        if !bad {
            g.w(&stmt);
            return;
        }
    }
}

/// Program `index` of batch `seed` (pure; independent of the batch size).
/// Closure primitives at the top level of a goog.module / goog.provide file: goog.getCssName,
/// goog.getMsg, goog.reflect.objectProperty, JSCompiler_renameProperty, goog.object.create,
/// `@this` functions and (provide files) goog.scope. Each has a number-typed use, so the
/// passes that rewrite them (closureReplaceGetCssName, replaceMessages,
/// removePropertyRenamingCalls, closureOptimizePrimitives, closureGoogScopeAliases) have
/// work (these were never generated).
fn closure_primitives(
    g: &mut Gen,
    style: &Style,
    ns: &str,
    alias: &[(usize, String, Vec<Item>)],
    mine: &[Item],
) {
    let local = |it: &Item| match style {
        Style::Provide(ns) => format!("{ns}.{}", it.name()),
        _ => it.name().to_string(),
    };
    for _ in 0..g.rng.range(1, 3) {
        g.nl();
        match g.rng.below(8) {
            6 => {
                // goog.forwardDeclare of a type no file defines, used only in JSDoc
                // (closurePrimitives records it; closureProvidesRequires drops the call).
                let t = g.fresh("Fwd");
                let v = g.fresh("fv");
                g.w(&format!("goog.forwardDeclare('gen.fwd.{t}');"));
                g.nl();
                let lhs = match style {
                    Style::Provide(ns) => format!("{ns}.{v}"),
                    _ => format!("const {v}"),
                };
                let r = lhs.trim_start_matches("const ").to_string();
                g.w(&format!(
                    "/** @type {{?gen.fwd.{t}}} */\n{lhs} = null;\nconsole.log({r} === null ? 1 : 0);"
                ));
                continue;
            }
            7 => {
                // goog.addDependency: replaced by `0` in closurePrimitives.
                let d = g.fresh("dep");
                g.w(&format!(
                    "goog.addDependency('gen/{d}.js', ['gen.{d}'], ['gen.m0']);"
                ));
                continue;
            }
            0 => {
                let c = *g.rng.pick(&["gen-btn", "gen-panel", "active", "gen-x-y"]);
                g.w(&format!("console.log(goog.getCssName('{c}').length);"));
            }
            1 => {
                let m = g.fresh("MSG_GEN_");
                let lhs = match style {
                    Style::Provide(ns) => format!("{ns}.{m}"),
                    _ => format!("const {m}"),
                };
                let r = lhs.trim_start_matches("const ").to_string();
                if g.rng.chance(1, 2) {
                    g.w(&format!(
                        "/** @desc A generated message. */\n{lhs} = goog.getMsg('Hello {{$who}}', {{'who': 'w'}});"
                    ));
                } else {
                    g.w(&format!(
                        "/** @desc A generated message. */\n{lhs} = goog.getMsg('Items');"
                    ));
                }
                g.nl();
                g.w(&format!("console.log({r}.length);"));
            }
            2 => match mine.iter().find(|it| matches!(it, Item::Class { .. })) {
                Some(Item::Class { name: _, methods }) => {
                    let it = mine
                        .iter()
                        .find(|it| matches!(it, Item::Class { .. }))
                        .cloned()
                        .unwrap_or_else(|| mine[0].clone());
                    let m = g.rng.pick(methods).clone();
                    let c = local(&it);
                    if g.rng.chance(1, 2) {
                        g.w(&format!(
                            "console.log(goog.reflect.objectProperty('{m}', {c}.prototype).length);"
                        ));
                    } else {
                        g.w(&format!(
                            "console.log(JSCompiler_renameProperty('{m}', {c}.prototype).length);"
                        ));
                    }
                }
                _ => g.w("console.log(JSCompiler_renameProperty('x').length);"),
            },
            3 => {
                g.w("console.log(Object.keys(goog.object.create('a', ");
                super::typed::literal(g, super::typed::Ty::Num);
                g.w(", 'b', 2)).length);");
            }
            4 => {
                // A free function with `@this`, called through .call.
                let f = g.fresh("viaThis");
                let o = g.fresh("o");
                g.w(&format!(
                    "/**\n * @this {{{{n: number}}}}\n * @return {{number}}\n */\nfunction {f}() {{ return this.n + 1; }}\nconst {o} = {{n: "
                ));
                super::typed::literal(g, super::typed::Ty::Num);
                g.w(&format!("}};\nconsole.log({f}.call({o}));"));
            }
            _ => {
                if let Style::Provide(_) = style {
                    // goog.scope with an alias of a required (or this) namespace.
                    let (pfx, its) = match alias.iter().find(|(_, _, its)| !its.is_empty()) {
                        Some((_, p, its)) => (p.trim_end_matches('.').to_string(), its.clone()),
                        None => (ns.to_string(), mine.to_vec()),
                    };
                    let a = g.fresh("sA");
                    g.w(&format!("goog.scope(function() {{\nconst {a} = {pfx};\n"));
                    items::observe(g, &|_| format!("{a}."), &its);
                    g.w("\n});");
                } else {
                    let c = *g.rng.pick(&["gen-a", "gen-b"]);
                    g.w(&format!("console.log(goog.getCssName('{c}').length);"));
                }
            }
        }
    }
}

/// How a goog.module / CommonJS file exports (only per-name exports were
/// generated).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    /// `exports.X = X;` / `module.exports.X = X;` after each declaration.
    EachName,
    /// One `exports = {X, Y};` / `module.exports = {X, Y};` at the end (named exports).
    Object,
    /// `exports = X;` / `module.exports = X;`: the module's value is one item. Importers bind
    /// it to a name and cannot destructure it.
    Value(Item),
}

impl Shape {
    fn named(&self) -> bool {
        !matches!(self, Shape::Value(_))
    }
}

/// `{A, B: r7}` over every name of `items` (each renamed with probability 1/3 when it can be),
/// and the items as reached through the pattern.
fn destructure(g: &mut Gen, items: &[Item]) -> (String, Vec<Item>) {
    let mut parts = vec![];
    let mut reached = vec![];
    for it in items {
        if g.rng.chance(1, 3)
            && let Some(r) = {
                let n = g.fresh("r");
                items::renamed(it, &n)
            }
        {
            parts.push(format!("{}: {}", it.name(), r.name()));
            reached.push(r);
        } else {
            parts.extend(it.names().iter().map(|n| n.to_string()));
            reached.push(it.clone());
        }
    }
    (format!("{{{}}}", parts.join(", ")), reached)
}

/// A module-scope function that reaches file `j` through `goog.module.get` (legal only inside a
/// function: JSC_MODULE_USES_GOOG_MODULE_GET), and a call of it.
fn module_get_use(g: &mut Gen, j: usize, shape: &Shape, its: &[Item]) {
    let f = g.fresh("getM");
    let v = g.fresh("mod");
    g.w(&format!(
        "/** @return {{number}} */\nfunction {f}() {{ const {v} = goog.module.get('gen.m{j}'); return "
    ));
    match shape {
        Shape::Value(it) => {
            let r = items::renamed(it, &v).unwrap_or_else(|| it.clone());
            items::use_number(g, "", &r);
        }
        _ => {
            let it = g.rng.pick(its).clone();
            items::use_number(g, &format!("{v}."), &it);
        }
    }
    g.w(&format!("; }}\nconsole.log({f}());"));
}

pub fn generate_closure_nth(seed: u64, index: u64, cfg: &Config) -> ClosureProgram {
    generate_closure(Rng::fork(seed ^ 0xC105_E000_0000_0000, index), cfg)
}

pub fn generate_closure(rng: rng::Rng, cfg: &Config) -> ClosureProgram {
    let mut g = Gen::new(rng, cfg);
    let kind = *g.rng.pick(&[
        FileKind::GoogModule,
        FileKind::GoogModule,
        FileKind::GoogProvide,
        FileKind::EsModule,
        FileKind::EsModule,
        FileKind::CommonJs,
    ]);
    let has_base = matches!(kind, FileKind::GoogModule | FileKind::GoogProvide);
    // With base.js in c0, two modules would leave the second one nothing legal to import
    // under chunks3 (c0 = {base.js}), so goog programs get at least three.
    let modules = g.rng.range(if has_base { 3 } else { 2 }, 5) as usize;
    let n = modules + usize::from(has_base);
    let mut files: Vec<(String, String)> = vec![];
    let mut deps: Vec<Vec<usize>> = vec![];
    // exports[i] = items exported by file i.
    let mut exports: Vec<Vec<Item>> = vec![];
    let mut has_default: Vec<bool> = vec![];
    // shapes[i]: how goog.module / CommonJS file i exports; default_item[i]: the item an ES
    // module declares as `export default function|class`.
    let mut shapes: Vec<Shape> = vec![];
    let mut default_item: Vec<Option<Item>> = vec![];
    if has_base {
        files.push(("base.js".into(), BASE_JS.into()));
        deps.push(vec![]);
        exports.push(vec![]);
        has_default.push(false);
        shapes.push(Shape::EachName);
        default_item.push(None);
    }
    for i in files.len()..n {
        let name = format!("m{i}.js");
        let ns = format!("gen.m{i}");
        let style = match kind {
            FileKind::GoogModule => Style::GoogModule,
            FileKind::GoogProvide => Style::Provide(ns.clone()),
            FileKind::EsModule => Style::EsModule,
            _ => Style::CommonJs,
        };
        // Choose imports among legal, non-base files.
        let legal: Vec<usize> = (usize::from(has_base)..i)
            .filter(|&j| may_import(i, j, n))
            .collect();
        let mut my_deps = vec![];
        for &j in &legal {
            if g.rng.chance(2, 3) {
                my_deps.push(j);
            }
        }
        if my_deps.is_empty()
            && let Some(&j) = legal.last()
        {
            my_deps.push(j);
        }
        set_kind(kind);
        g.out.clear();
        g.push_scope();
        // Header and imports. `alias[j]` = how file i reaches file j's exports.
        let mut alias: Vec<(usize, String, Vec<Item>)> = vec![];
        let mut legacy = false;
        let mut type_uses: Vec<String> = vec![];
        let mut default_uses: Vec<String> = vec![];
        match kind {
            FileKind::GoogModule => {
                g.w(&format!("goog.module('{ns}');"));
                if g.rng.chance(1, 4) {
                    g.nl();
                    g.w("goog.module.declareLegacyNamespace();");
                    legacy = true;
                }
                for &j in &my_deps {
                    if let Shape::Value(it) = &shapes[j] {
                        // Whole-value export: the require is the value itself.
                        let a = format!("m{j}");
                        g.nl();
                        g.w(&format!("const {a} = goog.require('gen.m{j}');"));
                        let r = items::renamed(it, &a).unwrap_or_else(|| it.clone());
                        alias.push((j, String::new(), vec![r]));
                        continue;
                    }
                    // Sometimes a type-only dependency (goog.requireType), used in JSDoc only.
                    let cls = exports[j].iter().find_map(|it| match it {
                        Item::Class { name, .. } => Some(name.clone()),
                        _ => None,
                    });
                    if !exports[j].is_empty() && g.rng.chance(1, 3) {
                        // Destructuring require (named exports only).
                        let (pat, reached) = destructure(&mut g, &exports[j]);
                        g.nl();
                        g.w(&format!("const {pat} = goog.require('gen.m{j}');"));
                        alias.push((j, String::new(), reached));
                        continue;
                    }
                    if let Some(c) = cls.filter(|_| g.rng.chance(1, 4)) {
                        let t = format!("t{j}");
                        g.nl();
                        g.w(&format!("const {t} = goog.requireType('gen.m{j}');"));
                        type_uses.push(format!("{t}.{c}"));
                        continue;
                    }
                    let a = format!("m{j}");
                    g.nl();
                    g.w(&format!("const {a} = goog.require('gen.m{j}');"));
                    alias.push((j, format!("{a}."), exports[j].clone()));
                }
            }
            FileKind::GoogProvide => {
                g.w(&format!("goog.provide('{ns}');"));
                for &j in &my_deps {
                    g.nl();
                    g.w(&format!("goog.require('gen.m{j}');"));
                    alias.push((j, format!("gen.m{j}."), exports[j].clone()));
                }
            }
            FileKind::EsModule => {
                for &j in &my_deps {
                    if g.rng.chance(1, 8) {
                        // Side-effect import: the dependency is loaded, nothing is bound.
                        g.w(&format!("import './m{j}.js';"));
                        g.nl();
                        continue;
                    }
                    if let Some(it) = default_item[j].clone().filter(|_| g.rng.chance(2, 3)) {
                        // Default import of an `export default function|class` declaration.
                        let a = format!("m{j}");
                        let d = format!("d{j}");
                        g.w(&format!("import {d}, * as {a} from './m{j}.js';"));
                        alias.push((j, format!("{a}."), exports[j].clone()));
                        if let Some(r) = items::renamed(&it, &d) {
                            alias.push((j, String::new(), vec![r]));
                        }
                    } else if has_default[j] && g.rng.chance(2, 3) {
                        // Default import (combined with the namespace import: one import
                        // statement per module).
                        let a = format!("m{j}");
                        g.w(&format!("import d{j}, * as {a} from './m{j}.js';"));
                        alias.push((j, format!("{a}."), exports[j].clone()));
                        default_uses.push(format!("d{j}"));
                    } else if g.rng.chance(1, 2) || exports[j].is_empty() {
                        let a = format!("m{j}");
                        g.w(&format!("import * as {a} from './m{j}.js';"));
                        alias.push((j, format!("{a}."), exports[j].clone()));
                    } else {
                        // Named imports, some renamed (`import {a as b}`).
                        let mut specs = vec![];
                        let mut reached = vec![];
                        for it in &exports[j] {
                            let r = if g.rng.chance(1, 3) {
                                let n = g.fresh("r");
                                items::renamed(it, &n)
                            } else {
                                None
                            };
                            match r {
                                Some(r) => {
                                    specs.push(format!("{} as {}", it.name(), r.name()));
                                    reached.push(r);
                                }
                                None => {
                                    specs.extend(it.names().iter().map(|n| n.to_string()));
                                    reached.push(it.clone());
                                }
                            }
                        }
                        g.w(&format!(
                            "import {{{}}} from './m{j}.js';",
                            specs.join(", ")
                        ));
                        alias.push((j, String::new(), reached));
                    }
                    g.nl();
                }
            }
            _ => {
                for &j in &my_deps {
                    let a = format!("m{j}");
                    if let Shape::Value(it) = &shapes[j] {
                        // `module.exports = X`: the require is the value itself.
                        g.w(&format!("const {a} = require('./m{j}.js');"));
                        let r = items::renamed(it, &a).unwrap_or_else(|| it.clone());
                        alias.push((j, String::new(), vec![r]));
                    } else if !exports[j].is_empty() && g.rng.chance(1, 3) {
                        // Destructuring require.
                        let (pat, reached) = destructure(&mut g, &exports[j]);
                        g.w(&format!("const {pat} = require('./m{j}.js');"));
                        alias.push((j, String::new(), reached));
                    } else {
                        g.w(&format!("const {a} = require('./m{j}.js');"));
                        alias.push((j, format!("{a}."), exports[j].clone()));
                    }
                    g.nl();
                }
            }
        }
        if matches!(kind, FileKind::GoogModule) && g.rng.chance(1, 3) {
            let d = g.fresh("DEF");
            let (ty, v) = *g
                .rng
                .pick(&[("boolean", "false"), ("number", "3"), ("string", "'x'")]);
            g.nl();
            g.w(&format!(
                "/** @define {{{ty}}} */\nconst {d} = goog.define('{ns}.{d}', {v});"
            ));
            g.nl();
            g.w(&format!("console.log({d});"));
        }
        // Declarations.
        items::set_imports(
            alias
                .iter()
                .flat_map(|(_, p, its)| its.iter().map(move |it| (p.clone(), it.clone())))
                .collect(),
        );
        // Export shape (goog.module / CommonJS) or an ES default declaration slot.
        let shape_pick = match kind {
            FileKind::GoogModule | FileKind::CommonJs => g.rng.below(8),
            _ => 0,
        };
        let collect = shape_pick >= 5;
        let es_default = kind == FileKind::EsModule && g.rng.chance(1, 4);
        items::set_export_mode(collect, es_default);
        let k = g.rng.range(1, 4);
        let mut mine = vec![];
        for _ in 0..k {
            g.nl();
            mine.push(items::declare(&mut g, &style));
        }
        let defaulted = items::take_defaulted();
        items::set_export_mode(false, false);
        let shape = if !collect {
            Shape::EachName
        } else if shape_pick == 7 && !legacy {
            match mine
                .iter()
                .filter(|it| items::renamed(it, "_").is_some())
                .count()
            {
                0 => Shape::Object,
                c => {
                    let nth = g.rng.below(c as u64) as usize;
                    let it = mine
                        .iter()
                        .filter(|it| items::renamed(it, "_").is_some())
                        .nth(nth)
                        .cloned()
                        .unwrap_or_else(|| mine[0].clone());
                    Shape::Value(it)
                }
            }
        } else {
            Shape::Object
        };
        // Module-level typed computation over imported and local values (cross-module data
        // flow that ADVANCED cannot fold to a constant).
        if g.rng.chance(1, 2) {
            g.nl();
            let mut env = vec![];
            let acc = items::typed_stmts(&mut g, &mut env, None);
            g.w(&format!(" console.log({acc});"));
        }
        for t in &type_uses {
            let v = g.fresh("tv");
            g.nl();
            g.w(&format!(
                "/** @type {{?{t}}} */\nconst {v} = null;\nconsole.log({v} === null ? 0 : 1);"
            ));
        }
        for d in &default_uses {
            g.nl();
            g.w(&format!("console.log({d});"));
        }
        if matches!(kind, FileKind::GoogModule | FileKind::GoogProvide) {
            closure_primitives(&mut g, &style, &ns, &alias, &mine);
        }
        if kind == FileKind::GoogModule {
            // goog.module.get inside a function (never generated).
            let cands: Vec<usize> = my_deps
                .iter()
                .copied()
                .filter(|&j| !exports[j].is_empty())
                .collect();
            if !cands.is_empty() && g.rng.chance(1, 3) {
                let j = *g.rng.pick(&cands);
                g.nl();
                module_get_use(&mut g, j, &shapes[j], &exports[j]);
            }
        }
        // Some core-language filler (module-local).
        // Rare: untyped core-language code is the main source of type diagnostics, and the
        // single-file programs already cover it.
        if g.rng.chance(1, 4) {
            filler(&mut g);
        }
        // Uses of imported items (cross-file, often cross-chunk).
        for (_, pfx, its) in &alias {
            if its.is_empty() {
                continue;
            }
            g.nl();
            let p = pfx.clone();
            items::observe(&mut g, &|_| p.clone(), its);
        }
        // Wide classes: the declaring file only instantiates them (so crossChunkCodeMotion
        // cannot move the whole class to a deeper chunk) and leaves the method calls to
        // importers, often in a deeper chunk (crossChunkMethodMotion).
        for it in &mine {
            if let Item::Wide { name, .. } = it {
                let local = match &style {
                    Style::Provide(ns) => format!("{ns}.{name}"),
                    _ => name.clone(),
                };
                g.nl();
                g.w(&format!("console.log(new {local}("));
                super::typed::literal(&mut g, super::typed::Ty::Num);
                g.w(&format!(") instanceof {local} ? 1 : 0);"));
            }
        }
        let local_items: Vec<Item> = mine
            .iter()
            .filter(|it| !matches!(it, Item::Wide { .. }))
            .cloned()
            .collect();
        if g.rng.chance(1, 2) {
            g.nl();
            let local_pfx = match &style {
                Style::Provide(ns) => format!("{ns}."),
                _ => String::new(),
            };
            items::observe(&mut g, &|_| local_pfx.clone(), &local_items);
        }
        if matches!(kind, FileKind::GoogModule | FileKind::GoogProvide) && g.rng.chance(1, 3) {
            let it = g.rng.pick(&mine).clone();
            let target = match &style {
                Style::Provide(ns) => format!("{ns}.{}", it.name()),
                _ => it.name().to_string(),
            };
            g.nl();
            g.w(&format!("goog.exportSymbol('ex_{}', {target});", it.name()));
        }
        let mut default = false;
        let mut reexports: Vec<Item> = vec![];
        if kind == FileKind::EsModule {
            // Export specifiers with renaming, star re-exports, default export, dynamic import.
            if g.rng.chance(1, 3) {
                let it = g.rng.pick(&mine).clone();
                g.nl();
                g.w(&format!("export {{{0} as {0}_alias}};", it.name()));
            }
            let star = my_deps.first().copied().filter(|_| g.rng.chance(1, 3));
            if let Some(j) = star {
                g.nl();
                g.w(&format!("export * from './m{j}.js';"));
            }
            // `export {x} from` / `export {x as y} from`. A renamed re-export
            // becomes part of this module's exports; a plain one is not offered to importers
            // (a second binding of the same name would collide in a named import).
            let re_cands: Vec<usize> = my_deps
                .iter()
                .copied()
                .filter(|&j| !exports[j].is_empty())
                .collect();
            if !re_cands.is_empty() && g.rng.chance(1, 3) {
                let j = *g.rng.pick(&re_cands);
                let it = g.rng.pick(&exports[j]).clone();
                let alias_name = g.fresh("re");
                g.nl();
                match items::renamed(&it, &alias_name)
                    .filter(|_| star == Some(j) || g.rng.chance(1, 2))
                {
                    Some(r) => {
                        g.w(&format!(
                            "export {{{} as {alias_name}}} from './m{j}.js';",
                            it.name()
                        ));
                        reexports.push(r);
                    }
                    None => g.w(&format!(
                        "export {{{}}} from './m{j}.js';",
                        it.names().join(", ")
                    )),
                }
            }
            if defaulted.is_none() && !local_items.is_empty() && g.rng.chance(1, 5) {
                g.nl();
                g.w("export default ");
                let it = g.rng.pick(&local_items).clone();
                items::use_number(&mut g, "", &it);
                g.w(";");
                default = true;
            }
            // Rare: under the chunk profiles Java crashes on a dynamic import (AstValidator
            // "AST should not contain Dynamic module import", exit 254, dropped by D-009), so a
            // high rate would waste the chunk budget (3 of 23 chunk runs in a 165-program
            // sample at 1 in 6).
            if let Some(&j) = my_deps.last()
                && !exports[j].is_empty()
                && g.rng.chance(1, 24)
            {
                let it = g.rng.pick(&exports[j]).clone();
                let v = g.fresh("ns");
                g.nl();
                g.w(&format!(
                    "import('./m{j}.js').then(({v}) => {{ console.log({v}.{}); }});",
                    it.name()
                ));
            }
        }
        // Whole-object / whole-value exports (goog.module, CommonJS).
        let target = if kind == FileKind::GoogModule {
            "exports"
        } else {
            "module.exports"
        };
        match &shape {
            Shape::Object => {
                let names: Vec<&str> = mine.iter().flat_map(|it| it.names()).collect();
                g.nl();
                g.w(&format!("{target} = {{{}}};", names.join(", ")));
            }
            Shape::Value(it) => {
                g.nl();
                g.w(&format!("{target} = {};", it.name()));
            }
            Shape::EachName => {}
        }
        items::set_imports(vec![]);
        g.pop_scope();
        g.out.push('\n');
        files.push((name, std::mem::take(&mut g.out)));
        deps.push(my_deps);
        // Named exports offered to importers: an ES default declaration is not a named export;
        // renamed re-exports are.
        let dflt = defaulted
            .as_ref()
            .and_then(|d| mine.iter().find(|it| it.name() == d).cloned());
        let mut named: Vec<Item> = mine
            .iter()
            .filter(|it| Some(it.name()) != defaulted.as_deref())
            .cloned()
            .collect();
        named.extend(reexports);
        exports.push(named);
        has_default.push(default);
        debug_assert!(shape.named() || !matches!(kind, FileKind::EsModule | FileKind::GoogProvide));
        shapes.push(shape);
        default_item.push(dflt);
    }
    set_kind(FileKind::Script);
    let extra_flags = if kind == FileKind::CommonJs {
        vec!["--process_common_js_modules".to_string()]
    } else {
        vec![]
    };
    ClosureProgram {
        kind,
        files,
        extra_flags,
        deps,
    }
}
