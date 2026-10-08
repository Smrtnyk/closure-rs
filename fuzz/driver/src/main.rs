//! fuzz-driver: differential fuzzing loop (docs/PORTING.md §4.6).
//!
//! Subcommands:
//!   parse-rate  --seed S --count N [--servers K]
//!       Generate N single-file programs (`jsgen::Config::default()`, closure on) and parse
//!       each with the Java oracle's `parse_dump` (CLI default options, i.e. language_in
//!       STABLE_IN). Accepted = parse_dump `errors` is empty.
//!   export      --seed S --count N --out DIR [--servers K] [--source gen|mutate|mixed]
//!       Write programs 0..N of the run (seed, source) exactly as `run` would draw them, with
//!       the parse-filter verdict and the profile `run` would pick, to DIR/p<i>/<file> and
//!       DIR/manifest.jsonl (+ DIR/summary.json). Gate 0.3 (a) and (b) use this.
//!   run         --seed S (--count N | --duration SECS) [--servers K]
//!               [--engine-b java|java-perturbed|rust] [--rust-bin PATH]
//!               [--source gen|mutate|mixed]
//!               [--findings DIR] [--budget B] [--work DIR] [--report FILE] [--wide]
//!       Generate/mutate programs, drop those the Java parser rejects, compile each under a
//!       random applicable D2 profile drawn with the D-016 weights (ADVANCED family 65%, ws 3%,
//!       other 32%; `PROFILE_WEIGHTS_PCT`), plus random in-scope option flags for one program in
//!       three (`OPTION_POOL`, checked against scope/flags.txt at start-up), with argv from
//!       gates/lib/case_args.py via fuzz_args.py, on
//!       engine A (Java oracle) and engine B, compare exit code/stdout/stderr/output files,
//!       minimize every mismatch (AST ddmin over parse_dump ranges, then lines; file by file
//!       for multi-file programs) and file it as DIR/<id>.md (default fuzz/findings). Engine
//!       B "java" is a second, independent oracle JVM (proves the comparison path);
//!       "java-perturbed" is Java with a synthetic bug (output altered when the input
//!       contains `switch`) to prove the minimize-and-file path. "rust" runs the closure-rs
//!       CLI (`--rust-bin`, default `$CARGO_TARGET_DIR/release/closure-rs`, else
//!       `target/release/closure-rs`) with the same argv as the oracle, from the repository root,
//!       in the golden environment with stdin /dev/null (as gates/d2_rust.py does); its outcome
//!       is its exit status, stdout, stderr and every file under the run's out_dir.
//!       With --duration, workers stop drawing programs after SECS seconds. --wide adds two
//!       draws on top of the D-016 ones: `--jscomp_off|--jscomp_warning=checkTypes` (one program
//!       in four, never ws) and, for single-file programs under simple/advanced/advanced_strict/
//!       pretty/sourcemap, `--language_out=ECMASCRIPT5|ECMASCRIPT_2015` (one in three; the
//!       program is generated in low_target mode).
//!       A worker thread that panics is a harness crash: it is counted in the report
//!       (`harness_crashes`) and the process exits 3.
//!   minimize-selftest --seed S [--count N]
//!       Synthetic predicate: "the program still parses and Java's SIMPLE output still contains
//!       NEEDLE". Reports size before/after and predicate calls.
//!
//! Program sources (`--source`), drawn per program index from `Rng::fork(seed ^ MIX, i)`:
//!   mixed:  30% lang (jsgen, closure off), 25% closure (jsgen single file, closure on),
//!           15% multi (jsgen multi-file Closure program: goog.module/goog.provide/ES
//!           modules/CommonJS, 2..6 files, chunk profiles apply), 5% sloppy (jsgen sloppy
//!           dialect, closure off, `--strict_mode_input=false` for parse and compile),
//!           25% mutate (mutator over a visible single-input D2 file, whole pool as donors).
//!   gen:    the four jsgen categories in the same 30:25:15:5 ratio.
//!   mutate: mutate only.
//!
//! JVM heap: each worker owns two servers at -Xmx1536m (engine A and B), or one with engine
//! B `rust` (which runs the binary as a child process).

#![forbid(unsafe_code)]

use fuzz_oracle::{ArgsHelper, Outcome, Server, repo_root};
use jsgen::lang::dialect::{self, Dialect};
use jsgen::rng::Rng;
use minimize::{Predicate, Tree, TreeSource};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const XMX: &str = "1536m";
const JAVA_CRASH: &str = "java_crash: ";
const MIX_SALT: u64 = 0x6d69_7865_6420;
const PROFILE_SALT: u64 = 0x7072_6f66;
const MUTATE_SALT: u64 = 0x6d75_7461_7465;
const OPTION_SALT: u64 = 0x6f70_7469_6f6e;
const WIDE_SALT: u64 = 0x7769_6465;

/// `--wide`: type checking forced off or on, one program in four (never under ws).
const WIDE_TYPECHECK: &[&str] = &["--jscomp_off=checkTypes", "--jscomp_warning=checkTypes"];
/// `--wide`: a lower `--language_out` for single-file programs under the profiles that do not
/// lock it, one program in three; the program is generated in `low_target` mode.
const WIDE_LANG_OUT: &[&str] = &[
    "--language_out=ECMASCRIPT5",
    "--language_out=ECMASCRIPT_2015",
];
const WIDE_LANG_OUT_PROFILES: &[&str] = &[
    "simple",
    "advanced",
    "advanced_strict",
    "pretty",
    "sourcemap",
];

/// `--wide` language_out override for single-file program `i` under `profile` (deterministic;
/// depends only on seed, index and profile, so generation can use it).
fn wide_lang_out(seed: u64, i: u64, profile: &str) -> Option<&'static str> {
    if !WIDE_LANG_OUT_PROFILES.contains(&profile) {
        return None;
    }
    let mut r = Rng::fork(seed ^ WIDE_SALT, i);
    if !r.chance(1, 3) {
        return None;
    }
    Some(WIDE_LANG_OUT[r.below(WIDE_LANG_OUT.len() as u64) as usize])
}

/// `--wide` type-checking flag for program `i` under `profile` (deterministic).
fn wide_typecheck(seed: u64, i: u64, profile: &str) -> Option<&'static str> {
    if profile == "ws" {
        return None;
    }
    let mut r = Rng::fork(seed ^ WIDE_SALT ^ 0x7463, i);
    if !r.chance(1, 4) {
        return None;
    }
    Some(WIDE_TYPECHECK[r.below(WIDE_TYPECHECK.len() as u64) as usize])
}

/// When an option-flag group may be drawn (on top of "never under ws" and the profile's
/// `locked` keys, both checked in [`option_flags`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum When {
    /// Every profile except ws.
    Any,
    /// ADVANCED-family profiles only.
    Advanced,
    /// SIMPLE-level profiles only (`--renaming=false` is a usage error under ADVANCED).
    NotAdvanced,
    /// Chunk profiles only.
    Chunks,
    /// Single-output profiles only (wrappers whose chunk form differs).
    SingleOutput,
    /// The sourcemap profile only (source-map content flags).
    Sourcemap,
    /// Programs whose first file is jsgen's `base.js` (goog.provide/goog.module programs).
    Goog,
    /// Multi-file programs.
    Multi,
}

/// In-scope CommandLineRunner flags drawn on top of the D2 profile (docs/PORTING.md §4.6 "random
/// option profile"; D-016 item 4: the pool comes from `scope/flags.txt`). Each entry is a group
/// of mutually exclusive alternatives with the condition under which it may be drawn. Every key
/// here must be `in` in `scope/flags.txt`, and every in-scope flag is either here or in
/// [`NOT_DRAWN`] with the reason the driver does not draw it ([`check_option_pool`]; checked at
/// start-up and by the unit tests). Flags that change argv structure or the file layout (inputs,
/// output paths, extra input or output files) are never drawn: case_args owns that layout.
const OPTION_POOL: &[(&[&str], When)] = &[
    (
        &["--isolate_polyfills", "--rewrite_polyfills=false"],
        When::Any,
    ),
    (&["--rename_prefix_namespace=_g"], When::Any),
    (&["--assume_function_wrapper"], When::Any),
    (&["--angular_pass"], When::Any),
    (&["--emit_use_strict=false", "--emit_use_strict"], When::Any),
    (&["--inject_libraries=false"], When::Any),
    (&["--dynamic_import_alias=__imp"], When::Any),
    (
        &["--warning_level=VERBOSE", "--warning_level=QUIET"],
        When::Any,
    ),
    (
        &[
            "--formatting=PRINT_INPUT_DELIMITER",
            "--formatting=SINGLE_QUOTES",
        ],
        When::Any,
    ),
    (&["--rename_variable_prefix=v_"], When::Any),
    (&["--third_party"], When::Any),
    (&["--charset=UTF-8"], When::Any),
    (&["--strict_mode_input=false"], When::Any),
    (&["--preserve_type_annotations"], When::Any),
    (
        &["--error_format=JSON", "--summary_detail_level=3"],
        When::Any,
    ),
    (&["--use_types_for_optimization=false"], When::Advanced),
    (&["--debug"], When::Advanced),
    (
        &[
            "--assume_static_inheritance_is_not_used=false",
            "--assume_no_prototype_method_enumeration",
        ],
        When::Advanced,
    ),
    (&["--renaming=false"], When::NotAdvanced),
    (&["--chunk_output_type=ES_MODULES"], When::Chunks),
    (
        &["--chunk_wrapper=c1:(function(){%s}).call(this);"],
        When::Chunks,
    ),
    (
        &[
            "--isolation_mode=IIFE",
            "--output_wrapper=(function(){%output%}).call(this);",
        ],
        When::SingleOutput,
    ),
    (&["--source_map_include_content"], When::Sourcemap),
    // goog programs: base.js defines goog.exportSymbol for the @export annotations.
    (
        &[
            "--generate_exports",
            "--export_local_property_definitions=false",
        ],
        When::Goog,
    ),
    (
        &[
            "--define=goog.DEBUG=false",
            "--process_closure_primitives=false",
        ],
        When::Goog,
    ),
    (&["--dependency_mode=SORT_ONLY"], When::Multi),
];

/// Pairs that CommandLineRunner rejects together (FlagUsageException): never drawn together.
const OPTION_CONFLICTS: &[(&str, &str)] = &[
    (
        "--chunk_output_type=ES_MODULES",
        "--rename_prefix_namespace",
    ),
    ("--chunk_output_type=ES_MODULES", "--emit_use_strict"),
];

/// In-scope flags (`scope/flags.txt`) the driver does not draw, with the reason.
const NOT_DRAWN: &[(&str, &str)] = &[
    // argv / file layout owned by case_args (RUNNER_OWNED) or the profile.
    ("--js", "runner-owned (case_args): the input layout"),
    ("--externs", "runner-owned (case_args): the input layout"),
    (
        "--js_output_file",
        "runner-owned (case_args): the output layout",
    ),
    ("--chunk", "runner-owned (case_args): the chunk layout"),
    (
        "--chunk_output_path_prefix",
        "runner-owned (case_args): the output layout",
    ),
    (
        "--create_source_map",
        "runner-owned (case_args): set by the sourcemap profile",
    ),
    (
        "--flagfile",
        "runner-owned (case_args): argv indirection through a file",
    ),
    (
        "--json_streams",
        "runner-owned (case_args): inputs and outputs move to stdin/stdout JSON",
    ),
    (
        "--compilation_level",
        "profile-owned (locked in every D2 profile)",
    ),
    (
        "--language_out",
        "profile-owned: jsgen's low_target mode is keyed on the profile's language_out (--wide draws ES5/ES2015 for single-file programs: WIDE_LANG_OUT)",
    ),
    (
        "--browser_featureset_year",
        "sets the output feature set, which the profile owns (language_out)",
    ),
    (
        "--language_in",
        "the parse filter accepts programs under the default language_in; a lower one rejects generated syntax",
    ),
    // Extra input files or path-valued options the driver does not lay out.
    ("--jszip", "needs a zip input file"),
    ("--output_wrapper_file", "needs a wrapper input file"),
    ("--source_map_input", "needs input source-map files"),
    (
        "--source_map_location_mapping",
        "path-prefix mapping over the run's directory layout",
    ),
    ("--warnings_allowlist_file", "needs an allowlist input file"),
    (
        "--conformance_configs",
        "needs conformance config input files",
    ),
    ("--translations_file", "needs a translations input file"),
    (
        "--translations_project",
        "only meaningful with --translations_file",
    ),
    (
        "--variable_map_input_file",
        "needs a variable map input file",
    ),
    (
        "--property_map_input_file",
        "needs a property map input file",
    ),
    (
        "--filename_to_restore_from",
        "needs a saved compilation state file",
    ),
    (
        "--js_module_root",
        "path-valued: module roots over the run's directory layout",
    ),
    ("--package_json_entry_names", "needs package.json inputs"),
    (
        "--browser_resolver_prefix_replacements",
        "path-prefix mapping over the run's directory layout",
    ),
    (
        "--hide_warnings_for",
        "path-prefix filter over the run's directory layout",
    ),
    (
        "--entry_point",
        "names an input file or namespace of the program",
    ),
    // Extra output files: their paths must follow the run's out_dir, and case_args expands
    // {out_dir} only in profile flags (not drawn yet).
    (
        "--variable_renaming_report",
        "writes an extra output file at a path case_args does not lay out",
    ),
    (
        "--property_renaming_report",
        "writes an extra output file at a path case_args does not lay out",
    ),
    (
        "--create_renaming_reports",
        "writes extra output files next to the output",
    ),
    (
        "--output_manifest",
        "writes an extra output file at a path case_args does not lay out",
    ),
    (
        "--output_chunk_dependencies",
        "writes an extra output file at a path case_args does not lay out",
    ),
    ("--filename_to_save_to", "writes a compilation state file"),
    (
        "--segment_of_compilation_to_run",
        "needs saved compilation state (--filename_to_save_to)",
    ),
    (
        "--checks_only",
        "writes no output file (changes the output layout)",
    ),
    (
        "--incremental_check_mode",
        "GENERATE_IJS/CHECK_IJS change the output to .i.js files (not drawn yet)",
    ),
    // No compile, or output that is not reproducible between two JVMs.
    ("--help", "prints usage and exits without compiling"),
    (
        "--help_markdown",
        "prints usage and exits without compiling",
    ),
    (
        "--version",
        "prints the version and exits without compiling",
    ),
    (
        "--print_tree",
        "prints the parse tree and exits before the passes",
    ),
    (
        "--print_tree_json",
        "prints the parse tree and exits before the passes",
    ),
    (
        "--print_ast",
        "prints the AST as dot and exits before the passes",
    ),
    (
        "--tracer_mode",
        "prints timings (not reproducible); the gate's entry measure uses it",
    ),
    (
        "--logging_level",
        "java.util.logging records carry timestamps (not reproducible)",
    ),
    (
        "--print_source_after_each_pass",
        "the gate's effect-measure instrument (compile_with_pass_dumps)",
    ),
    (
        "--jscomp_dev_mode",
        "very slow validity checks after every pass",
    ),
    (
        "--num_parallel_threads",
        "scheduling only; D6 covers thread-count determinism",
    ),
    // Values the generated programs cannot use well yet.
    (
        "--env",
        "CUSTOM drops the default externs (console, ...) the generated programs use",
    ),
    (
        "--allow_dynamic_import",
        "=false turns the generator's import() into an error",
    ),
    (
        "--source_map_format",
        "V3 is the only format (DEFAULT = V3)",
    ),
    (
        "--parse_inline_source_maps",
        "generated inputs carry no inline source maps",
    ),
    (
        "--apply_input_source_maps",
        "generated inputs carry no input source maps",
    ),
    (
        "--process_common_js_modules",
        "set by jsgen for its CommonJS multi-file programs (extra_flags)",
    ),
    (
        "--module_resolution",
        "module-resolution modes over package layouts (not drawn yet)",
    ),
    ("--force_inject_library", "library names (not drawn yet)"),
    (
        "--jscomp_error",
        "diagnostic-group levels: need a strict-aware generation mode (not drawn yet)",
    ),
    (
        "--jscomp_warning",
        "diagnostic-group levels: need a strict-aware generation mode (not drawn yet); --wide draws =checkTypes (WIDE_TYPECHECK)",
    ),
    (
        "--jscomp_off",
        "diagnostic-group levels: need a strict-aware generation mode (not drawn yet); --wide draws =checkTypes (WIDE_TYPECHECK)",
    ),
    (
        "--continue_after_errors",
        "passes run on ASTs with errors (not drawn yet)",
    ),
    (
        "--extra_annotation_name",
        "generated JSDoc uses no unknown annotations",
    ),
    (
        "--expected_diagnostics",
        "test-harness flag: needs per-program expected diagnostics",
    ),
];

fn flag_key(f: &str) -> &str {
    f.split('=').next().unwrap_or(f)
}

/// `(flag or alias) -> (scope, canonical flag)` from `scope/flags.txt`.
fn scope_table(text: &str) -> BTreeMap<String, (String, String)> {
    let mut m = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let entry = (cols[0].to_string(), cols[1].to_string());
        m.insert(cols[1].to_string(), entry.clone());
        if cols[2] != "-" {
            for a in cols[2].split(',') {
                m.insert(a.to_string(), entry.clone());
            }
        }
    }
    m
}

/// Problems with [`OPTION_POOL`] / [`NOT_DRAWN`] against `scope/flags.txt` (empty = consistent):
/// a drawn key that is unknown or out of scope, an in-scope flag that is neither drawn nor
/// listed with a reason, or a key in both tables.
fn check_option_pool(flags_txt: &str) -> Vec<String> {
    let t = scope_table(flags_txt);
    let mut errs = vec![];
    let mut drawn = std::collections::BTreeSet::new();
    for (grp, _) in OPTION_POOL {
        for f in *grp {
            let k = flag_key(f);
            match t.get(k) {
                None => errs.push(format!("drawn flag {k} is not in scope/flags.txt")),
                Some((s, _)) if s != "in" => {
                    errs.push(format!("drawn flag {k} is out of scope in scope/flags.txt"))
                }
                Some((_, canon)) => {
                    drawn.insert(canon.clone());
                }
            }
        }
    }
    let mut listed = std::collections::BTreeSet::new();
    for (k, _) in NOT_DRAWN {
        match t.get(*k) {
            None => errs.push(format!("NOT_DRAWN flag {k} is not in scope/flags.txt")),
            Some((_, canon)) => {
                if drawn.contains(canon) {
                    errs.push(format!("{k} is both drawn and in NOT_DRAWN"));
                }
                listed.insert(canon.clone());
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for (scope, canon) in t.values() {
        if scope == "in"
            && seen.insert(canon.clone())
            && !drawn.contains(canon)
            && !listed.contains(canon)
        {
            errs.push(format!(
                "in-scope flag {canon} is neither drawn nor listed in NOT_DRAWN"
            ));
        }
    }
    errs
}

/// Die unless the option pool agrees with `scope/flags.txt` (D-016 item 4).
fn require_option_pool() {
    let p = repo_root().join("scope/flags.txt");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| {
        die(&format!(
            "read {}: {e} (run python3 scope/gen_flags.py)",
            p.display()
        ))
    });
    let errs = check_option_pool(&text);
    if !errs.is_empty() {
        die(&format!(
            "option pool disagrees with scope/flags.txt: {}",
            errs.join("; ")
        ));
    }
}

/// The option pool, for reports.
fn option_pool_json() -> Value {
    json!({
        "source": "scope/flags.txt (in-scope flags; checked by check_option_pool)",
        "programs_with_flags": "1 in 3 (never ws), 1 or 2 flags",
        "groups": OPTION_POOL.iter().map(|(g, w)| json!({"alternatives": g, "when": format!("{w:?}")})).collect::<Vec<_>>(),
        "conflicts": OPTION_CONFLICTS,
        "not_drawn": NOT_DRAWN.iter().map(|(k, r)| json!({"flag": k, "reason": r})).collect::<Vec<_>>(),
    })
}

fn when_applies(w: When, profile: &str, prog: &Prog) -> bool {
    let adv = advanced_profiles().iter().any(|p| p == profile);
    let chunks = profile.starts_with("chunks");
    match w {
        When::Any => true,
        When::Advanced => adv,
        When::NotAdvanced => !adv,
        When::Chunks => chunks,
        When::SingleOutput => !chunks,
        When::Sourcemap => profile == "sourcemap",
        When::Goog => prog.files.first().is_some_and(|(n, _)| n == "base.js"),
        When::Multi => prog.files.len() >= 2,
    }
}

fn conflicts(a: &str, b: &str) -> bool {
    let m = |x: &str, pat: &str| x == pat || (!pat.contains('=') && flag_key(x) == pat);
    OPTION_CONFLICTS
        .iter()
        .any(|(p, q)| (m(a, p) && m(b, q)) || (m(b, p) && m(a, q)))
}

/// Random extra option flags for program `i` compiled under `profile` (deterministic). One
/// program in three gets one or two; never under WHITESPACE_ONLY, which runs no passes. A group
/// is skipped when the profile locks its key, when the program already sets the key (jsgen's
/// own extra flags) or when it conflicts with a flag already drawn.
fn option_flags(seed: u64, i: u64, profile: &str, prog: &Prog) -> Vec<String> {
    if profile == "ws" {
        return vec![];
    }
    let mut r = Rng::fork(seed ^ OPTION_SALT, i);
    if !r.chance(1, 3) {
        return vec![];
    }
    let locked = profile_locked(profile);
    let has = |k: &str| prog.extra_flags.iter().any(|f| flag_key(f) == k);
    let mut groups: Vec<&[&str]> = OPTION_POOL
        .iter()
        .filter(|(g, w)| {
            when_applies(*w, profile, prog)
                && g.iter()
                    .all(|f| !locked.iter().any(|l| l == flag_key(f)) && !has(flag_key(f)))
        })
        .map(|(g, _)| *g)
        .collect();
    let n = r.range(1, 2) as usize;
    let mut out: Vec<String> = vec![];
    for _ in 0..n {
        if groups.is_empty() {
            break;
        }
        let gi = r.below(groups.len() as u64) as usize;
        let grp = groups.remove(gi);
        let f = grp[r.below(grp.len() as u64) as usize];
        if out.iter().any(|o| conflicts(o, f)) {
            continue;
        }
        out.push(f.to_string());
    }
    out
}

/// D-009 drop rule: an internal compiler error or an uncaught exception (exit 254).
fn is_java_crash(o: &Outcome) -> bool {
    o.exit_code == 254 || String::from_utf8_lossy(&o.stderr).contains("INTERNAL COMPILER ERROR")
}
const TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone)]
struct Opts {
    cmd: String,
    seed: u64,
    count: u64,
    duration: Option<Duration>,
    servers: usize,
    engine_b: String,
    source: String,
    findings: PathBuf,
    budget: usize,
    out: Option<PathBuf>,
    work: Option<PathBuf>,
    report: Option<PathBuf>,
    /// Engine B `rust`: the closure-rs CLI binary (default [`default_rust_bin`]).
    rust_bin: Option<PathBuf>,
    /// `--wide`: also draw [`WIDE_TYPECHECK`] and [`WIDE_LANG_OUT`] (off by default, so the
    /// D-016 draws are unchanged without it).
    wide: bool,
}

fn parse_opts() -> Opts {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Opts {
        cmd: args.first().cloned().unwrap_or_default(),
        seed: 1,
        count: 10,
        duration: None,
        servers: 2,
        engine_b: "java".into(),
        source: "gen".into(),
        findings: repo_root().join("fuzz/findings"),
        budget: 400,
        out: None,
        work: None,
        report: None,
        rust_bin: None,
        wide: false,
    };
    let mut count_given = false;
    let abs = |v: &str| {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            p
        } else {
            repo_root().join(p)
        }
    };
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--wide" {
            o.wide = true;
            i += 1;
            continue;
        }
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--seed" => o.seed = v.parse().expect("--seed"),
            "--count" => {
                o.count = v.parse().expect("--count");
                count_given = true;
            }
            "--duration" => o.duration = Some(Duration::from_secs(v.parse().expect("--duration"))),
            "--servers" => o.servers = v.parse().expect("--servers"),
            "--engine-b" => o.engine_b = v,
            "--source" => o.source = v,
            "--findings" => o.findings = abs(&v),
            "--budget" => o.budget = v.parse().expect("--budget"),
            "--out" => o.out = Some(abs(&v)),
            "--work" => o.work = Some(abs(&v)),
            "--report" => o.report = Some(abs(&v)),
            "--rust-bin" => o.rust_bin = Some(abs(&v)),
            x => die(&format!("unknown argument {x}")),
        }
        i += 2;
    }
    if o.duration.is_some() && !count_given {
        o.count = u64::MAX;
    }
    if !["gen", "mutate", "mixed"].contains(&o.source.as_str()) {
        die(&format!("unknown --source {}", o.source));
    }
    if !ENGINES_B.contains(&o.engine_b.as_str()) {
        die(&format!("unknown --engine-b {}", o.engine_b));
    }
    if o.engine_b == "rust" {
        let bin = o.rust_bin.clone().unwrap_or_else(default_rust_bin);
        if !bin.is_file() {
            die(&format!(
                "--engine-b rust: no closure-rs binary at {} (cargo build --release -p closure-cli, or --rust-bin PATH)",
                bin.display()
            ));
        }
        o.rust_bin = Some(bin);
    }
    o
}

/// The engines `--engine-b` accepts.
const ENGINES_B: &[&str] = &["java", "java-perturbed", "rust"];

/// The release closure-rs CLI of this checkout: `$CARGO_TARGET_DIR/release/closure-rs`, else
/// `<repo>/target/release/closure-rs`.
fn default_rust_bin() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("target"));
    let target = if target.is_absolute() {
        target
    } else {
        repo_root().join(target)
    };
    target.join("release/closure-rs")
}

fn die(m: &str) -> ! {
    eprintln!("fuzz-driver: {m}");
    std::process::exit(2)
}

fn main() {
    let o = parse_opts();
    match o.cmd.as_str() {
        "parse-rate" => parse_rate(&o),
        "export" => export(&o),
        "run" => run(&o),
        "minimize-selftest" => minimize_selftest(&o),
        _ => die(
            "usage: fuzz-driver parse-rate|export|run|minimize-selftest [options]; see src/main.rs",
        ),
    }
}

// ---------------------------------------------------------------------------------------
// Shared helpers

/// Worker threads parse, walk and drop deep `parse_dump` JSON: give them a large stack.
fn spawn_deep<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> std::thread::JoinHandle<T> {
    std::thread::Builder::new()
        .stack_size(fuzz_oracle::DEEP_STACK)
        .spawn(f)
        .unwrap_or_else(|e| die(&format!("spawn worker: {e}")))
}

fn parse_dump(s: &mut Server, src: &str, name: &str, args: &[String]) -> Result<Value, String> {
    let mut req = json!({"op": "parse_dump", "content": src, "name": name});
    if !args.is_empty() {
        req["args"] = json!(args);
    }
    let r = s.request(req, TIMEOUT)?;
    if r.get("ok") != Some(&Value::Bool(true)) {
        return Err(format!(
            "parse_dump failed: {}",
            r.get("error").and_then(|e| e.as_str()).unwrap_or("?")
        ));
    }
    Ok(r)
}

fn parse_errors(r: &Value) -> Vec<String> {
    r.get("errors")
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
        .unwrap_or_default()
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo_root())
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

fn compile(s: &mut Server, args: &[String]) -> Result<Outcome, String> {
    let r = s.request(json!({"op": "compile", "args": args}), TIMEOUT)?;
    Outcome::from_compile(&r)
}

/// `a//b/./c` -> `a/b/c` (output-file keys: the oracle reports the path the compiler opened,
/// e.g. `--chunk_output_path_prefix=<out_dir>/` + `c0.js`).
fn normalize_key(k: &str) -> String {
    let abs = k.starts_with('/');
    let parts: Vec<&str> = k
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    format!("{}{}", if abs { "/" } else { "" }, parts.join("/"))
}

fn normalize_file_keys(files: BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    files
        .into_iter()
        .map(|(k, v)| (normalize_key(&k), v))
        .collect()
}

/// Every regular file under `dir`, keyed `<key_prefix>/<path relative to dir>` (sorted).
fn collect_files(dir: &Path, key_prefix: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("read_dir {}: {e}", d.display())),
        };
        for ent in rd {
            let p = ent.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                let r = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy();
                let bytes = std::fs::read(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
                out.insert(format!("{key_prefix}/{r}"), bytes);
            }
        }
    }
    Ok(out)
}

/// Engine `rust`: run `bin args` as the D2 runner runs the port (`gates/lib/d2_rust_core.py`
/// `run_pair`): cwd `cwd`, the golden environment, stdin `/dev/null`.
/// The outcome holds the exit status (128 + signal for a signal), stdout, stderr and every
/// file under `out_dir` (keys: [`collect_files`] with `out_key`). A run still alive after
/// `timeout` is killed and reported as exit [`RUST_TIMEOUT_EXIT`] with a note on stderr.
fn run_binary(
    bin: &Path,
    args: &[String],
    cwd: &Path,
    out_dir: &Path,
    out_key: &str,
    timeout: Duration,
) -> Result<Outcome, String> {
    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(fuzz_oracle::golden_env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = vec![];
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out_t = pipe(
        child
            .stdout
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let err_t = pipe(
        child
            .stderr
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let t0 = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| format!("wait: {e}"))? {
            Some(st) => break Some(st),
            None if t0.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    };
    let stdout = out_t.join().unwrap_or_default();
    let mut stderr = err_t.join().unwrap_or_default();
    let exit_code = match status {
        Some(st) => match (st.code(), st.signal()) {
            (Some(c), _) => i64::from(c),
            (None, Some(sig)) => 128 + i64::from(sig),
            (None, None) => -1,
        },
        None => {
            stderr.extend_from_slice(
                format!(
                    "\n[fuzz-driver] closure-rs killed after {} s\n",
                    timeout.as_secs()
                )
                .as_bytes(),
            );
            RUST_TIMEOUT_EXIT
        }
    };
    Ok(Outcome {
        exit_code,
        stdout,
        stderr,
        files: collect_files(out_dir, out_key)?,
    })
}

fn restart_if_dead(s: &mut Server, err: &str) {
    if ["timeout", "died", "write", "flush", "bad response"]
        .iter()
        .any(|k| err.contains(k))
    {
        s.kill();
        match Server::start(XMX) {
            Ok(n) => *s = n,
            Err(e) => die(&format!("cannot restart oracle: {e}")),
        }
    }
}

/// (single-input profiles, all profiles), in `corpus/d2/profiles.json` order.
fn profile_lists(h: &mut ArgsHelper) -> (Vec<String>, Vec<String>) {
    let p = h
        .ask(&json!({"op": "profiles"}))
        .unwrap_or_else(|e| die(&e));
    let names = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let multi = names(&p["multi_input_only"]);
    let all = names(&p["profiles"]);
    let single = all.iter().filter(|n| !multi.contains(n)).cloned().collect();
    (single, all)
}

/// The profile `run` uses for program `i` (deterministic).
fn pick_profile(seed: u64, i: u64, prog: &Prog, lists: &(Vec<String>, Vec<String>)) -> String {
    pick_profile_n(seed, i, prog.files.len(), lists)
}

/// D-016 profile weights: the ADVANCED family gets 65% of all draws, ws 3%, and the remaining
/// profiles 32%. Within a group the applicable profiles are equally likely (single-file programs:
/// advanced and advanced_strict share the family's 65%; multi-file programs: the four family
/// profiles do). The draw is `Rng::fork(seed ^ PROFILE_SALT, i)`: group first, then member.
const ADVANCED_FAMILY: &[&str] = &["advanced", "advanced_strict", "chunks2", "chunks3"];
const PROFILE_WEIGHTS_PCT: [(&str, u64); 3] = [("advanced_family", 65), ("ws", 3), ("other", 32)];

fn profile_group(p: &str) -> &'static str {
    if ADVANCED_FAMILY.contains(&p) {
        "advanced_family"
    } else if p == "ws" {
        "ws"
    } else {
        "other"
    }
}

/// The documented weights, for reports (`export` summary, `run` report).
fn profile_weights_json() -> Value {
    json!({
        "decision": "D-016",
        "groups_pct": PROFILE_WEIGHTS_PCT.iter().map(|(g, w)| (g.to_string(), json!(w))).collect::<serde_json::Map<_, _>>(),
        "advanced_family": ADVANCED_FAMILY,
        "within_group": "uniform over the profiles that apply to the program",
        "draw": "Rng::fork(seed ^ PROFILE_SALT, i): below(100) picks the group, then below(n) the member",
    })
}

/// The profile for program `i` with `nfiles` input files. It depends only on the file count,
/// so single-file programs know their profile before they are generated (`program`).
fn pick_profile_n(seed: u64, i: u64, nfiles: usize, lists: &(Vec<String>, Vec<String>)) -> String {
    let l = if nfiles >= 2 { &lists.1 } else { &lists.0 };
    let mut r = Rng::fork(seed ^ PROFILE_SALT, i);
    let x = r.below(100);
    let mut acc = 0;
    let mut group = PROFILE_WEIGHTS_PCT[PROFILE_WEIGHTS_PCT.len() - 1].0;
    for (g, w) in PROFILE_WEIGHTS_PCT {
        acc += w;
        if x < acc {
            group = g;
            break;
        }
    }
    let members: Vec<&String> = l.iter().filter(|p| profile_group(p) == group).collect();
    if members.is_empty() {
        die(&format!(
            "no applicable profile in group {group} (corpus/d2/profiles.json changed?)"
        ));
    }
    members[r.below(members.len() as u64) as usize].clone()
}

/// The keys a profile locks (`corpus/d2/profiles.json`): case flags may not override them.
fn profile_locked(profile: &str) -> Vec<String> {
    static LOCKED: std::sync::OnceLock<BTreeMap<String, Vec<String>>> = std::sync::OnceLock::new();
    LOCKED
        .get_or_init(|| {
            let path = repo_root().join("corpus/d2/profiles.json");
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| die(&format!("read {}: {e}", path.display())));
            let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| die(&e.to_string()));
            let mut m = BTreeMap::new();
            if let Some(ps) = v["profiles"].as_object() {
                for (name, p) in ps {
                    let l = p["locked"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    m.insert(name.clone(), l);
                }
            }
            m
        })
        .get(profile)
        .cloned()
        .unwrap_or_default()
}

/// Profiles whose `--language_out` (corpus/d2/profiles.json) is below ES2018: Java cannot
/// transpile BigInt or the ES2018+ regexp features to them (JSC_UNTRANSPILABLE, no output),
/// so jsgen programs for these profiles are generated with `dialect::low_target` set.
fn low_target_profiles() -> &'static Vec<String> {
    static LOW: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    LOW.get_or_init(|| {
        let low = [
            "ECMASCRIPT3",
            "ECMASCRIPT5",
            "ECMASCRIPT5_STRICT",
            "ECMASCRIPT_2015",
            "ECMASCRIPT6",
            "ECMASCRIPT6_STRICT",
            "ECMASCRIPT_2016",
            "ECMASCRIPT_2017",
        ];
        profiles_with_flag(|f| {
            f.strip_prefix("--language_out=")
                .is_some_and(|x| low.contains(&x))
        })
    })
}

/// Profiles that compile with `--compilation_level=ADVANCED` (corpus/d2/profiles.json).
fn advanced_profiles() -> &'static Vec<String> {
    static ADV: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ADV.get_or_init(|| profiles_with_flag(|f| f == "--compilation_level=ADVANCED"))
}

fn profiles_with_flag(pred: impl Fn(&str) -> bool) -> Vec<String> {
    let path = repo_root().join("corpus/d2/profiles.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| die(&format!("read {}: {e}", path.display())));
    let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| die(&e.to_string()));
    let mut out = vec![];
    if let Some(m) = v["profiles"].as_object() {
        for (name, p) in m {
            let flags = p["flags"].as_array().cloned().unwrap_or_default();
            if flags.iter().filter_map(|f| f.as_str()).any(&pred) {
                out.push(name.clone());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Program sources

/// One fuzz program: one or more files compiled together.
#[derive(Clone, Debug)]
struct Prog {
    /// `(file name, contents)` in `--js` order.
    files: Vec<(String, String)>,
    /// Case `extra_flags` (go through case_args precedence).
    extra_flags: Vec<String>,
    /// Extra CLI flags for the parse filter (`parse_dump` `args`).
    parse_args: Vec<String>,
    origin: String,
    category: &'static str,
}

impl Prog {
    fn all_text(&self) -> String {
        self.files.iter().map(|(_, c)| c.as_str()).collect()
    }
}

struct D2Inputs {
    files: Vec<PathBuf>,
    /// The files whose D2 golden ADVANCED run exits 0 with non-trivial output: the host pool
    /// for mutants compiled under an ADVANCED-family profile.
    adv_files: Vec<PathBuf>,
    donors: mutate::Donors,
}

impl D2Inputs {
    /// Visible single-input D2 cases whose input is present and at most 32 KB (the
    /// mutator's filter); the whole pool is the donor set.
    fn load() -> D2Inputs {
        let files = mutate::visible_d2_inputs(&repo_root(), 32 * 1024);
        let srcs: Vec<String> = files
            .iter()
            .map(|f| std::fs::read_to_string(f).unwrap_or_default())
            .collect();
        let donors = mutate::Donors::from_sources(&srcs);
        let adv_files = mutate::advanced_viable(&repo_root(), &files);
        D2Inputs {
            files,
            adv_files,
            donors,
        }
    }

    fn empty() -> D2Inputs {
        D2Inputs {
            files: vec![],
            adv_files: vec![],
            donors: mutate::Donors::default(),
        }
    }
}

/// Category of program `i` (see the module doc for the proportions).
fn category(o: &Opts, i: u64) -> &'static str {
    if o.source == "mutate" {
        return "mutate";
    }
    let mut r = Rng::fork(o.seed ^ MIX_SALT, i);
    let x = if o.source == "mixed" {
        r.below(100)
    } else {
        r.below(75)
    };
    match x {
        0..30 => "lang",
        30..55 => "closure",
        55..70 => "multi",
        70..75 => "sloppy",
        _ => "mutate",
    }
}

/// Program `i`: the generated or mutated files, plus random in-scope option flags for the
/// profile it will be compiled under ([`option_flags`]).
fn program(o: &Opts, d2: &D2Inputs, i: u64, lists: &(Vec<String>, Vec<String>)) -> Prog {
    let mut p = program_base(o, d2, i, lists);
    let profile = pick_profile_n(o.seed, i, p.files.len(), lists);
    let mut extra = option_flags(o.seed, i, &profile, &p);
    if o.wide {
        let has = |k: &str| extra.iter().chain(&p.extra_flags).any(|f| flag_key(f) == k);
        let mut w = vec![];
        if p.files.len() == 1
            && let Some(f) = wide_lang_out(o.seed, i, &profile)
            && !has(flag_key(f))
        {
            w.push(f.to_string());
        }
        if let Some(f) = wide_typecheck(o.seed, i, &profile) {
            w.push(f.to_string());
        }
        extra.extend(w);
    }
    if !extra.is_empty() {
        p.origin.push_str(&format!(":opts={}", extra.join(",")));
        p.extra_flags.extend(extra);
    }
    p
}

fn program_base(o: &Opts, d2: &D2Inputs, i: u64, lists: &(Vec<String>, Vec<String>)) -> Prog {
    let seed = o.seed;
    // Single-file jsgen programs: generate for the profile they will be compiled under.
    let p1 = pick_profile_n(seed, i, 1, lists);
    let low =
        low_target_profiles().contains(&p1) || (o.wide && wide_lang_out(seed, i, &p1).is_some());
    let lowt = |f: &dyn Fn() -> String| dialect::with_low_target(low, f);
    let lang_cfg = jsgen::Config {
        closure: false,
        ..jsgen::Config::default()
    };
    let single = |src: String, origin: String, category, flags: Vec<String>| Prog {
        files: vec![("input.js".into(), src)],
        extra_flags: flags.clone(),
        parse_args: flags,
        origin,
        category,
    };
    match category(o, i) {
        "mutate" if !d2.files.is_empty() => {
            let mut r = Rng::fork(seed ^ MUTATE_SALT, i);
            let adv = advanced_profiles().contains(&pick_profile_n(seed, i, 1, lists));
            let pool = if adv && !d2.adv_files.is_empty() {
                &d2.adv_files
            } else {
                &d2.files
            };
            let f = &pool[r.below(pool.len() as u64) as usize];
            let src = std::fs::read_to_string(f).unwrap_or_default();
            let n = r.range(1, 3);
            let m = mutate::mutate_with(&src, Some(&d2.donors), r.next_u64(), n);
            let ops: Vec<&str> = m.ops.iter().map(|op| op.name()).collect();
            single(
                m.text,
                format!("mutate:{}:n={n}:ops={}", rel(f), ops.join(",")),
                "mutate",
                vec![],
            )
        }
        "closure" => single(
            lowt(&|| jsgen::generate_nth(seed, i, &jsgen::Config::default())),
            format!(
                "jsgen:closure:seed={seed}:index={i}{}",
                if low { ":low_target" } else { "" }
            ),
            "closure",
            vec![],
        ),
        "multi" => {
            let p = jsgen::generate_closure_nth(seed, i, &jsgen::Config::default());
            Prog {
                files: p.files,
                extra_flags: p.extra_flags,
                parse_args: vec![],
                origin: format!("jsgen:multi:{:?}:seed={seed}:index={i}", p.kind),
                category: "multi",
            }
        }
        "sloppy" => single(
            lowt(&|| dialect::generate_nth(seed, i, &lang_cfg, Dialect::SLOPPY)),
            format!(
                "jsgen:sloppy:seed={seed}:index={i}{}",
                if low { ":low_target" } else { "" }
            ),
            "sloppy",
            Dialect::SLOPPY
                .flags()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        _ => single(
            lowt(&|| jsgen::generate_nth(seed, i, &lang_cfg)),
            format!(
                "jsgen:lang:seed={seed}:index={i}{}",
                if low { ":low_target" } else { "" }
            ),
            "lang",
            vec![],
        ),
    }
}

/// Write `prog`'s files into `dir` (cleared first); returns their paths in `--js` order.
fn materialize(prog: &Prog, dir: &Path) -> Result<Vec<PathBuf>, String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let mut paths = vec![];
    for (name, content) in &prog.files {
        let p = dir.join(name);
        std::fs::write(&p, content).map_err(|e| format!("write {}: {e}", p.display()))?;
        paths.push(p);
    }
    Ok(paths)
}

// ---------------------------------------------------------------------------------------
// Engines

/// Rust engine timeout. Java's is [`TIMEOUT`]; a Rust compile that runs this long is filed as a
/// mismatch (the outcome says so), not dropped.
const RUST_TIMEOUT: Duration = Duration::from_secs(120);
/// Exit code recorded for a Rust run killed at [`RUST_TIMEOUT`] (no real process status).
const RUST_TIMEOUT_EXIT: i64 = -9;

struct Engines {
    a: Server,
    /// The second oracle JVM (engines `java`, `java-perturbed`); `None` for `rust`.
    b: Option<Server>,
    b_kind: String,
    /// Engine `rust`: the closure-rs CLI binary.
    rust_bin: Option<PathBuf>,
    args: ArgsHelper,
}

impl Engines {
    fn start(b_kind: &str, rust_bin: Option<PathBuf>) -> Engines {
        let rust = b_kind == "rust";
        Engines {
            a: Server::start(XMX).unwrap_or_else(|e| die(&e)),
            b: if rust {
                None
            } else {
                Some(Server::start(XMX).unwrap_or_else(|e| die(&e)))
            },
            b_kind: b_kind.to_string(),
            rust_bin: if rust {
                Some(rust_bin.unwrap_or_else(default_rust_bin))
            } else {
                None
            },
            args: ArgsHelper::start().unwrap_or_else(|e| die(&e)),
        }
    }

    fn argv(
        &mut self,
        inputs: &[PathBuf],
        extra_flags: &[String],
        profile: &str,
        out_dir: &Path,
    ) -> Result<Vec<String>, String> {
        let ins: Vec<String> = inputs.iter().map(|p| rel(p)).collect();
        let a = self.args.ask(
            &json!({"id": "fuzz", "inputs": ins, "extra_flags": extra_flags,
            "profile": profile, "out_dir": rel(out_dir)}),
        )?;
        a["args"]
            .as_array()
            .ok_or_else(|| format!("fuzz_args: no args in {a}"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| "fuzz_args: non-string arg".to_string())
            })
            .collect()
    }

    fn engine_a(&mut self, args: &[String]) -> Result<Outcome, String> {
        compile(&mut self.a, args).inspect_err(|e| restart_if_dead(&mut self.a, e))
    }

    fn engine_b(&mut self, args: &[String], src: &str, out_dir: &Path) -> Result<Outcome, String> {
        if self.b_kind == "rust" {
            let bin = self
                .rust_bin
                .as_ref()
                .ok_or("engine rust without a binary")?;
            return run_binary(
                bin,
                args,
                &repo_root(),
                out_dir,
                &rel(out_dir),
                RUST_TIMEOUT,
            );
        }
        let b = self.b.as_mut().ok_or("engine B has no oracle server")?;
        let mut out = compile(b, args).inspect_err(|e| restart_if_dead(b, e))?;
        match self.b_kind.as_str() {
            "java" => {}
            "java-perturbed" => {
                if src.contains("switch") {
                    for v in out.files.values_mut() {
                        v.extend_from_slice(b"/* perturbed */\n");
                    }
                    if out.files.is_empty() {
                        out.stderr.extend_from_slice(b"perturbed\n");
                    }
                }
            }
            k => die(&format!("unknown engine {k}")),
        }
        Ok(out)
    }

    /// Compare `prog` (written to `in_dir`) under `profile`.
    fn compare(
        &mut self,
        prog: &Prog,
        in_dir: &Path,
        profile: &str,
        out_dir: &Path,
    ) -> Result<Option<(String, Outcome, Outcome)>, String> {
        let paths = materialize(prog, in_dir)?;
        let args = self.argv(&paths, &prog.extra_flags, profile, out_dir)?;
        let _ = std::fs::remove_dir_all(out_dir);
        let mut a = self.engine_a(&args)?;
        if is_java_crash(&a) {
            // D-009: a Java crash is not reference behaviour; such pairs are dropped, not compared.
            return Err(format!("{JAVA_CRASH}exit {}", a.exit_code));
        }
        let _ = std::fs::remove_dir_all(out_dir);
        let mut b = self.engine_b(&args, &prog.all_text(), out_dir)?;
        if self.b_kind == "rust" {
            // The oracle keys output files by the path the compiler opened; the Rust engine
            // by `<out_dir>/<relative path>`. Compare both under one spelling.
            a.files = normalize_file_keys(std::mem::take(&mut a.files));
            b.files = normalize_file_keys(std::mem::take(&mut b.files));
        }
        Ok(a.diff(&b).map(|d| (d, a, b)))
    }

    /// The parse filter for one file: `Ok(Some(dump))` if accepted, `Ok(None)` if the parser
    /// reports errors, `Err` on an oracle failure.
    fn parse_file(
        &mut self,
        src: &str,
        name: &str,
        args: &[String],
    ) -> Result<Option<Value>, String> {
        match parse_dump(&mut self.a, src, name, args) {
            Ok(r) if parse_errors(&r).is_empty() => Ok(Some(r)),
            Ok(_) => Ok(None),
            Err(e) => {
                restart_if_dead(&mut self.a, &e);
                Err(e)
            }
        }
    }

    fn parses(&mut self, src: &str, args: &[String]) -> Option<Value> {
        self.parse_file(src, "input.js", args).ok().flatten()
    }

    /// Every file of `prog` passes the parse filter.
    fn prog_parses(&mut self, prog: &Prog) -> Result<bool, String> {
        for (name, src) in &prog.files {
            if self.parse_file(src, name, &prog.parse_args)?.is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

struct Shared<'e> {
    eng: &'e RefCell<Engines>,
    parse_args: Vec<String>,
}

impl TreeSource for Shared<'_> {
    fn tree(&mut self, src: &str) -> Option<Tree> {
        let r = self.eng.borrow_mut().parses(src, &self.parse_args)?;
        Tree::from_parse_dump(&r, src)
    }
}

// ---------------------------------------------------------------------------------------
// parse-rate

fn parse_rate(o: &Opts) {
    let cfg = jsgen::Config::default();
    let next = Arc::new(AtomicU64::new(0));
    let results = Arc::new(Mutex::new(Vec::<(u64, Vec<String>)>::new()));
    let out_dir = repo_root().join(format!("build/fuzz/parse-rate/{}", o.seed));
    std::fs::create_dir_all(&out_dir).unwrap();
    let t0 = Instant::now();
    let mut hs = vec![];
    for _ in 0..o.servers {
        let (next, results, out_dir, cfg) =
            (next.clone(), results.clone(), out_dir.clone(), cfg.clone());
        let (seed, count) = (o.seed, o.count);
        hs.push(spawn_deep(move || {
            let mut s = Server::start(XMX).unwrap_or_else(|e| die(&e));
            loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= count {
                    return;
                }
                let src = jsgen::generate_nth(seed, i, &cfg);
                std::fs::write(out_dir.join(format!("p{i}.js")), &src).unwrap();
                let errs = match parse_dump(&mut s, &src, &format!("p{i}.js"), &[]) {
                    Ok(r) => parse_errors(&r),
                    Err(e) => {
                        restart_if_dead(&mut s, &e);
                        vec![format!("ORACLE: {e}")]
                    }
                };
                results.lock().unwrap().push((i, errs));
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    let mut res = results.lock().unwrap().clone();
    res.sort();
    let accepted = res.iter().filter(|(_, e)| e.is_empty()).count();
    let rejected: Vec<Value> = res
        .iter()
        .filter(|(_, e)| !e.is_empty())
        .map(|(i, e)| json!({"index": i, "errors": e}))
        .collect();
    let report = json!({
        "seed": o.seed, "count": res.len(), "accepted": accepted,
        "rate": accepted as f64 / res.len().max(1) as f64,
        "method": "oracle parse_dump (CLI default options: language_in STABLE_IN=ECMASCRIPT_NEXT, strict mode as a real compile); accepted = errors == []",
        "generator": {"max_depth": cfg.max_depth(), "closure": cfg.closure},
        "programs_dir": rel(&out_dir), "wall_s": t0.elapsed().as_secs_f64(), "rejected": rejected,
    });
    let p = repo_root().join(format!("build/fuzz/parse-rate-{}.json", o.seed));
    std::fs::write(&p, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&json!({"accepted": accepted, "count": res.len(), "rate": report["rate"], "report": rel(&p)})).unwrap());
}

// ---------------------------------------------------------------------------------------
// export

fn export(o: &Opts) {
    let out = o
        .out
        .clone()
        .unwrap_or_else(|| die("export needs --out DIR"));
    if o.count == u64::MAX {
        die("export needs --count");
    }
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    require_option_pool();
    let d2 = Arc::new(if o.source == "gen" {
        D2Inputs::empty()
    } else {
        D2Inputs::load()
    });
    let lists = {
        let mut h = ArgsHelper::start().unwrap_or_else(|e| die(&e));
        Arc::new(profile_lists(&mut h))
    };
    let next = Arc::new(AtomicU64::new(0));
    let rows = Arc::new(Mutex::new(Vec::<(u64, Value)>::new()));
    let t0 = Instant::now();
    let mut hs = vec![];
    for _ in 0..o.servers {
        let (next, rows, d2, lists, o, out) = (
            next.clone(),
            rows.clone(),
            d2.clone(),
            lists.clone(),
            o.clone(),
            out.clone(),
        );
        hs.push(spawn_deep(move || {
            let mut s = Server::start(XMX).unwrap_or_else(|e| die(&e));
            loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= o.count {
                    return;
                }
                let prog = program(&o, &d2, i, &lists);
                let dir = out.join(format!("p{i}"));
                let paths = materialize(&prog, &dir).unwrap_or_else(|e| die(&e));
                let mut errors = vec![];
                let mut oracle_error = None;
                for (name, src) in &prog.files {
                    match parse_dump(&mut s, src, name, &prog.parse_args) {
                        Ok(r) => errors
                            .extend(parse_errors(&r).into_iter().map(|e| format!("{name}: {e}"))),
                        Err(e) => {
                            restart_if_dead(&mut s, &e);
                            oracle_error = Some(e);
                        }
                    }
                }
                let profile = pick_profile(o.seed, i, &prog, &lists);
                let row = json!({
                    "index": i, "category": prog.category, "origin": prog.origin,
                    "files": paths.iter().map(|p| rel(p)).collect::<Vec<_>>(),
                    "extra_flags": prog.extra_flags, "parse_args": prog.parse_args,
                    "profile": profile,
                    "accepted": errors.is_empty() && oracle_error.is_none(),
                    "errors": errors, "oracle_error": oracle_error,
                    "bytes": prog.files.iter().map(|(_, c)| c.len()).sum::<usize>(),
                });
                rows.lock().unwrap().push((i, row));
            }
        }));
    }
    for h in hs {
        if h.join().is_err() {
            die("export worker panicked");
        }
    }
    let mut rows = rows.lock().unwrap().clone();
    rows.sort_by_key(|(i, _)| *i);
    let mut man = String::new();
    let mut by_cat: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for (_, r) in &rows {
        man.push_str(&serde_json::to_string(r).unwrap());
        man.push('\n');
        let e = by_cat
            .entry(r["category"].as_str().unwrap_or("?").to_string())
            .or_default();
        e.0 += 1;
        if r["accepted"] == Value::Bool(true) {
            e.1 += 1;
        }
    }
    std::fs::write(out.join("manifest.jsonl"), man).unwrap();
    let accepted: u64 = by_cat.values().map(|v| v.1).sum();
    let summary = json!({
        "seed": o.seed, "source": o.source, "count": rows.len(), "accepted": accepted,
        "rate": accepted as f64 / rows.len().max(1) as f64,
        "by_category": by_cat.iter().map(|(k, (n, a))| (k.clone(), json!({"programs": n, "accepted": a, "rate": *a as f64 / (*n).max(1) as f64}))).collect::<serde_json::Map<_, _>>(),
        "method": "oracle parse_dump per file (CLI default options; sloppy programs add --strict_mode_input=false), golden env; accepted = every file's errors == [] and no oracle error",
        "profile_weights": profile_weights_json(),
        "option_pool": option_pool_json(),
        "wall_s": t0.elapsed().as_secs_f64(),
    });
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}

// ---------------------------------------------------------------------------------------
// run

#[derive(Default, Clone, Debug)]
struct Counts {
    programs: u64,
    parse_rejected: u64,
    compared: u64,
    mismatches: u64,
    oracle_errors: u64,
    java_crashes: u64,
}

impl Counts {
    fn to_json(&self) -> Value {
        json!({"programs": self.programs, "parse_rejected": self.parse_rejected,
            "compared": self.compared, "mismatches": self.mismatches,
            "oracle_errors": self.oracle_errors, "java_crashes": self.java_crashes})
    }
}

#[derive(Default, Clone, Debug)]
struct Tally {
    total: Counts,
    by_category: BTreeMap<String, Counts>,
    by_profile: BTreeMap<String, u64>,
    java_crashes: Vec<String>,
    oracle_error_samples: Vec<String>,
    filed: Vec<String>,
}

impl Tally {
    fn bump(&mut self, cat: &str, f: impl Fn(&mut Counts)) {
        f(&mut self.total);
        f(self.by_category.entry(cat.to_string()).or_default());
    }
}

/// `(path, sha256)` of the Rust engine's binary, for reports (null for the Java engines).
fn rust_bin_json(o: &Opts) -> Value {
    match &o.rust_bin {
        Some(p) if o.engine_b == "rust" => {
            static SHA: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            let sha = SHA.get_or_init(|| {
                std::fs::read(p)
                    .map(|b| {
                        Sha256::digest(&b)
                            .iter()
                            .map(|x| format!("{x:02x}"))
                            .collect::<String>()
                    })
                    .unwrap_or_default()
            });
            json!({"path": p.display().to_string(), "sha256": sha})
        }
        _ => Value::Null,
    }
}

fn run_report(o: &Opts, run_id: &str, t: &Tally, wall: f64, crashes: u64, done: bool) -> Value {
    json!({
        "run": run_id, "engine_a": "java-oracle", "engine_b": o.engine_b, "source": o.source,
        "rust_bin": rust_bin_json(o),
        "seed": o.seed, "servers": o.servers, "wide": o.wide,
        "duration_s": o.duration.map(|d| d.as_secs()),
        "count_limit": if o.count == u64::MAX { Value::Null } else { json!(o.count) },
        "finished": done, "wall_s": wall, "harness_crashes": crashes,
        "programs": t.total.programs, "parse_rejected": t.total.parse_rejected,
        "compared": t.total.compared, "mismatches": t.total.mismatches,
        "oracle_errors": t.total.oracle_errors,
        "oracle_error_samples": t.oracle_error_samples,
        "java_crashes_dropped": t.java_crashes, "filed": t.filed,
        "by_category": t.by_category.iter().map(|(k, v)| (k.clone(), v.to_json())).collect::<serde_json::Map<_, _>>(),
        "by_profile": t.by_profile,
        "profile_weights": profile_weights_json(),
        "findings_dir": rel(&o.findings),
    })
}

fn run(o: &Opts) {
    require_option_pool();
    let d2 = Arc::new(if o.source == "gen" {
        D2Inputs::empty()
    } else {
        D2Inputs::load()
    });
    let run_id = format!("{}-{}-{}", o.source, o.engine_b, o.seed);
    let work = o
        .work
        .clone()
        .unwrap_or_else(|| repo_root().join(format!("build/fuzz/work/{run_id}")));
    let report_path = o
        .report
        .clone()
        .unwrap_or_else(|| repo_root().join(format!("build/fuzz/run-{run_id}.json")));
    std::fs::create_dir_all(&work).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::create_dir_all(&o.findings).unwrap_or_else(|e| die(&e.to_string()));
    let next = Arc::new(AtomicU64::new(0));
    let tally = Arc::new(Mutex::new(Tally::default()));
    let opts = Arc::new(o.clone());
    let t0 = Instant::now();
    let deadline = o.duration.map(|d| t0 + d);
    let mut hs = vec![];
    for w in 0..o.servers {
        let (next, tally, opts, d2, work) = (
            next.clone(),
            tally.clone(),
            opts.clone(),
            d2.clone(),
            work.clone(),
        );
        hs.push(spawn_deep(move || {
            let eng = RefCell::new(Engines::start(&opts.engine_b, opts.rust_bin.clone()));
            let lists = profile_lists(&mut eng.borrow_mut().args);
            loop {
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    return;
                }
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= opts.count {
                    return;
                }
                let prog = program(&opts, &d2, i, &lists);
                let cat = prog.category;
                tally.lock().unwrap().bump(cat, |c| c.programs += 1);
                match eng.borrow_mut().prog_parses(&prog) {
                    Ok(true) => {}
                    Ok(false) => {
                        tally.lock().unwrap().bump(cat, |c| c.parse_rejected += 1);
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[{i}] oracle error in parse filter: {e}");
                        let mut t = tally.lock().unwrap();
                        t.bump(cat, |c| c.oracle_errors += 1);
                        if t.oracle_error_samples.len() < 50 {
                            t.oracle_error_samples.push(format!("{i} parse: {e}"));
                        }
                        continue;
                    }
                }
                let profile = pick_profile(opts.seed, i, &prog, &lists);
                *tally
                    .lock()
                    .unwrap()
                    .by_profile
                    .entry(profile.clone())
                    .or_default() += 1;
                let dir = work.join(format!("w{w}"));
                let in_dir = dir.join("in");
                let out_dir = dir.join("out");
                let res = eng.borrow_mut().compare(&prog, &in_dir, &profile, &out_dir);
                match res {
                    Err(e) if e.starts_with(JAVA_CRASH) => {
                        // Keep the program for inspection (a Java crash, not a port defect: D-009).
                        let keep = repo_root()
                            .join("build/fuzz/java-crashes")
                            .join(format!("{}-{i}-{profile}", opts.seed));
                        let _ = materialize(&prog, &keep);
                        let _ = std::fs::write(
                            keep.join("origin.txt"),
                            format!("{}\n{:?}\n", prog.origin, prog.extra_flags),
                        );
                        eprintln!(
                            "[{i}] Java crash under {profile} ({e}); kept as {}",
                            rel(&keep)
                        );
                        let mut t = tally.lock().unwrap();
                        t.bump(cat, |c| c.java_crashes += 1);
                        t.java_crashes.push(format!("{} {profile}", prog.origin));
                    }
                    Err(e) => {
                        eprintln!("[{i}] oracle error: {e}");
                        let mut t = tally.lock().unwrap();
                        t.bump(cat, |c| c.oracle_errors += 1);
                        if t.oracle_error_samples.len() < 50 {
                            t.oracle_error_samples
                                .push(format!("{i} {profile} {}: {e}", prog.origin));
                        }
                    }
                    Ok(None) => tally.lock().unwrap().bump(cat, |c| c.compared += 1),
                    Ok(Some((diff, _, _))) => {
                        tally.lock().unwrap().bump(cat, |c| {
                            c.compared += 1;
                            c.mismatches += 1;
                        });
                        eprintln!("[{i}] MISMATCH under {profile}: {diff}; minimizing");
                        let id =
                            file_finding(&opts, &eng, &prog, &profile, &diff, &in_dir, &out_dir);
                        tally.lock().unwrap().filed.push(id);
                    }
                }
            }
        }));
    }
    // Progress file every 30 s while the workers run.
    let mut last = Instant::now();
    while hs.iter().any(|h| !h.is_finished()) {
        std::thread::sleep(Duration::from_millis(500));
        if last.elapsed() >= Duration::from_secs(30) {
            last = Instant::now();
            let t = tally.lock().map(|t| t.clone()).unwrap_or_default();
            let rep = run_report(o, &run_id, &t, t0.elapsed().as_secs_f64(), 0, false);
            let _ = std::fs::write(
                report_path.with_extension("progress.json"),
                serde_json::to_string_pretty(&rep).unwrap_or_default(),
            );
            eprintln!(
                "[progress {:.0}s] programs={} compared={} rejected={} mismatches={} oracle_errors={} java_crashes={}",
                t0.elapsed().as_secs_f64(),
                t.total.programs,
                t.total.compared,
                t.total.parse_rejected,
                t.total.mismatches,
                t.total.oracle_errors,
                t.total.java_crashes
            );
        }
    }
    let mut crashes = 0u64;
    for h in hs {
        if h.join().is_err() {
            crashes += 1;
        }
    }
    let t = tally
        .lock()
        .map(|t| t.clone())
        .unwrap_or_else(|p| p.into_inner().clone());
    let report = run_report(o, &run_id, &t, t0.elapsed().as_secs_f64(), crashes, true);
    if let Some(p) = report_path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if crashes > 0 {
        eprintln!("fuzz-driver: {crashes} worker thread(s) panicked (harness crash)");
        std::process::exit(3);
    }
}

fn file_finding(
    o: &Opts,
    eng: &RefCell<Engines>,
    prog: &Prog,
    profile: &str,
    diff: &str,
    in_dir: &Path,
    out_dir: &Path,
) -> String {
    let kind = diff.split_whitespace().next().unwrap_or("").to_string();
    let t0 = Instant::now();
    let mut cur = prog.clone();
    let nfiles = cur.files.len().max(1);
    let per_file = (o.budget / nfiles).max(20);
    let mut stats_txt = vec![];
    // Reduce one file at a time (the others held at their current reduced text).
    for k in 0..cur.files.len() {
        let base = cur.clone();
        let mut pred = |cand: &str| -> bool {
            if eng.borrow_mut().parses(cand, &base.parse_args).is_none() {
                return false;
            }
            let mut p = base.clone();
            p.files[k].1 = cand.to_string();
            match eng.borrow_mut().compare(&p, in_dir, profile, out_dir) {
                Ok(Some((d, _, _))) => d.split_whitespace().next() == Some(kind.as_str()),
                _ => false,
            }
        };
        let (min, stats) = minimize::reduce(
            &base.files[k].1,
            &mut Shared {
                eng,
                parse_args: base.parse_args.clone(),
            },
            &mut pred as &mut dyn Predicate,
            per_file,
        );
        stats_txt.push(format!(
            "{}: {} -> {} bytes, {} predicate calls ({} AST steps), budget exhausted: {}",
            base.files[k].0,
            base.files[k].1.len(),
            min.len(),
            stats.tests,
            stats.ast_steps,
            stats.budget_exhausted
        ));
        cur.files[k].1 = min;
    }
    // Re-run the minimized repro to record the final diff and argv.
    let final_cmp = eng.borrow_mut().compare(&cur, in_dir, profile, out_dir);
    let paths: Vec<PathBuf> = cur.files.iter().map(|(n, _)| in_dir.join(n)).collect();
    let args = eng
        .borrow_mut()
        .argv(&paths, &cur.extra_flags, profile, out_dir)
        .unwrap_or_else(|e| vec![format!("(argv error: {e})")]);
    let (final_diff, a, b) = match final_cmp {
        Ok(Some((d, a, b))) => (d, Some(a), Some(b)),
        Ok(None) => ("(no longer reproduces)".into(), None, None),
        Err(e) => (format!("oracle error: {e}"), None, None),
    };
    let min_text: String = cur
        .files
        .iter()
        .map(|(n, c)| format!("// --- {n} ---\n{c}"))
        .collect();
    let hash = Sha256::digest(format!("{profile}\n{min_text}").as_bytes());
    let id = format!(
        "fz-{}",
        hash.iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let show = |x: &Option<Outcome>| {
        x.as_ref()
            .map(|o| {
                let mut s = format!(
                    "exit_code: {}\n--- stderr ---\n{}\n",
                    o.exit_code,
                    String::from_utf8_lossy(&o.stderr)
                );
                for (k, v) in &o.files {
                    s.push_str(&format!("--- {k} ---\n{}\n", String::from_utf8_lossy(v)));
                }
                s.chars().take(4000).collect::<String>()
            })
            .unwrap_or_default()
    };
    let orig_text: String = prog
        .files
        .iter()
        .map(|(n, c)| format!("// --- {n} ---\n{c}"))
        .collect();
    let md = format!(
        "# Fuzz finding {id}\n\n- engines: A = java-oracle, B = {eb}{bin}\n- profile: `{profile}`\n- extra flags: `{:?}`\n- difference: {final_diff}\n- original difference: {diff}\n- origin: `{}`\n- minimization ({:.1}s):\n{}\n\n## Argv (input paths as used)\n\n```\n{}\n```\n\n## Minimized repro\n\n```js\n{min_text}```\n\n## Engine A\n\n```\n{}\n```\n\n## Engine B\n\n```\n{}\n```\n\n## Original program\n\n```js\n{}```\n",
        cur.extra_flags,
        prog.origin,
        t0.elapsed().as_secs_f64(),
        stats_txt
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n"),
        args.join(" "),
        show(&a),
        show(&b),
        orig_text.chars().take(20_000).collect::<String>(),
        eb = o.engine_b,
        bin = match rust_bin_json(o) {
            Value::Null => String::new(),
            v => format!(
                " (`{}`, sha256 {})",
                rel(Path::new(v["path"].as_str().unwrap_or(""))),
                &v["sha256"].as_str().unwrap_or("")
                    [..12.min(v["sha256"].as_str().unwrap_or("").len())]
            ),
        },
    );
    if let Err(e) = std::fs::write(o.findings.join(format!("{id}.md")), md) {
        eprintln!("cannot write finding {id}: {e}");
    }
    eprintln!(
        "filed {id}: {} -> {} bytes",
        prog.all_text().len(),
        cur.all_text().len()
    );
    id
}

// ---------------------------------------------------------------------------------------
// minimize-selftest

fn minimize_selftest(o: &Opts) {
    let eng = RefCell::new(Engines::start("java", None));
    let cfg = jsgen::Config::default();
    let dir = repo_root().join("build/fuzz/minimize-selftest");
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.js");
    let out_dir = dir.join("out");
    let mut rows = vec![];
    let mut tried = 0u64;
    let mut i = 0u64;
    while (rows.len() as u64) < o.count && tried < o.count * 20 {
        tried += 1;
        let src = jsgen::generate_nth(o.seed, i, &cfg);
        i += 1;
        if eng.borrow_mut().parses(&src, &[]).is_none() {
            continue;
        }
        let Ok(args) = eng
            .borrow_mut()
            .argv(std::slice::from_ref(&input), &[], "simple", &out_dir)
        else {
            continue;
        };
        std::fs::write(&input, &src).unwrap();
        let Ok(out) = eng.borrow_mut().engine_a(&args) else {
            continue;
        };
        let text = out.all_text();
        let Some(needle) = ["switch", "while", "class ", "try", "for("]
            .iter()
            .find(|n| text.contains(**n))
        else {
            continue;
        };
        let needle = needle.to_string();
        let mut pred = |cand: &str| -> bool {
            if eng.borrow_mut().parses(cand, &[]).is_none() {
                return false;
            }
            std::fs::write(&input, cand).unwrap();
            let _ = std::fs::remove_dir_all(&out_dir);
            match eng.borrow_mut().engine_a(&args) {
                Ok(o) => o.exit_code == 0 && o.all_text().contains(&needle),
                Err(_) => false,
            }
        };
        let t0 = Instant::now();
        let (min, st) = minimize::reduce(
            &src,
            &mut Shared {
                eng: &eng,
                parse_args: vec![],
            },
            &mut pred as &mut dyn Predicate,
            o.budget,
        );
        let still = pred(&min);
        println!(
            "program {}: needle {:?}: {} -> {} bytes, {} calls ({} AST steps, {} line tests), holds={still}, {:.1}s\n{}",
            i - 1,
            needle,
            src.len(),
            min.len(),
            st.tests,
            st.ast_steps,
            st.line_steps,
            t0.elapsed().as_secs_f64(),
            min.trim_end()
        );
        rows.push(
            json!({"index": i - 1, "needle": needle, "before": src.len(), "after": min.len(),
            "calls": st.tests, "ast_steps": st.ast_steps, "holds": still, "minimized": min}),
        );
    }
    let p = repo_root().join(format!("build/fuzz/minimize-selftest-{}.json", o.seed));
    std::fs::write(
        &p,
        serde_json::to_string_pretty(&json!({"rows": rows})).unwrap(),
    )
    .unwrap();
    println!("report: {}", rel(&p));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags_txt() -> String {
        std::fs::read_to_string(repo_root().join("scope/flags.txt")).expect("scope/flags.txt")
    }

    #[test]
    fn option_pool_matches_scope_flags() {
        let errs = check_option_pool(&flags_txt());
        assert!(errs.is_empty(), "{errs:#?}");
    }

    #[test]
    fn option_pool_check_rejects_out_of_scope_and_unaccounted_flags() {
        let mut t = flags_txt();
        // Mark a drawn flag out of scope: the check must fail.
        t = t.replace("in\t--angular_pass\t", "out\t--angular_pass\t");
        assert!(
            check_option_pool(&t)
                .iter()
                .any(|e| e.contains("--angular_pass"))
        );
        // An in-scope flag that is neither drawn nor listed must be reported.
        let extra = format!(
            "{}in\t--made_up_flag\t-\tboolean\tfalse\tno\tx\ty\tz\n",
            flags_txt()
        );
        assert!(
            check_option_pool(&extra)
                .iter()
                .any(|e| e.contains("--made_up_flag"))
        );
    }

    #[test]
    fn advanced_family_is_the_advanced_profiles() {
        let mut a: Vec<String> = advanced_profiles().clone();
        a.sort();
        let mut f: Vec<String> = ADVANCED_FAMILY.iter().map(|s| s.to_string()).collect();
        f.sort();
        assert_eq!(a, f);
    }

    fn lists() -> (Vec<String>, Vec<String>) {
        let all: Vec<String> = [
            "ws",
            "simple",
            "advanced",
            "advanced_strict",
            "lang_es5",
            "lang_es2015",
            "lang_next",
            "pretty",
            "sourcemap",
            "chunks2",
            "chunks3",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let single = all
            .iter()
            .filter(|p| !p.starts_with("chunks"))
            .cloned()
            .collect();
        (single, all)
    }

    #[test]
    fn profile_weights_are_d016_and_deterministic() {
        let l = lists();
        for nfiles in [1usize, 3] {
            let n = 60_000u64;
            let mut by: BTreeMap<&str, u64> = BTreeMap::new();
            for i in 0..n {
                let p = pick_profile_n(20261006, i, nfiles, &l);
                assert_eq!(p, pick_profile_n(20261006, i, nfiles, &l));
                if nfiles == 1 {
                    assert!(!p.starts_with("chunks"));
                }
                *by.entry(profile_group(&p)).or_default() += 1;
            }
            let share = |g: &str| by.get(g).copied().unwrap_or(0) as f64 / n as f64;
            assert!((share("advanced_family") - 0.65).abs() < 0.01, "{by:?}");
            assert!((share("ws") - 0.03).abs() < 0.005, "{by:?}");
            assert!((share("other") - 0.32).abs() < 0.01, "{by:?}");
        }
    }

    /// A scratch directory for one test (removed first).
    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("fuzz-driver-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A fake `closure-rs`: a shell script with `body`.
    fn fake_bin(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("fake-closure-rs");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn normalize_key_collapses_separators() {
        assert_eq!(normalize_key("build/w0/out//c0.js"), "build/w0/out/c0.js");
        assert_eq!(
            normalize_key("build/./w0/out/out.js"),
            "build/w0/out/out.js"
        );
        assert_eq!(normalize_key("/abs//x"), "/abs/x");
        assert_eq!(normalize_key("out.js"), "out.js");
    }

    #[test]
    fn rust_engine_outcome_has_exit_streams_and_out_dir_files() {
        let d = scratch("outcome");
        // argv as case_args builds it: relative paths, resolved from the cwd.
        let bin = fake_bin(
            &d,
            r#"mkdir -p out/sub
printf 'js:%s\n' "$2" > out/out.js
printf 'map' > out/sub/c1.js.map
printf 'stdout:%s:%s' "$1" "$(cat)"
printf 'LANG=%s TZ=%s CARGO=%s' "$LANG" "$TZ" "${CARGO:-unset}" >&2
exit 3"#,
        );
        let args = vec!["--a=1".to_string(), "x y".to_string()];
        let o = run_binary(
            &bin,
            &args,
            &d,
            &d.join("out"),
            "w/out",
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!(o.exit_code, 3);
        // stdin is /dev/null.
        assert_eq!(o.stdout, b"stdout:--a=1:");
        // The golden environment, nothing inherited.
        assert_eq!(o.stderr, b"LANG=C.UTF-8 TZ=UTC CARGO=unset");
        let keys: Vec<&str> = o.files.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["w/out/out.js", "w/out/sub/c1.js.map"]);
        assert_eq!(o.files["w/out/out.js"], b"js:x y\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rust_engine_matches_the_oracle_outcome_shape() {
        // The same files keyed as the oracle keys them (the path the compiler opened) compare
        // equal once both sides are normalized, and any byte difference is reported.
        let d = scratch("shape");
        let bin = fake_bin(&d, "mkdir -p o; printf 'a' > o/c0.js; exit 0");
        let b = run_binary(&bin, &[], &d, &d.join("o"), "r/o", Duration::from_secs(30)).unwrap();
        let mut a = Outcome {
            exit_code: 0,
            stdout: vec![],
            stderr: vec![],
            files: BTreeMap::from([("r/o//c0.js".to_string(), b"a".to_vec())]),
        };
        a.files = normalize_file_keys(std::mem::take(&mut a.files));
        let b = Outcome {
            files: normalize_file_keys(b.files.clone()),
            ..b
        };
        assert_eq!(a.diff(&b), None);
        let mut c = b.clone();
        c.files.insert("r/o/c0.js".into(), b"b".to_vec());
        assert_eq!(a.diff(&c).as_deref(), Some("output file r/o/c0.js differs"));
        let mut e = b.clone();
        e.exit_code = 1;
        assert_eq!(a.diff(&e).as_deref(), Some("exit_code 0 vs 1"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rust_engine_reports_signals_timeouts_and_missing_out_dir() {
        let d = scratch("signal");
        let bin = fake_bin(&d, "kill -SEGV $$");
        let o = run_binary(&bin, &[], &d, &d.join("none"), "n", Duration::from_secs(30)).unwrap();
        assert_eq!(o.exit_code, 128 + 11);
        assert!(o.files.is_empty());
        let bin = fake_bin(&d, "exec sleep 20");
        let t0 = Instant::now();
        let o = run_binary(
            &bin,
            &[],
            &d,
            &d.join("none"),
            "n",
            Duration::from_millis(300),
        )
        .unwrap();
        assert!(t0.elapsed() < Duration::from_secs(10));
        assert_eq!(o.exit_code, RUST_TIMEOUT_EXIT);
        assert!(String::from_utf8_lossy(&o.stderr).contains("killed after"));
        assert!(run_binary(&d.join("missing"), &[], &d, &d, "n", Duration::from_secs(1)).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn engine_b_kinds_and_default_binary() {
        assert!(ENGINES_B.contains(&"rust"));
        assert!(ENGINES_B.contains(&"java"));
        assert!(default_rust_bin().ends_with("release/closure-rs"));
    }

    #[test]
    fn wide_draws_are_deterministic_and_respect_profiles() {
        let (mut lang, mut tc) = (0, 0);
        for i in 0..3000 {
            for prof in ["ws", "lang_es5", "lang_next", "chunks2"] {
                assert_eq!(wide_lang_out(9, i, prof), None, "{prof}");
            }
            assert_eq!(wide_typecheck(9, i, "ws"), None);
            let l = wide_lang_out(9, i, "advanced");
            assert_eq!(l, wide_lang_out(9, i, "advanced"));
            lang += usize::from(l.is_some());
            let t = wide_typecheck(9, i, "simple");
            assert_eq!(t, wide_typecheck(9, i, "simple"));
            tc += usize::from(t.is_some());
            for f in l.into_iter().chain(t) {
                assert!(WIDE_LANG_OUT.contains(&f) || WIDE_TYPECHECK.contains(&f));
            }
        }
        assert!((800..1200).contains(&lang), "{lang}");
        assert!((600..900).contains(&tc), "{tc}");
    }

    #[test]
    fn option_flags_respect_locks_and_conditions() {
        let single = Prog {
            files: vec![("input.js".into(), "x;".into())],
            extra_flags: vec![],
            parse_args: vec![],
            origin: String::new(),
            category: "lang",
        };
        for i in 0..3000 {
            for prof in [
                "advanced_strict",
                "simple",
                "advanced",
                "lang_es5",
                "sourcemap",
            ] {
                let f = option_flags(7, i, prof, &single);
                assert!(f.len() <= 2);
                for x in &f {
                    for l in profile_locked(prof) {
                        assert_ne!(flag_key(x), l, "{prof} locks {l}");
                    }
                    if advanced_profiles().iter().any(|p| p == prof) {
                        assert_ne!(x, "--renaming=false");
                    }
                    assert!(!x.starts_with("--chunk_"), "{x} under {prof}");
                    assert!(
                        !x.starts_with("--generate_exports"),
                        "{x} for a non-goog program"
                    );
                }
            }
            assert!(option_flags(7, i, "ws", &single).is_empty());
        }
    }
}
