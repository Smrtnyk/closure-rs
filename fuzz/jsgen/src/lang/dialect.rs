//! Opt-in dialects for syntax the CLI-default parse (strict mode input, `language_in` =
//! STABLE_IN) rejects.
//!
//! The default dialect is what `jsgen::generate_nth` produces and what the driver's parse
//! filter accepts. Two extra dialects exist because the fuzzer needs features that are only
//! legal outside that default:
//! - `sloppy`: legacy octal literals (`017`), legacy octal and `\8` escapes in strings,
//!   `with`, `delete identifier` and Annex B `if (x) function f() {}`. Compile them with
//!   `--strict_mode_input=false` (StrictModeCheck reports `with`; IRFactory warns on legacy
//!   octal escapes in a strict context). Measured: `08`/`09` and strict-mode reserved words
//!   as identifiers are rejected by the parser in every mode, so they are never emitted.
//! - `unsupported`: private class elements (`#x`, `#m()`, `#x in o`). The parser parses
//!   them, but FeatureSet marks `PRIVATE_ELEMENTS` as `ES_UNSUPPORTED`, so every CLI
//!   language mode reports `JSC_UNSUPPORTED_LANGUAGE_FEATURE` ("Private elements") and the
//!   CLI refuses `--language_in=UNSUPPORTED` ("Cannot specify the unsupported set of
//!   features for language_in"). These programs therefore exercise only the parser and the
//!   error path; they need no extra flags.
//!
//! The dialect lives in a thread-local so that productions (plain `fn`s) can gate on it
//! without changing `engine.rs`. It is set only for the duration of [`generate_nth`], so the
//! default stream is unchanged: dialect-only productions have `when == false` and add no
//! weight.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dialect {
    pub sloppy: bool,
    pub unsupported: bool,
}

impl Dialect {
    pub const DEFAULT: Dialect = Dialect {
        sloppy: false,
        unsupported: false,
    };
    pub const SLOPPY: Dialect = Dialect {
        sloppy: true,
        unsupported: false,
    };
    pub const UNSUPPORTED: Dialect = Dialect {
        sloppy: false,
        unsupported: true,
    };

    /// Extra compiler flags a program of this dialect needs (append to the profile argv).
    pub fn flags(self) -> Vec<&'static str> {
        let mut v = vec![];
        if self.sloppy {
            v.push("--strict_mode_input=false");
        }
        v
    }

    pub fn name(self) -> &'static str {
        match (self.sloppy, self.unsupported) {
            (false, false) => "default",
            (true, false) => "sloppy",
            (false, true) => "unsupported",
            (true, true) => "sloppy+unsupported",
        }
    }
}

thread_local! {
    static CURRENT: Cell<Dialect> = const { Cell::new(Dialect::DEFAULT) };
    static LOW_TARGET: Cell<bool> = const { Cell::new(false) };
}

/// True while generating for a profile whose `--language_out` is below ES2018
/// (`lang_es5`, `lang_es2015`). Java reports `JSC_UNTRANSPILABLE` and writes no output for
/// BigInt, the ES2018+ regexp features (`s`/`d` flags, lookbehind, named groups, `\p{..}`)
/// and `new.target` outside a class constructor, so a program containing them never reaches
/// the transpilation and optimisation passes those profiles exist for
/// (63% of lang programs failed lang_es5). Productions for those features are disabled then.
pub fn low_target() -> bool {
    LOW_TARGET.with(|c| c.get())
}

struct ResetLow(bool);

impl Drop for ResetLow {
    fn drop(&mut self) {
        LOW_TARGET.with(|c| c.set(self.0));
    }
}

/// Run `f` with [`low_target`] set to `on` (restored afterwards, also on panic).
pub fn with_low_target<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let _reset = ResetLow(low_target());
    LOW_TARGET.with(|c| c.set(on));
    f()
}

pub fn current() -> Dialect {
    CURRENT.with(|c| c.get())
}

pub fn sloppy() -> bool {
    current().sloppy
}

pub fn unsupported() -> bool {
    current().unsupported
}

struct Reset(Dialect);

impl Drop for Reset {
    fn drop(&mut self) {
        CURRENT.with(|c| c.set(self.0));
    }
}

/// Run `f` with dialect `d` active (restored afterwards, also on panic).
pub fn with_dialect<T>(d: Dialect, f: impl FnOnce() -> T) -> T {
    let _reset = Reset(current());
    CURRENT.with(|c| c.set(d));
    f()
}

/// Program `index` of batch `seed` in dialect `d`. `Dialect::DEFAULT` gives exactly
/// `jsgen::generate_nth(seed, index, cfg)`.
pub fn generate_nth(seed: u64, index: u64, cfg: &crate::Config, d: Dialect) -> String {
    with_dialect(d, || crate::generate_nth(seed, index, cfg))
}
