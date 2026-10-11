//! Strict-aware generation for the `advanced_strict` profile (`--warning_level=VERBOSE
//! --jscomp_error=*`).
//!
//! Under that profile every warning is an error, including the lint group (`var`, missing
//! JSDoc, `goog.provide`, default exports, unsorted requires, ...). A compile with an error
//! still runs every optimization pass but writes no output, so the differential comparison
//! sees only diagnostics and the exit code. Random core-language code and most mutants
//! always trip at least one check, so almost no `advanced_strict` run had its optimized
//! output compared.
//!
//! While [`active`] is set, the Closure generator emits only code meant to be clean under
//! that profile:
//! - single-file programs ([`generate_strict_nth`]) are built from the typed items of
//!   `items.rs` (script style) and typed statements over them, never from the untyped
//!   core-language productions;
//! - multi-file programs (`program.rs`) leave out the forms the lint checks reject
//!   (`goog.provide` and CommonJS files, default exports, `var`, unsorted or unused imports,
//!   untyped filler), and `items.rs`/`typed.rs` the forms the type checker or the lint checks
//!   reject (unused or never-modified private fields, `@override` without `@return`, dead
//!   branches, fractional bitwise operands, constants initialized to `null`).
//!
//! The mode is a thread-local, like `lang::dialect::low_target`, so productions (plain `fn`s)
//! can consult it; it is set only for the duration of one generation.

use super::items::{self, Item, Style};
use super::typed::{self, Ty};
use crate::engine::Gen;
use crate::rng::Rng;
use crate::{Config, MAX_OBSERVED};
use std::cell::Cell;

thread_local! {
    static STRICT: Cell<bool> = const { Cell::new(false) };
}

/// True while generating a program for `advanced_strict` in strict-aware mode.
pub fn active() -> bool {
    STRICT.with(|c| c.get())
}

struct Reset(bool);

impl Drop for Reset {
    fn drop(&mut self) {
        STRICT.with(|c| c.set(self.0));
    }
}

/// Run `f` with [`active`] set to `on` (restored afterwards, also on panic).
pub fn with_strict<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let _reset = Reset(active());
    STRICT.with(|c| c.set(on));
    f()
}

/// Salt of the single-file strict stream (independent of `generate_nth`'s).
const STRICT_SALT: u64 = 0x5354_5249_4354_0000;

/// Single-file strict program `index` of batch `seed` (pure; independent of the batch size).
pub fn generate_strict_nth(seed: u64, index: u64, cfg: &Config) -> String {
    with_strict(true, || {
        generate_strict(Rng::fork(seed ^ STRICT_SALT, index), cfg)
    })
}

fn generate_strict(rng: Rng, cfg: &Config) -> String {
    let mut g = Gen::new(rng, cfg);
    let mut mine: Vec<Item> = vec![];
    let n = g.rng.range(3, 8);
    for i in 0..n {
        if i > 0 {
            g.nl();
        }
        // Later bodies call into earlier items (data flow ADVANCED cannot fold away).
        items::set_imports(mine.iter().map(|it| (String::new(), it.clone())).collect());
        mine.push(items::declare(&mut g, &Style::Script));
        if g.rng.chance(1, 3) {
            g.nl();
            let mut env = vec![];
            let acc = items::typed_stmts(&mut g, &mut env, None);
            g.w(&format!(" console.log({acc});"));
        }
    }
    items::set_imports(mine.iter().map(|it| (String::new(), it.clone())).collect());
    // Top-level typed computation over the items, then observe them.
    let mut env: Vec<(String, Ty)> = vec![];
    for _ in 0..g.rng.range(0, 2) {
        let v = g.fresh("k");
        let ty = typed::pick_scalar(&mut g);
        g.nl();
        g.w(&format!("/** @type {{{}}} */\nconst {v} = ", ty.jsdoc()));
        typed::value(&mut g, ty, &env);
        g.w(";");
        env.push((v, ty));
    }
    g.nl();
    let acc = items::typed_stmts(&mut g, &mut env, None);
    let mut seen: Vec<String> = env
        .iter()
        .filter(|(n, _)| *n != acc)
        .map(|(n, t)| {
            let save = std::mem::take(&mut g.out);
            typed::use_as_number(&mut g, n, *t);
            std::mem::replace(&mut g.out, save)
        })
        .collect();
    seen.push(acc);
    if seen.len() > MAX_OBSERVED {
        seen.drain(..seen.len() - MAX_OBSERVED);
    }
    g.w(&format!(" console.log({});", seen.join(", ")));
    for _ in 0..g.rng.range(1, 3) {
        g.nl();
        items::observe(&mut g, &|_| String::new(), &mine);
    }
    items::set_imports(vec![]);
    g.out.push('\n');
    g.out
}
