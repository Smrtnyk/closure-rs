//! Oracle-backed proof of the minimizer: 20 synthetic predicates and one real Java crash.
//!
//!   cargo run --release -p minimize --example selftest -- [--workers 4] [--budget 300]
//!       [--deadline-s 150] [--crash-case closure-library-file-structs.circularbuffer]
//!       [--crash-profile advanced] [--out build/fuzz/mutate-minimize/minimize-selftest.json]
//!
//! Every candidate must parse (`parse_dump`, CLI defaults, golden env) before the predicate
//! runs. Predicates (all built deterministically):
//!   * 12 x "Java SIMPLE output still contains NEEDLE" on jsgen programs (seed 20261006);
//!   * 4 x "Java SIMPLE still reports warning key K" on jsgen programs;
//!   * 4 x "Java SIMPLE output still contains word W" on small visible D2 inputs;
//!   * 1 real divergence: a visible D2 case listed in corpus/d2/JAVA_FAILURES.md whose Java
//!     compile crashes (exit 254): "the crash signature (exit code, exception class, first
//!     com.google.javascript frame) persists".
//!
//! Two predicates are minimized twice to check determinism.

use fuzz_oracle::{ArgsHelper, Outcome, Server, repo_root};
use minimize::{Config, Predicate, Tree, TreeSource, reduce_with};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Oracle {
    s: Server,
    args: ArgsHelper,
    dir: PathBuf,
}

impl Oracle {
    fn start(dir: PathBuf) -> Oracle {
        std::fs::create_dir_all(&dir).unwrap();
        Oracle {
            s: Server::start("1536m").expect("oracle"),
            args: ArgsHelper::start().expect("fuzz_args"),
            dir,
        }
    }

    fn restart(&mut self) {
        self.s = Server::start("1536m").expect("oracle restart");
    }

    fn parse(&mut self, src: &str) -> Option<Value> {
        let r = match self.s.request(
            json!({"op": "parse_dump", "content": src, "name": "input.js"}),
            Duration::from_secs(120),
        ) {
            Ok(r) => r,
            Err(_) => {
                self.restart();
                return None;
            }
        };
        let errs = r.get("errors").and_then(|e| e.as_array()).map(|a| a.len());
        (r.get("ok") == Some(&Value::Bool(true)) && errs == Some(0)).then_some(r)
    }

    fn compile(&mut self, src: &str, profile: &str) -> Option<Outcome> {
        let root = repo_root();
        let input = self.dir.join("input.js");
        let out = self.dir.join("out");
        std::fs::write(&input, src).ok()?;
        let _ = std::fs::remove_dir_all(&out);
        let rel = |p: &PathBuf| {
            p.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        };
        let a = self
            .args
            .ask(&json!({"id": "selftest", "inputs": [rel(&input)], "profile": profile, "out_dir": rel(&out)}))
            .ok()?;
        let argv: Vec<String> = a["args"]
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        match self.s.request(
            json!({"op": "compile", "args": argv}),
            Duration::from_secs(180),
        ) {
            Ok(r) => Outcome::from_compile(&r).ok(),
            Err(_) => {
                self.restart();
                None
            }
        }
    }
}

struct Trees<'a>(&'a RefCell<Oracle>);

impl TreeSource for Trees<'_> {
    fn tree(&mut self, src: &str) -> Option<Tree> {
        let r = self.0.borrow_mut().parse(src)?;
        Tree::from_parse_dump(&r, src)
    }
}

#[derive(Clone, Debug)]
enum Check {
    Contains(String),
    Warning(String),
    Crash { class: String, frame: String },
}

#[derive(Clone, Debug)]
struct Spec {
    name: String,
    origin: String,
    src: String,
    profile: String,
    check: Check,
}

fn crash_signature(o: &Outcome) -> Option<(String, String)> {
    if o.exit_code != 254 {
        return None;
    }
    let err = String::from_utf8_lossy(&o.stderr);
    let class = err
        .lines()
        .find(|l| l.contains("Exception") || l.contains("Error"))?
        .split(':')
        .next()?
        .trim()
        .trim_start_matches("Exception in thread \"main\" ")
        .to_string();
    let frame = err
        .lines()
        .find(|l| l.trim_start().starts_with("at com.google.javascript"))?
        .trim()
        .split('(')
        .next()?
        .to_string();
    Some((class, frame))
}

fn holds(o: &Outcome, c: &Check) -> bool {
    match c {
        Check::Contains(n) => o.exit_code == 0 && o.all_text().contains(n.as_str()),
        Check::Warning(k) => String::from_utf8_lossy(&o.stderr).contains(k.as_str()),
        Check::Crash { class, frame } => {
            crash_signature(o).is_some_and(|(c, f)| &c == class && &f == frame)
        }
    }
}

fn longest_word(text: &str) -> Option<String> {
    let mut words: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.len() >= 6 && !w.chars().next().unwrap().is_ascii_digit())
        .collect();
    words.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    words.first().map(|w| w.to_string())
}

fn build_specs(o: &RefCell<Oracle>, crash_case: &str, crash_profile: &str) -> Vec<Spec> {
    let root = repo_root();
    let cfg = jsgen::Config::default();
    let mut specs = vec![];
    let needles = [
        "switch",
        "while",
        "class ",
        "try",
        "for(",
        "typeof",
        "instanceof",
        "new ",
        "throw",
        "break",
        "continue",
        "?",
    ];
    let mut idx = 0u64;
    for n in needles {
        while idx < 2000 {
            let src = jsgen::generate_nth(20261006, idx, &cfg);
            idx += 1;
            if src.len() < 300 || o.borrow_mut().parse(&src).is_none() {
                continue;
            }
            let Some(out) = o.borrow_mut().compile(&src, "simple") else {
                continue;
            };
            if out.exit_code == 0 && out.all_text().contains(n) {
                specs.push(Spec {
                    name: format!("contains {n:?}"),
                    origin: format!("jsgen:seed=20261006:index={}", idx - 1),
                    src,
                    profile: "simple".into(),
                    check: Check::Contains(n.into()),
                });
                break;
            }
        }
    }
    let mut keys_seen = vec![];
    while idx < 4000 && keys_seen.len() < 4 {
        let src = jsgen::generate_nth(20261006, idx, &cfg);
        idx += 1;
        if src.len() < 300 || o.borrow_mut().parse(&src).is_none() {
            continue;
        }
        let Some(out) = o.borrow_mut().compile(&src, "simple") else {
            continue;
        };
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        let Some(k) = err
            .split('[')
            .skip(1)
            .filter_map(|s| s.split(']').next())
            .find(|k| k.starts_with("JSC_") && !keys_seen.contains(&k.to_string()))
        else {
            continue;
        };
        keys_seen.push(k.to_string());
        specs.push(Spec {
            name: format!("warning {k}"),
            origin: format!("jsgen:seed=20261006:index={}", idx - 1),
            src,
            profile: "simple".into(),
            check: Check::Warning(format!("[{k}]")),
        });
    }
    let mut d2 = 0;
    for f in mutate::visible_d2_inputs(&root, 3 * 1024) {
        if d2 == 4 {
            break;
        }
        let src = std::fs::read_to_string(&f).unwrap_or_default();
        if src.len() < 800 || o.borrow_mut().parse(&src).is_none() {
            continue;
        }
        let Some(out) = o.borrow_mut().compile(&src, "simple") else {
            continue;
        };
        let text = out
            .files
            .values()
            .map(|v| String::from_utf8_lossy(v))
            .collect::<String>();
        let Some(w) = longest_word(&text) else {
            continue;
        };
        if out.exit_code != 0 {
            continue;
        }
        d2 += 1;
        specs.push(Spec {
            name: format!("d2 output contains {w:?}"),
            origin: f
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            src,
            profile: "simple".into(),
            check: Check::Contains(w),
        });
    }
    // The real divergence: a visible Java crash case.
    let cases = std::fs::read_to_string(root.join("corpus/d2/cases.jsonl")).unwrap();
    let case = cases
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|c| c["id"] == crash_case)
        .expect("crash case in cases.jsonl");
    let input = root.join(case["inputs"][0].as_str().unwrap());
    let src = std::fs::read_to_string(&input).unwrap();
    let out = o
        .borrow_mut()
        .compile(&src, crash_profile)
        .expect("crash compile");
    let (class, frame) = crash_signature(&out).expect("the crash case does not crash");
    specs.push(Spec {
        name: format!("java crash {class} at {frame}"),
        origin: format!("{crash_case} / {crash_profile} (corpus/d2/JAVA_FAILURES.md)"),
        src,
        profile: crash_profile.into(),
        check: Check::Crash { class, frame },
    });
    specs
}

fn minimize_spec(o: &RefCell<Oracle>, s: &Spec, cfg: &Config) -> Value {
    let t0 = Instant::now();
    let mut pred = |cand: &str| -> bool {
        o.borrow_mut()
            .compile(cand, &s.profile)
            .is_some_and(|out| holds(&out, &s.check))
    };
    let original_holds = pred(&s.src);
    let (min, st) = reduce_with(&s.src, &mut Trees(o), &mut pred as &mut dyn Predicate, cfg);
    let final_parses = o.borrow_mut().parse(&min).is_some();
    let final_holds = pred(&min);
    json!({
        "name": s.name, "origin": s.origin, "profile": s.profile,
        "original_holds": original_holds, "before": s.src.len(), "after": min.len(),
        "final_parses": final_parses, "final_holds": final_holds,
        "tests": st.tests, "parses": st.parses, "invalid": st.invalid,
        "ast_steps": st.ast_steps, "replace_steps": st.replace_steps,
        "comment_steps": st.comment_steps, "line_steps": st.line_steps, "rounds": st.rounds,
        "budget_exhausted": st.budget_exhausted, "deadline_hit": st.deadline_hit,
        "wall_s": t0.elapsed().as_secs_f64(), "minimized": min,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str, d: &str| -> String {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1).cloned())
            .unwrap_or_else(|| d.to_string())
    };
    let workers: usize = get("--workers", "4").parse().unwrap();
    let budget: usize = get("--budget", "300").parse().unwrap();
    let deadline: u64 = get("--deadline-s", "150").parse().unwrap();
    let crash_budget: usize = get("--crash-budget", "800").parse().unwrap();
    let crash_case = get(
        "--crash-case",
        "closure-library-file-structs.circularbuffer",
    );
    let crash_profile = get("--crash-profile", "advanced");
    let root = repo_root();
    let out = root.join(get(
        "--out",
        "build/fuzz/mutate-minimize/minimize-selftest.json",
    ));
    let work = root.join("build/fuzz/mutate-minimize/selftest-work");
    let t0 = Instant::now();
    let o0 = RefCell::new(Oracle::start(work.join("specs")));
    let specs = Arc::new(build_specs(&o0, &crash_case, &crash_profile));
    drop(o0);
    eprintln!(
        "{} predicates built in {:.1}s",
        specs.len(),
        t0.elapsed().as_secs_f64()
    );
    // Jobs: every spec, plus determinism re-runs of spec 0 and spec 12.
    let mut jobs: Vec<usize> = (0..specs.len()).collect();
    jobs.push(0);
    jobs.push(12.min(specs.len() - 1));
    // Start the expensive crash case first.
    jobs.rotate_right(3);
    let jobs = Arc::new(jobs);
    let next = Arc::new(AtomicUsize::new(0));
    let rows: Arc<Mutex<Vec<(usize, Value)>>> = Arc::default();
    let mut hs = vec![];
    for w in 0..workers {
        let (specs, jobs, next, rows, work) = (
            specs.clone(),
            jobs.clone(),
            next.clone(),
            rows.clone(),
            work.clone(),
        );
        hs.push(std::thread::spawn(move || {
            let o = RefCell::new(Oracle::start(work.join(format!("w{w}"))));
            loop {
                let j = next.fetch_add(1, Ordering::SeqCst);
                if j >= jobs.len() {
                    return;
                }
                let s = &specs[jobs[j]];
                let crash = matches!(s.check, Check::Crash { .. });
                let cfg = Config {
                    deadline: Some(Duration::from_secs(if crash {
                        deadline * 4
                    } else {
                        deadline
                    })),
                    ..Config::calls(if crash { crash_budget } else { budget })
                };
                let mut r = minimize_spec(&o, s, &cfg);
                r["spec"] = json!(jobs[j]);
                eprintln!(
                    "[{}] {}: {} -> {} bytes, {} tests, holds={} ({:.0}s)",
                    jobs[j],
                    s.name,
                    r["before"],
                    r["after"],
                    r["tests"],
                    r["final_holds"],
                    r["wall_s"].as_f64().unwrap_or(0.0)
                );
                rows.lock().unwrap().push((j, r));
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    let mut rows = rows.lock().unwrap().clone();
    rows.sort_by_key(|r| r.0);
    let rows: Vec<Value> = rows.into_iter().map(|r| r.1).collect();
    // Determinism: compare the re-runs with the first runs.
    let mut det = vec![];
    for sid in [0usize, 12.min(specs.len() - 1)] {
        let runs: Vec<&Value> = rows.iter().filter(|r| r["spec"] == sid).collect();
        if runs.len() == 2 {
            det.push(
                json!({"spec": sid, "same_output": runs[0]["minimized"] == runs[1]["minimized"],
                "deadline_hit": runs.iter().any(|r| r["deadline_hit"] == true)}),
            );
        }
    }
    let synthetic: Vec<&Value> = rows
        .iter()
        .filter(|r| !r["name"].as_str().unwrap_or("").starts_with("java crash"))
        .collect();
    let report = json!({
        "predicates": specs.len(),
        "synthetic_rows": synthetic.len(),
        "synthetic_all_hold": synthetic.iter().all(|r| r["final_holds"] == true && r["final_parses"] == true),
        "synthetic_bytes_before": synthetic.iter().map(|r| r["before"].as_u64().unwrap()).sum::<u64>(),
        "synthetic_bytes_after": synthetic.iter().map(|r| r["after"].as_u64().unwrap()).sum::<u64>(),
        "determinism": det,
        "config": {"budget": budget, "deadline_s": deadline, "crash_budget": crash_budget, "crash_deadline_s": deadline * 4},
        "rows": rows, "wall_s": t0.elapsed().as_secs_f64(),
    });
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("report {}", out.display());
}
