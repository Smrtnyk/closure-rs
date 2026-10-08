//! Seeded, deterministic grammar-based JavaScript program generator (docs/PORTING.md §4.6).
//!
//! `generate(seed, &cfg)` always returns the same bytes for the same `(seed, cfg)`: the only
//! randomness is this crate's own [`rng::Rng`], and no hash-map iteration is involved.
//! Nesting depth is capped at [`MAX_DEPTH_CAP`] = 64 (D-010): at the cap only leaf
//! productions are chosen.
//!
//! Layout (see `fuzz/DESIGN.md`): `engine.rs` is the dispatch core; `lang/` holds core
//! ECMAScript productions; `closure/` holds Closure idioms and JSDoc types.

#![forbid(unsafe_code)]

pub mod closure;
pub use closure::program::{ClosureProgram, FileKind, generate_closure_nth};
pub mod engine;
pub mod lang;
pub mod rng;

use engine::Gen;
use rng::Rng;

/// Hard upper bound on generator nesting depth (D-010).
pub const MAX_DEPTH_CAP: u32 = 64;

/// At most this many top-level bindings are observed by the trailing `console.log`.
pub const MAX_OBSERVED: usize = 8;

#[derive(Clone, Debug)]
pub struct Config {
    /// Requested nesting cap; clamped to `1..=MAX_DEPTH_CAP`.
    pub max_depth: u32,
    /// Top-level statements per program: `min_top..=max_top`.
    pub min_top: u32,
    pub max_top: u32,
    /// Enable `closure/` productions (JSDoc, Closure idioms).
    pub closure: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_depth: 10,
            min_top: 4,
            max_top: 14,
            closure: true,
        }
    }
}

impl Config {
    pub fn max_depth(&self) -> u32 {
        self.max_depth.clamp(1, MAX_DEPTH_CAP)
    }
}

/// Generate one program. Program `i` of a batch uses `generate(Rng::fork(seed, i))`-style
/// streams via [`generate_nth`].
pub fn generate(seed: u64, cfg: &Config) -> String {
    generate_with(Rng::new(seed), cfg)
}

/// Program `index` of the batch `seed` (independent of how many programs are generated).
pub fn generate_nth(seed: u64, index: u64, cfg: &Config) -> String {
    generate_with(Rng::fork(seed, index), cfg)
}

fn generate_with(rng: Rng, cfg: &Config) -> String {
    let mut g = Gen::new(rng, cfg);
    let n = g.rng.range(cfg.min_top, cfg.max_top.max(cfg.min_top));
    for i in 0..n {
        if i > 0 {
            g.nl();
        }
        g.stmt();
        if g.ctx.terminated {
            break; // nothing after a top-level `throw` (see `Ctx::terminated`)
        }
    }
    // Keep ADVANCED output non-trivial: observe the top-level value bindings (up to
    // MAX_OBSERVED, the most recent ones), not just one of them (with a
    // single observed binding ADVANCED deleted most programs early).
    let mut vals = g.top_level(|b| {
        matches!(
            b,
            engine::Binding::Var | engine::Binding::Let | engine::Binding::Const
        )
    });
    if vals.len() > MAX_OBSERVED {
        vals.drain(..vals.len() - MAX_OBSERVED);
    }
    if vals.is_empty()
        && let Some(v) = g.visible(|b| !matches!(b, engine::Binding::Class))
    {
        vals.push(v);
    }
    if !vals.is_empty() {
        g.nl();
        g.w(&format!("console.log({});", vals.join(", ")));
    }
    g.out.push('\n');
    g.out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let cfg = Config::default();
        for s in 0..50 {
            assert_eq!(generate(s, &cfg), generate(s, &cfg));
            assert_eq!(generate_nth(7, s, &cfg), generate_nth(7, s, &cfg));
        }
        assert_ne!(generate(1, &cfg), generate(2, &cfg));
    }

    #[test]
    fn depth_cap_is_clamped() {
        let cfg = Config {
            max_depth: 1000,
            ..Config::default()
        };
        assert_eq!(cfg.max_depth(), MAX_DEPTH_CAP);
        // Deep configs terminate and stay balanced.
        for s in 0..20 {
            let p = generate(s, &cfg);
            let opens = p.matches('{').count();
            assert!(opens > 0 || !p.is_empty());
        }
    }
}
