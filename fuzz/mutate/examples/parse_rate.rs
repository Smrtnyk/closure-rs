//! Java parse-acceptance rate of the mutator (Gate 0.3 style), measured with the oracle.
//!
//!   cargo run --release -p mutate --example parse_rate -- --seeds 2000 [--start 1] \
//!       [--servers 4] [--out build/fuzz/mutate-minimize/parse-rate.json]
//!
//! For seed `s`: `Rng::fork(start, s)` picks a visible single-input D2 file (the driver's
//! filter, at most 32 KB) and a mutation count `n` in 1..=3, then `mutate_with` applies `n`
//! operators with the whole D2 pool as donors. Every mutant and every base file goes
//! through the oracle's `parse_dump` with CLI default options (the driver's parse filter);
//! accepted means `errors == []`. Servers run in the golden environment (D-010).

use fuzz_oracle::{Server, repo_root};
use jsgen::rng::Rng;
use mutate::{Donors, Op, mutate_with};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn parse_errors(s: &mut Server, src: &str) -> Result<Vec<String>, String> {
    let r = s.request(
        json!({"op": "parse_dump", "content": src, "name": "input.js"}),
        Duration::from_secs(120),
    )?;
    if r.get("ok") != Some(&Value::Bool(true)) {
        return Err("parse_dump failed".into());
    }
    Ok(r.get("errors")
        .and_then(|e| e.as_array())
        .map(|a| {
            a.iter()
                .map(|d| {
                    format!(
                        "{}@{}:{} {}",
                        d.get("key").and_then(|v| v.as_str()).unwrap_or("?"),
                        d.get("lineno").and_then(|v| v.as_i64()).unwrap_or(-1),
                        d.get("charno").and_then(|v| v.as_i64()).unwrap_or(-1),
                        d.get("description").and_then(|v| v.as_str()).unwrap_or("")
                    )
                })
                .collect()
        })
        .unwrap_or_default())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str, d: &str| -> String {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1).cloned())
            .unwrap_or_else(|| d.to_string())
    };
    let seeds: u64 = get("--seeds", "2000").parse().unwrap();
    let start: u64 = get("--start", "1").parse().unwrap();
    let servers: usize = get("--servers", "4").parse().unwrap();
    let root = repo_root();
    let out = root.join(get("--out", "build/fuzz/mutate-minimize/parse-rate.json"));
    let files = mutate::visible_d2_inputs(&root, 32 * 1024);
    let srcs: Vec<String> = files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap_or_default())
        .collect();
    let t0 = Instant::now();
    let donors = Arc::new(Donors::from_sources(&srcs));
    eprintln!(
        "{} files, donors {:?}, built in {:.1}s",
        files.len(),
        donors.len(),
        t0.elapsed().as_secs_f64()
    );
    let srcs = Arc::new(srcs);
    let next = Arc::new(AtomicU64::new(0));
    let base_ok: Arc<Mutex<BTreeMap<usize, bool>>> = Arc::default();
    let rows: Arc<Mutex<Vec<Value>>> = Arc::default();
    let mut hs = vec![];
    for _ in 0..servers {
        let (donors, srcs, next, base_ok, rows) = (
            donors.clone(),
            srcs.clone(),
            next.clone(),
            base_ok.clone(),
            rows.clone(),
        );
        hs.push(std::thread::spawn(move || {
            let mut s = Server::start("1536m").expect("oracle");
            loop {
                let k = next.fetch_add(1, Ordering::SeqCst);
                if k >= seeds {
                    return;
                }
                let seed = start + k;
                let mut r = Rng::fork(start, seed);
                let fi = r.below(srcs.len() as u64) as usize;
                let n = r.range(1, 3);
                let m = mutate_with(&srcs[fi], Some(&donors), r.next_u64(), n);
                let cached = base_ok.lock().unwrap().get(&fi).copied();
                let bok = match cached {
                    Some(b) => b,
                    None => {
                        let b = parse_errors(&mut s, &srcs[fi]).is_ok_and(|e| e.is_empty());
                        base_ok.lock().unwrap().insert(fi, b);
                        b
                    }
                };
                let errs = match parse_errors(&mut s, &m.text) {
                    Ok(e) => e,
                    Err(e) => {
                        s = Server::start("1536m").expect("oracle restart");
                        vec![format!("ORACLE: {e}")]
                    }
                };
                rows.lock().unwrap().push(json!({
                    "seed": seed, "file": fi, "n": n,
                    "ops": m.ops.iter().map(|o| o.name()).collect::<Vec<_>>(),
                    "base_ok": bok, "changed": m.text != srcs[fi], "errors": errs,
                }));
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    let mut rows = rows.lock().unwrap().clone();
    rows.sort_by_key(|r| r["seed"].as_u64());
    let ok = |r: &Value| r["errors"].as_array().is_some_and(|e| e.is_empty());
    let total = rows.len();
    let accepted = rows.iter().filter(|r| ok(r)).count();
    let on_ok: Vec<&Value> = rows.iter().filter(|r| r["base_ok"] == true).collect();
    let accepted_ok = on_ok.iter().filter(|r| ok(r)).count();
    let changed = rows.iter().filter(|r| r["changed"] == true).count();
    // Per operator: mutants with exactly one operator, on parseable bases.
    let mut per_op = serde_json::Map::new();
    for o in Op::ALL {
        let single: Vec<&&Value> = on_ok
            .iter()
            .filter(|r| {
                r["ops"]
                    .as_array()
                    .is_some_and(|a| a.len() == 1 && a[0] == o.name())
            })
            .collect();
        let any: Vec<&&Value> = on_ok
            .iter()
            .filter(|r| {
                r["ops"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x == o.name()))
            })
            .collect();
        per_op.insert(
            o.name().into(),
            json!({
                "single_op": single.len(), "single_op_accepted": single.iter().filter(|r| ok(r)).count(),
                "any": any.len(), "any_accepted": any.iter().filter(|r| ok(r)).count(),
            }),
        );
    }
    let rejected: Vec<Value> = rows
        .iter()
        .filter(|r| r["base_ok"] == true && !ok(r))
        .map(|r| {
            json!({"seed": r["seed"], "file": files[r["file"].as_u64().unwrap() as usize]
                .strip_prefix(&root).unwrap().to_string_lossy(), "ops": r["ops"], "errors": r["errors"]})
        })
        .collect();
    let bases = base_ok.lock().unwrap().clone();
    let report = json!({
        "method": "oracle parse_dump, CLI default options (STABLE_IN), golden env; accepted = errors == []",
        "seeds": seeds, "start": start, "files_in_pool": files.len(),
        "donors": {"stmts": donors.len().0, "exprs": donors.len().1, "strings": donors.len().2, "regexes": donors.len().3},
        "mutants": total, "accepted": accepted, "rate_all": accepted as f64 / total.max(1) as f64,
        "mutants_on_parseable_base": on_ok.len(), "accepted_on_parseable_base": accepted_ok,
        "rate_on_parseable_base": accepted_ok as f64 / on_ok.len().max(1) as f64,
        "changed": changed,
        "bases_checked": bases.len(), "bases_parseable": bases.values().filter(|b| **b).count(),
        "per_op": per_op, "rejected_on_parseable_base": rejected,
        "wall_s": t0.elapsed().as_secs_f64(),
    });
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!(
        "mutants {total}: accepted {accepted} ({:.4}); on parseable bases {accepted_ok}/{} ({:.4}); changed {changed}; report {}",
        accepted as f64 / total.max(1) as f64,
        on_ok.len(),
        accepted_ok as f64 / on_ok.len().max(1) as f64,
        out.display()
    );
}
