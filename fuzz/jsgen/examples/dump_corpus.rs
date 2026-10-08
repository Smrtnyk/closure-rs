//! Corpus dump for the oracle parse-acceptance measurement of the core-ECMAScript
//! generator (`build/fuzz/lang/parse_rate.py`). Not a test (docs/PORTING.md §6 item 4 forbids ignored
//! tests), so it lives here as an example:
//!
//! `JSGEN_LANG_DUMP=<dir> JSGEN_LANG_SEEDS=<n> JSGEN_LANG_DIALECT=default|sloppy|unsupported
//!  JSGEN_LANG_CLOSURE=0|1 cargo run -p jsgen --release --example dump_corpus`
//!
//! The program for seed `s` (1..=n) is `dialect::generate_nth(s, 0, cfg, d)`, written to
//! `<dir>/s<s>.js`.

use jsgen::Config;
use jsgen::lang::dialect::{self, Dialect};

fn main() {
    let Ok(dir) = std::env::var("JSGEN_LANG_DUMP") else {
        eprintln!("dump_corpus: set JSGEN_LANG_DUMP=<dir>");
        std::process::exit(2);
    };
    let n: u64 = std::env::var("JSGEN_LANG_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let d = match std::env::var("JSGEN_LANG_DIALECT").as_deref() {
        Ok("sloppy") => Dialect::SLOPPY,
        Ok("unsupported") => Dialect::UNSUPPORTED,
        _ => Dialect::DEFAULT,
    };
    let cfg = Config {
        closure: std::env::var("JSGEN_LANG_CLOSURE").as_deref() == Ok("1"),
        ..Config::default()
    };
    std::fs::create_dir_all(&dir).expect("create dump dir");
    for s in 1..=n {
        let p = dialect::generate_nth(s, 0, &cfg, d);
        std::fs::write(format!("{dir}/s{s}.js"), p).expect("write program");
    }
}
