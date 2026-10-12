//! jsgen CLI.
//!
//!   jsgen --seed S [--index I]                     print one program to stdout
//!   jsgen --seed S --count N --out-dir D           write D/p<I>.js for I in 0..N
//!   options: --max-depth K (<= 64), --no-closure, --max-top N, --strict (the strict-aware
//!   single-file generator for advanced_strict, `generate_strict_nth`)

use jsgen::{Config, generate_nth, generate_strict_nth};
use std::path::PathBuf;

fn main() {
    let mut seed: u64 = 0;
    let mut index: u64 = 0;
    let mut count: Option<u64> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut cfg = Config::default();
    let mut strict = false;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    let val = |i: &mut usize| -> String {
        *i += 1;
        args.get(*i)
            .cloned()
            .unwrap_or_else(|| die("missing value"))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => seed = val(&mut i).parse().unwrap_or_else(|_| die("bad --seed")),
            "--index" => index = val(&mut i).parse().unwrap_or_else(|_| die("bad --index")),
            "--count" => count = Some(val(&mut i).parse().unwrap_or_else(|_| die("bad --count"))),
            "--out-dir" => out_dir = Some(PathBuf::from(val(&mut i))),
            "--max-depth" => {
                cfg.max_depth = val(&mut i)
                    .parse()
                    .unwrap_or_else(|_| die("bad --max-depth"))
            }
            "--max-top" => {
                cfg.max_top = val(&mut i).parse().unwrap_or_else(|_| die("bad --max-top"))
            }
            "--no-closure" => cfg.closure = false,
            "--strict" => strict = true,
            "-h" | "--help" => {
                println!(
                    "usage: jsgen --seed S [--index I | --count N --out-dir D] [--max-depth K] [--max-top N] [--no-closure] [--strict]"
                );
                return;
            }
            other => die(&format!("unknown argument {other}")),
        }
        i += 1;
    }
    let gen_one = |k: u64| {
        if strict {
            generate_strict_nth(seed, k, &cfg)
        } else {
            generate_nth(seed, k, &cfg)
        }
    };
    match (count, out_dir) {
        (Some(n), Some(d)) => {
            std::fs::create_dir_all(&d).unwrap_or_else(|e| die(&e.to_string()));
            for k in 0..n {
                let p = d.join(format!("p{k}.js"));
                std::fs::write(&p, gen_one(k)).unwrap_or_else(|e| die(&e.to_string()));
            }
        }
        (None, None) => print!("{}", gen_one(index)),
        _ => die("--count and --out-dir go together"),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("jsgen: {msg}");
    std::process::exit(2)
}
