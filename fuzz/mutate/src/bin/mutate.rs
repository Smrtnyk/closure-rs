//! mutate CLI: `mutate --file F --seed S [--n N] [--d2-donors]` prints the mutant to stdout
//! and the applied operators to stderr. With `--d2-donors`, statements and expressions are
//! spliced from every visible single-input D2 input (the driver's pool); otherwise from F.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file = None;
    let mut seed = 0u64;
    let mut n = 1u32;
    let mut d2 = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--file" => {
                i += 1;
                file = args.get(i).cloned();
            }
            "--seed" => {
                i += 1;
                seed = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "--n" => {
                i += 1;
                n = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1);
            }
            "--d2-donors" => d2 = true,
            o => {
                eprintln!("mutate: unknown argument {o}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let Some(file) = file else {
        eprintln!("usage: mutate --file F --seed S [--n N] [--d2-donors]");
        std::process::exit(2)
    };
    let src = std::fs::read_to_string(&file).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let donors = d2.then(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let srcs: Vec<String> = mutate::visible_d2_inputs(&root, 32 * 1024)
            .iter()
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .collect();
        mutate::Donors::from_sources(&srcs)
    });
    let m = mutate::mutate_with(&src, donors.as_ref(), seed, n);
    eprintln!(
        "mutate: ops {:?}",
        m.ops.iter().map(|o| o.name()).collect::<Vec<_>>()
    );
    print!("{}", m.text);
}
