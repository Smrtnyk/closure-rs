//! Writes Closure-builder programs for the oracle measurement in `build/fuzz/closure/`.
//! Not a test (docs/PORTING.md §6 item 4 forbids ignored tests), so it lives here as an example:
//!
//! `CLOSURE_DUMP_DIR=... CLOSURE_DUMP_SEED=... CLOSURE_DUMP_COUNT=...
//!  cargo run -p jsgen --release --example dump_for_measurement`
//!
//! Output: `<dir>/multi/<i>/<file>.js` (multi-file program `i`), `<dir>/single/p<i>.js`
//! (single-file program `i`, closure on) and `<dir>/manifest.jsonl`.

use jsgen::{Config, generate_closure_nth, generate_nth};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("dump_for_measurement: set {name}");
        std::process::exit(2)
    })
}

fn main() {
    let dir = std::path::PathBuf::from(env("CLOSURE_DUMP_DIR"));
    let seed: u64 = env("CLOSURE_DUMP_SEED").parse().expect("CLOSURE_DUMP_SEED");
    let count: u64 = env("CLOSURE_DUMP_COUNT")
        .parse()
        .expect("CLOSURE_DUMP_COUNT");
    let cfg = Config::default();
    let mut manifest = String::new();
    for i in 0..count {
        let p = generate_closure_nth(seed, i, &cfg);
        let d = dir.join(format!("multi/{i}"));
        std::fs::create_dir_all(&d).expect("create program dir");
        let mut names = vec![];
        for (name, src) in &p.files {
            std::fs::write(d.join(name), src).expect("write file");
            names.push(format!("\"{name}\""));
        }
        let flags: Vec<String> = p.extra_flags.iter().map(|f| format!("\"{f}\"")).collect();
        manifest.push_str(&format!(
            "{{\"index\": {i}, \"kind\": \"{:?}\", \"files\": [{}], \"extra_flags\": [{}]}}\n",
            p.kind,
            names.join(", "),
            flags.join(", ")
        ));
        let s = dir.join("single");
        std::fs::create_dir_all(&s).expect("create single dir");
        std::fs::write(s.join(format!("p{i}.js")), generate_nth(seed, i, &cfg))
            .expect("write program");
    }
    std::fs::write(dir.join("manifest.jsonl"), manifest).expect("write manifest");
}
