//! Rust client for the Java oracle server (oracle/PROTOCOL.md).
//!
//! Every server runs in the golden environment (D-010): the environment is cleared and set to
//! exactly `oracle_client.golden_env()`, and the ready line's `env` is checked against
//! `GOLDEN_JVM_ENV`; a mismatch refuses the server.

#![forbid(unsafe_code)]

use base64::Engine;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

/// Repository root: `$CLOSURE_RS_ROOT`, else two levels above this crate.
pub fn repo_root() -> PathBuf {
    if let Ok(r) = std::env::var("CLOSURE_RS_ROOT") {
        return PathBuf::from(r);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// `oracle_client.golden_env()`.
pub fn golden_env() -> Vec<(String, String)> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("HOME".into(), home),
        ("LANG".into(), "C.UTF-8".into()),
        ("LC_ALL".into(), "C.UTF-8".into()),
        ("TZ".into(), "UTC".into()),
    ]
}

/// `oracle_client.GOLDEN_JVM_ENV` (None = JSON null).
pub const GOLDEN_JVM_ENV: &[(&str, Option<&str>)] = &[
    ("user.language", Some("en")),
    ("user.country", None),
    ("user.variant", None),
    ("locale.default", Some("en")),
    ("locale.format", Some("en")),
    ("native.encoding", Some("UTF-8")),
    ("stdout.encoding", Some("UTF-8")),
    ("stderr.encoding", Some("UTF-8")),
    ("sun.jnu.encoding", Some("UTF-8")),
    ("file.encoding", Some("UTF-8")),
    ("timezone", Some("UTC")),
];

pub fn check_jvm_env(env: &Value) -> Result<(), String> {
    let obj = env.as_object().ok_or("ready line has no env object")?;
    let mut bad = Vec::new();
    for (k, want) in GOLDEN_JVM_ENV {
        let got = obj.get(*k).and_then(|v| v.as_str());
        if got != *want {
            bad.push(format!("{k}: got {got:?} want {want:?}"));
        }
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("oracle JVM env differs: {}", bad.join(", ")))
    }
}

/// JVM flags for pool servers (as gates/lib/gate01.py server_jvm_flags, heap from caller).
pub fn server_jvm_flags(xmx: &str) -> Vec<String> {
    [
        &format!("-Xmx{xmx}"),
        "-Xms128m",
        "-XX:+UseSerialGC",
        "-XX:MinHeapFreeRatio=10",
        "-XX:MaxHeapFreeRatio=30",
        "-XX:-UsePerfData",
        "-XX:+ExitOnOutOfMemoryError",
        "-XX:+DisplayVMOutputToStderr",
        "-Xlog:disable",
        "-Xlog:all=off",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub struct Server {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    pub ready: Value,
    n: u64,
}

impl Server {
    pub fn start(xmx: &str) -> Result<Server, String> {
        let root = repo_root();
        let java = root.join("tools/jdk-21/bin/java");
        let cp = format!(
            "{}:{}",
            root.join("build/oracle/oracle.jar").display(),
            root.join("build/reference/closure-compiler.jar").display()
        );
        let mut cmd = Command::new(java);
        cmd.args(server_jvm_flags(xmx))
            .args([
                "-cp",
                &cp,
                "closurers.oracle.Main",
                "server",
                "--isolate=none",
            ])
            .current_dir(&root)
            .env_clear()
            .envs(golden_env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|e| format!("spawn oracle: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        let hello = rx
            .recv_timeout(Duration::from_secs(120))
            .map_err(|e| format!("no ready line: {e}"))?;
        let ready: Value =
            serde_json::from_str(&hello).map_err(|e| format!("bad ready line: {e}"))?;
        let mut s = Server {
            child,
            stdin,
            rx,
            ready: ready.clone(),
            n: 0,
        };
        if ready.get("ready") != Some(&Value::Bool(true)) {
            s.kill();
            return Err(format!("not ready: {hello}"));
        }
        if let Err(e) = check_jvm_env(ready.get("env").unwrap_or(&Value::Null)) {
            s.kill();
            return Err(e);
        }
        Ok(s)
    }

    /// One request; on timeout the server is killed and `Err` returned (caller restarts).
    pub fn request(&mut self, mut req: Value, timeout: Duration) -> Result<Value, String> {
        self.n += 1;
        if let Some(o) = req.as_object_mut() {
            o.entry("id").or_insert(json!(self.n));
        }
        let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        self.stdin.flush().map_err(|e| format!("flush: {e}"))?;
        match self.rx.recv_timeout(timeout) {
            Ok(resp) => parse_deep(&resp).map_err(|e| format!("bad response: {e}")),
            Err(RecvTimeoutError::Timeout) => {
                self.kill();
                Err("timeout".into())
            }
            Err(RecvTimeoutError::Disconnected) => Err("oracle server died".into()),
        }
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Parse one response line without serde_json's 128-level recursion limit: a `parse_dump`
/// of a deeply nested input (minified D2 code, mutants) nests far deeper than that. The
/// recursion runs on the caller's stack, so callers that may receive deep ASTs run on
/// threads with a large stack (see `DEEP_STACK`).
pub fn parse_deep(s: &str) -> Result<Value, serde_json::Error> {
    use serde::Deserialize;
    let mut de = serde_json::Deserializer::from_str(s);
    de.disable_recursion_limit();
    let v = Value::deserialize(&mut de)?;
    de.end()?;
    Ok(v)
}

/// Stack size for threads that parse, walk or drop deep `parse_dump` values (256 MiB of
/// address space; only touched pages are committed).
pub const DEEP_STACK: usize = 256 << 20;

pub fn b64(v: Option<&Value>) -> Vec<u8> {
    v.and_then(|x| x.as_str())
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
        .unwrap_or_default()
}

/// The observable result of one compile (docs/PORTING.md §1: output, diagnostics, exit code).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub exit_code: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Outcome {
    pub fn from_compile(resp: &Value) -> Result<Outcome, String> {
        if resp.get("ok") != Some(&Value::Bool(true)) {
            let e = resp.get("error").and_then(|e| e.as_str()).unwrap_or("?");
            return Err(format!(
                "oracle failure: {}",
                e.lines().next().unwrap_or("")
            ));
        }
        let mut files = BTreeMap::new();
        if let Some(m) = resp.get("output_files").and_then(|m| m.as_object()) {
            for (k, v) in m {
                files.insert(k.clone(), b64(Some(v)));
            }
        }
        Ok(Outcome {
            exit_code: resp
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(-999),
            stdout: b64(resp.get("stdout_b64")),
            stderr: b64(resp.get("stderr_b64")),
            files,
        })
    }

    /// `None` when identical, else a short description of the first difference.
    pub fn diff(&self, other: &Outcome) -> Option<String> {
        if self.exit_code != other.exit_code {
            return Some(format!(
                "exit_code {} vs {}",
                self.exit_code, other.exit_code
            ));
        }
        if self.stdout != other.stdout {
            return Some("stdout differs".into());
        }
        if self.stderr != other.stderr {
            return Some("stderr differs".into());
        }
        let ka: Vec<_> = self.files.keys().collect();
        let kb: Vec<_> = other.files.keys().collect();
        if ka != kb {
            return Some(format!("output file set differs: {ka:?} vs {kb:?}"));
        }
        for (k, v) in &self.files {
            if other.files.get(k) != Some(v) {
                return Some(format!("output file {k} differs"));
            }
        }
        None
    }

    /// Everything concatenated (for "output contains X" predicates).
    pub fn all_text(&self) -> String {
        let mut s = String::from_utf8_lossy(&self.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&self.stderr));
        for v in self.files.values() {
            s.push_str(&String::from_utf8_lossy(v));
        }
        s
    }
}

/// Persistent `python3 gates/lib/fuzz_args.py` process: argv for a synthetic case under a
/// D2 profile, computed by gates/lib/case_args.py (the single source of argv truth).
pub struct ArgsHelper {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<std::process::ChildStdout>,
}

impl ArgsHelper {
    pub fn start() -> Result<ArgsHelper, String> {
        let root = repo_root();
        let mut child = Command::new("python3")
            .arg(root.join("gates/lib/fuzz_args.py"))
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn fuzz_args.py: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let out = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        Ok(ArgsHelper { child, stdin, out })
    }

    /// `query` = {"id", "inputs": [repo-relative paths], "profile", "out_dir"} or
    /// {"op": "profiles"}. Returns the helper's JSON answer.
    pub fn ask(&mut self, query: &Value) -> Result<Value, String> {
        let mut line = serde_json::to_string(query).map_err(|e| e.to_string())?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut resp = String::new();
        self.out.read_line(&mut resp).map_err(|e| e.to_string())?;
        let v: Value =
            serde_json::from_str(&resp).map_err(|e| format!("fuzz_args: {e}: {resp}"))?;
        if let Some(e) = v.get("error") {
            return Err(format!("fuzz_args: {e}"));
        }
        Ok(v)
    }
}

impl Drop for ArgsHelper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
