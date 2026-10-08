//! minimize CLI (oracle-free): line-level ddmin of a file under "the text contains X".
//! The AST-level path needs a tree source and runs inside `fuzz-driver minimize-selftest` /
//! the fuzz loop, which own the oracle pool.
//!
//!   minimize --file F --contains X [--budget N]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file = None;
    let mut needle = None;
    let mut budget = 10_000usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--file" => {
                i += 1;
                file = args.get(i).cloned();
            }
            "--contains" => {
                i += 1;
                needle = args.get(i).cloned();
            }
            "--budget" => {
                i += 1;
                budget = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(budget);
            }
            o => {
                eprintln!("minimize: unknown argument {o}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let (Some(file), Some(needle)) = (file, needle) else {
        eprintln!("usage: minimize --file F --contains X [--budget N]");
        std::process::exit(2);
    };
    let src = std::fs::read_to_string(&file).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let mut p = |s: &str| s.contains(needle.as_str());
    let (out, st) = minimize::ddmin_lines(&src, &mut p, budget);
    eprintln!(
        "minimize: {} -> {} bytes in {} tests",
        src.len(),
        out.len(),
        st.tests
    );
    print!("{out}");
}
