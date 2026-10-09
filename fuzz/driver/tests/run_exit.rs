//! `fuzz-driver run` argument validation and exit status, end to end against a fake repository
//! (`CLOSURE_RS_ROOT`): a fake oracle "JVM" and a fake `gates/lib/fuzz_args.py`, both small
//! Python scripts, plus the real `scope/flags.txt` and `corpus/d2/profiles.json`.

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// The fake oracle server: the ready line `fuzz_oracle::Server::start` accepts, then one answer
/// per request. `parse_dump` accepts every program, unless the file `reject` exists next to
/// the script's root, in which case every program has a parse error.
const FAKE_JAVA: &str = r#"#!/usr/bin/env python3
import json, os, sys
root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
reject = os.path.exists(os.path.join(root, "reject"))
env = {"user.language": "en", "user.country": None, "user.variant": None,
       "locale.default": "en", "locale.format": "en", "native.encoding": "UTF-8",
       "stdout.encoding": "UTF-8", "stderr.encoding": "UTF-8", "sun.jnu.encoding": "UTF-8",
       "file.encoding": "UTF-8", "timezone": "UTC"}
print(json.dumps({"ready": True, "env": env}), flush=True)
for line in sys.stdin:
    q = json.loads(line)
    if q.get("op") == "parse_dump":
        errs = [{"key": "JSC_PARSE_ERROR", "lineno": 1, "charno": 0, "description": "x"}] if reject else []
        ans = {"id": q.get("id"), "ok": True, "errors": errs}
    else:
        ans = {"id": q.get("id"), "ok": False, "error": "unexpected op"}
    print(json.dumps(ans), flush=True)
"#;

/// The fake argv helper: the real profile lists, and the error case_args raises for an absolute
/// out_dir for every argv request (the systematic failure the driver must stop on).
const FAKE_FUZZ_ARGS: &str = r#"#!/usr/bin/env python3
import json, sys
profiles = json.load(open("corpus/d2/profiles.json"))["profiles"]
for line in sys.stdin:
    q = json.loads(line)
    if q.get("op") == "profiles":
        ans = {"profiles": list(profiles),
               "multi_input_only": [k for k, v in profiles.items() if v.get("applies_to") == "multi_input"]}
    else:
        ans = {"error": "CaseProfileError: out_dir must be repo-relative"}
    print(json.dumps(ans), flush=True)
"#;

fn real_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn write_exec(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A fresh fake repository root for test `name`.
fn fake_root(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("fuzz-driver-run-{}-{name}", std::process::id()))
        .join("repo");
    let _ = std::fs::remove_dir_all(d.parent().unwrap());
    for f in ["scope/flags.txt", "corpus/d2/profiles.json"] {
        std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
        std::fs::copy(real_root().join(f), d.join(f)).unwrap();
    }
    write_exec(&d.join("tools/jdk-21/bin/java"), FAKE_JAVA);
    write_exec(&d.join("gates/lib/fuzz_args.py"), FAKE_FUZZ_ARGS);
    write_exec(&d.join("fake-closure-rs"), "#!/bin/sh\nexit 0\n");
    d
}

fn driver(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fuzz-driver"))
        .args(args)
        .env("CLOSURE_RS_ROOT", root)
        .output()
        .unwrap()
}

fn run_args<'a>(root: &'a Path, extra: &[&'a str]) -> Vec<String> {
    let mut v: Vec<String> = [
        "run",
        "--seed",
        "1",
        "--servers",
        "1",
        "--source",
        "gen",
        "--engine-b",
        "rust",
        "--rust-bin",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.push(root.join("fake-closure-rs").display().to_string());
    v.push("--report".into());
    v.push(root.join("build/report.json").display().to_string());
    v.extend(extra.iter().map(|s| s.to_string()));
    v
}

fn report(root: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root.join("build/report.json")).unwrap()).unwrap()
}

#[test]
fn work_outside_the_repository_is_a_usage_error() {
    let root = fake_root("outside");
    let outside = root.parent().unwrap().join("work");
    for w in [outside.display().to_string(), "../work".to_string()] {
        let args = run_args(&root, &["--count", "5", "--work", &w]);
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let t0 = Instant::now();
        let out = driver(&root, &args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{w}: {err}");
        assert!(err.contains("is outside the repository"), "{w}: {err}");
        assert!(t0.elapsed() < Duration::from_secs(30));
        // Rejected before anything ran: no work directory, no report.
        assert!(!outside.exists());
        assert!(!root.join("build/report.json").exists());
    }
    let _ = std::fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn systematic_oracle_errors_stop_the_run_with_exit_4() {
    let root = fake_root("oracle-failure");
    // An absolute --work inside the repository is accepted (and made repo-relative for
    // case_args); here the fake case_args refuses every argv anyway.
    let work = root.join("build/work").display().to_string();
    let args = run_args(&root, &["--duration", "600", "--work", &work]);
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let t0 = Instant::now();
    let out = driver(&root, &args);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(4), "{err}");
    assert!(
        t0.elapsed() < Duration::from_secs(300),
        "did not stop early"
    );
    assert!(
        err.contains("stopped on systematic oracle failure"),
        "{err}"
    );
    assert!(err.contains("out_dir must be repo-relative"), "{err}");
    let r = report(&root);
    assert_eq!(r["status"], "oracle_failure");
    assert_eq!(r["compared"], 0);
    assert!(r["abort_reason"].as_str().unwrap().contains("first: "));
    let errors = r["oracle_errors"].as_u64().unwrap();
    assert!((50..60).contains(&errors), "{errors}");
    let _ = std::fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn a_run_that_compares_nothing_exits_5() {
    let root = fake_root("nothing");
    std::fs::write(root.join("reject"), "").unwrap();
    let args = run_args(&root, &["--count", "20"]);
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = driver(&root, &args);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(5), "{err}");
    assert!(err.contains("the run compared no program"), "{err}");
    let r = report(&root);
    assert_eq!(r["status"], "nothing_compared");
    assert_eq!(r["programs"], 20);
    assert_eq!(r["parse_rejected"], 20);
    assert_eq!(r["oracle_errors"], 0);
    let _ = std::fs::remove_dir_all(root.parent().unwrap());
}
