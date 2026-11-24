/*
 * Copyright 2009 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/CompilerInput.java.

//! Rust-only, not in Java (crates/DESIGN.md, "Performance divergences"): parses the inputs of a
//! compilation on worker threads while the compiler asks for their ASTs one by one.
//!
//! Java parses each input when `CompilerInput#getAstRoot` first needs it, on the compiler thread
//! (or, with `--num_parallel_threads`, in `PrebuildAst`). Here `start` sets worker threads to
//! parse inputs that the compiler may ask for, each into an arena of its own
//! (`Ast::new_for_preparse`). When `CompilerInput#parse` then runs for an input, `take` waits for
//! that input's parse and moves its nodes into the compiler's arena (`Ast::append_preparsed`)
//! instead of parsing, provided the parse used the same code, parser configuration and source
//! kind that parsing now would. The appended nodes get the ids, and the errors the order, that
//! parsing at that point would have given them, so the compilation is unchanged. An input no
//! worker has started yet, or whose parse failed or panicked on a worker, is parsed by the
//! compiler itself; inputs the compiler never asks for are dropped by `finish`.

use crate::{
    abstract_compiler::{AbstractCompiler, ConfigContext},
    compiler::Compiler,
    compiler_input::CompilerInput,
    js_error::JSError,
};
use closure_parsing::{config::Config, parser_runner::ParseResult, parser_runner::ParserRunner};
use closure_rhino::{
    fast_hash::IndexMap,
    js_string::JsString,
    node::Ast,
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::collections::VecDeque;
use std::sync::{
    Arc, Condvar, Mutex, Once,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// The most worker threads one compilation uses.
const MAX_THREADS: usize = 8;
const THREAD_NAME: &str = "closure-preparse";

/// Rust-only (performance divergence): allows disabling multi-threaded helpers via environment.
static PARALLEL_ENABLED: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("CLOSURE_RS_PARALLEL").as_deref() != Ok("off"));

/// Rust-only (performance divergence): worker thread budget for scoped intra-pass parallelism,
/// scaled by `work_items / min_per_thread` and using the same busy-machine guard as `Preparser`.
pub(crate) fn parallel_thread_count(
    work_items: usize,
    min_per_thread: usize,
    max_threads: usize,
) -> usize {
    if !*PARALLEL_ENABLED || min_per_thread == 0 || work_items < min_per_thread.saturating_mul(2) {
        return 1;
    }
    let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let threads = cpus.min(max_threads).min(work_items / min_per_thread);
    if threads < 2 || machine_is_busy(cpus) {
        1
    } else {
        threads
    }
}

/// The parsing of a compilation's inputs on worker threads (`start` ... `finish`).
pub(crate) struct Preparser {
    shared: Arc<Shared>,
    /// The job of each input.
    jobs: IndexMap<CompilerInput, usize>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

struct Shared {
    jobs: Vec<Job>,
    next: AtomicUsize,
    imports: Option<ImportQueue>,
    stop: AtomicBool,
    slots: Vec<(Mutex<Slot>, Condvar)>,
}

/// For builds that prune from file entry points, follow relative imports on the workers.
/// Only the compiler decides which modules are dependencies and which diagnostics to report.
struct ImportQueue {
    paths: IndexMap<String, usize>,
    state: Mutex<PendingImports>,
    ready: Condvar,
}

struct PendingImports {
    pending: VecDeque<usize>,
    seen: Vec<bool>,
}

impl ImportQueue {
    fn for_inputs(compiler: &AbstractCompiler, jobs: &[Job], threads: usize) -> Option<Self> {
        let deps = compiler.get_options().get_dependency_options();
        // When all inputs fit in one wave, every parse can start immediately.
        if jobs.len() <= threads
            || !deps.should_drop_moochers()
            || deps.entry_points().is_empty()
            || compiler.get_options().get_process_common_js_modules()
        {
            return None;
        }
        let paths: IndexMap<_, _> = jobs
            .iter()
            .enumerate()
            .map(|(i, job)| (job.input.get_name().trim_start_matches("./").to_owned(), i))
            .collect();
        let mut pending = PendingImports {
            pending: VecDeque::new(),
            seen: vec![false; jobs.len()],
        };
        for entry in deps.entry_points() {
            pending.enqueue(Self::job_for_path(&paths, entry.name())?);
        }
        for (i, job) in jobs.iter().enumerate() {
            if job.input.get_source_file().is_extern() {
                pending.enqueue(i);
            }
        }
        Some(Self {
            paths,
            state: Mutex::new(pending),
            ready: Condvar::new(),
        })
    }

    fn job_for_path(paths: &IndexMap<String, usize>, path: &str) -> Option<usize> {
        let path = path.trim_start_matches("./");
        paths
            .get(path)
            .copied()
            .or_else(|| paths.get(&format!("{path}.js")).copied())
    }

    fn next(&self, stop: &AtomicBool) -> Option<usize> {
        let mut state = self.state.lock().unwrap();
        loop {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(i) = state.pending.pop_front() {
                return Some(i);
            }
            state = self.ready.wait(state).unwrap();
        }
    }

    fn follow(&self, filename: &str, parsed: &Preparsed) {
        let ast = &parsed.ast;
        let Some(body) = parsed
            .result
            .ast
            .and_then(|script| script.get_first_child(ast))
            .filter(|body| body.is_module_body(ast))
        else {
            return;
        };
        let mut imports = Vec::new();
        for node in body.children(ast) {
            if !(node.is_import(ast) || (node.is_export(ast) && node.has_two_children(ast))) {
                continue;
            }
            let Some(name) = node.get_last_child(ast).and_then(|n| n.string_payload(ast)) else {
                continue;
            };
            let path = name.to_string_lossy();
            if (path.starts_with("./") || path.starts_with("../"))
                && let Some(i) =
                    Self::job_for_path(&self.paths, &Compiler::resolve_sibling(filename, &path))
            {
                imports.push(i);
            }
        }
        self.enqueue(imports.into_iter());
    }

    fn enqueue(&self, mut jobs: impl Iterator<Item = usize>) {
        let Some(first) = jobs.next() else { return };
        let mut state = self.state.lock().unwrap();
        let was_empty = state.pending.is_empty();
        state.enqueue(first);
        for i in jobs {
            state.enqueue(i);
        }
        if was_empty && !state.pending.is_empty() {
            drop(state);
            self.ready.notify_all();
        }
    }
}

impl Preparser {
    /// The compiler's resolver also discovers Closure imports, type-only imports and aliases.
    pub(crate) fn schedule_imports<'a>(&self, inputs: impl Iterator<Item = &'a CompilerInput>) {
        if let Some(imports) = &self.shared.imports {
            imports.enqueue(inputs.filter_map(|input| self.jobs.get(input).copied()));
        }
    }
}

impl PendingImports {
    fn enqueue(&mut self, i: usize) {
        if !std::mem::replace(&mut self.seen[i], true) {
            self.pending.push_back(i);
        }
    }
}

struct Job {
    input: CompilerInput,
    config: Config,
    kind: SourceKind,
}

enum Slot {
    Waiting,
    Parsing,
    Parsed(Option<Box<Preparsed>>),
    Taken,
}

/// One input parsed on a worker thread.
struct Preparsed {
    ast: Ast,
    result: ParseResult,
    errors: Vec<JSError>,
    code: JsString,
}

/// Starts parsing `inputs` that have no AST yet on worker threads, unless there is too little
/// to do or the machine is busy.
pub(crate) fn start(compiler: &mut AbstractCompiler, inputs: &[CompilerInput]) {
    finish(compiler);
    if !*PARALLEL_ENABLED {
        return;
    }
    let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let threads = cpus.min(MAX_THREADS);
    // ASTs that come from a TypedAST filesystem are not parsed.
    if threads < 2
        || inputs.len() < 2
        || compiler.has_typed_ast_filesystem()
        || machine_is_busy(cpus)
    {
        return;
    }
    let mut jobs = Vec::with_capacity(inputs.len());
    let mut job_of = IndexMap::<_, _>::default();
    for input in inputs {
        if input.is_parsed() || job_of.contains_key(input) {
            continue;
        }
        let source_file = input.get_source_file();
        let context = if source_file.is_extern() {
            ConfigContext::EXTERNS
        } else {
            ConfigContext::DEFAULT
        };
        let config = compiler.get_parser_config(context);
        if source_file.is_extern() {
            let Ok(code) = source_file.read_source_uncached() else {
                continue;
            };
            if source_file.extern_snapshot(&config, &code).is_some() {
                continue;
            }
        }
        let kind = StaticSourceFile::get_kind(source_file);
        job_of.insert(input.clone(), jobs.len());
        jobs.push(Job {
            input: input.clone(),
            config,
            kind,
        });
    }
    if jobs.len() < 2 {
        return;
    }
    silence_worker_panics();
    let shared = Arc::new(Shared {
        imports: ImportQueue::for_inputs(compiler, &jobs, threads),
        slots: jobs
            .iter()
            .map(|_| (Mutex::new(Slot::Waiting), Condvar::new()))
            .collect(),
        jobs,
        next: AtomicUsize::new(0),
        stop: AtomicBool::new(false),
    });
    // A worker that cannot be started leaves its share to the others or to the compiler.
    let workers = (0..threads.min(shared.jobs.len()))
        .filter_map(|_| {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name(THREAD_NAME.into())
                .stack_size(crate::compiler_executor::COMPILER_STACK_SIZE)
                .spawn(move || work(&shared))
                .ok()
        })
        .collect();
    compiler.preparser = Some(Preparser {
        shared,
        jobs: job_of,
        workers,
    });
}

/// Rust-only: whether other threads already keep the CPUs busy, so that parse workers would only
/// compete with them (and with each other) for CPUs and caches: at least half as many runnable
/// threads as the CPUs available to this process, not counting this one (Linux `/proc/loadavg`;
/// elsewhere never). Parsing on the compiler thread gives the same output.
fn machine_is_busy(cpus: usize) -> bool {
    let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") else {
        return false;
    };
    let running = loadavg
        .split_whitespace()
        .nth(3)
        .and_then(|field| field.split('/').next())
        .and_then(|n| n.parse::<usize>().ok());
    running.is_some_and(|running| running.saturating_sub(1) * 2 >= cpus)
}

fn work(shared: &Shared) {
    let mut configs: Vec<(Config, Config)> = Vec::new();
    while !shared.stop.load(Ordering::Relaxed) {
        let i = if let Some(imports) = &shared.imports {
            let Some(i) = imports.next(&shared.stop) else {
                break;
            };
            i
        } else {
            shared.next.fetch_add(1, Ordering::Relaxed)
        };
        let Some(job) = shared.jobs.get(i) else {
            break;
        };
        let (slot, ready) = &shared.slots[i];
        {
            let mut slot = slot.lock().unwrap();
            if !matches!(*slot, Slot::Waiting) {
                continue; // The compiler parses it itself.
            }
            *slot = Slot::Parsing;
        }
        let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            closure_rhino::rhino_string_pool::RhinoStringPool::with_thread_cache(|| {
                let parsed = parse(job, &mut configs)?;
                if let Some(imports) = &shared.imports {
                    imports.follow(job.input.get_name(), &parsed);
                }
                Some(parsed)
            })
        }))
        .ok()
        .flatten()
        .map(Box::new);
        *slot.lock().unwrap() = Slot::Parsed(parsed);
        ready.notify_all();
    }
}

/// Stops the workers and drops what they parsed and the compiler did not take.
pub(crate) fn finish(compiler: &mut AbstractCompiler) {
    if let Some(preparser) = compiler.preparser.take() {
        preparser.shared.stop.store(true, Ordering::Relaxed);
        if let Some(imports) = &preparser.shared.imports {
            let _state = imports.state.lock().unwrap();
            imports.ready.notify_all();
        }
        for worker in preparser.workers {
            let _ = worker.join();
        }
    }
}

// port: CompilerInput.JsAst#parse (the ParserRunner call, into an arena of its own)
fn parse(job: &Job, configs: &mut Vec<(Config, Config)>) -> Option<Preparsed> {
    let code = job.input.get_source_file().load_code_uncached().ok()?;
    let mut ast = Ast::new_for_preparse();
    let index = configs
        .iter()
        .position(|(config, _)| *config == job.config)
        .unwrap_or_else(|| {
            configs.push((job.config.clone(), job.config.unshared_copy()));
            configs.len() - 1
        });
    let unshared_config = &configs[index].1;
    let mut errors = crate::rhino_error_reporter::RecordingErrorHandler::default();
    let mut reporter = crate::rhino_error_reporter::RhinoErrorReporter::for_old_rhino(&mut errors);
    let result = ParserRunner::try_parse(
        &mut ast,
        job.input.get_source_file_arc(),
        code.clone(),
        unshared_config,
        &mut reporter,
    )
    .ok()?;
    Some(Preparsed {
        ast,
        result,
        errors: errors.errors,
        code,
    })
}

/// The preparsed AST of `input`, moved into the compiler's arena, with the errors its parse
/// reported, if `input` was preparsed with this `config`, its current source kind and `code`.
pub(crate) fn take(
    compiler: &mut AbstractCompiler,
    input: &CompilerInput,
    config: &Config,
    code: &JsString,
) -> Option<(ParseResult, Vec<JSError>)> {
    let preparser = compiler.preparser.as_ref()?;
    let i = *preparser.jobs.get(input)?;
    let job = &preparser.shared.jobs[i];
    let kind = StaticSourceFile::get_kind(input.get_source_file());
    if job.config != *config || job.kind != kind {
        return None;
    }
    let (slot, ready) = &preparser.shared.slots[i];
    let mut slot = slot.lock().unwrap();
    while matches!(*slot, Slot::Parsing) {
        slot = ready.wait(slot).unwrap();
    }
    // Waiting: no worker has started it, so the compiler parses it now.
    let Slot::Parsed(preparsed) = std::mem::replace(&mut *slot, Slot::Taken) else {
        return None;
    };
    drop(slot);
    let preparsed = preparsed?;
    if preparsed.code != *code {
        return None;
    }
    let map = compiler.ast.append_preparsed(preparsed.ast);
    let mut result = preparsed.result;
    result.ast = result.ast.map(|root| map.map(root));
    Some((result, preparsed.errors))
}

/// A panic on a worker only means that the compiler parses that input itself (and meets the
/// same panic there, reported as usual), so the panic hook stays silent for workers.
fn silence_worker_panics() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if std::thread::current().name() != Some(THREAD_NAME) {
                previous(info);
            }
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_options::{CompilerOptions, LanguageMode};
    use crate::dependency_options::DependencyOptions;
    use crate::module_identifier::ModuleIdentifier;
    use crate::source_file::SourceFile;

    fn job(config: &Config, name: &str, text: &str) -> Job {
        let input = CompilerInput::new(SourceFile::from_code(name, text));
        Job {
            kind: StaticSourceFile::get_kind(input.get_source_file()),
            input,
            config: config.clone(),
        }
    }

    #[test]
    fn workers_follow_imports_and_reexports_once() {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
        options.set_dependency_options(DependencyOptions::prune_for_entry_points([
            ModuleIdentifier::for_file("./src/main"),
        ]));
        compiler.init_options(options);
        let config = compiler.get_parser_config(ConfigContext::DEFAULT);
        let jobs = [
            job(
                &config,
                "src/main.js",
                "import './lib/a'; export * from './b.js';",
            ),
            job(
                &config,
                "src/lib/a.js",
                "import '../main.js'; import '../b.js';",
            ),
            job(&config, "src/b.js", "export default 1;"),
            job(&config, "src/unused.js", "throw 2;"),
        ];
        let queue = ImportQueue::for_inputs(&compiler, &jobs, 2).unwrap();
        let stop = AtomicBool::new(false);
        let mut configs = Vec::new();
        for expected in 0..3 {
            assert_eq!(queue.state.lock().unwrap().pending.front(), Some(&expected));
            let i = queue.next(&stop).unwrap();
            queue.follow(
                jobs[i].input.get_name(),
                &parse(&jobs[i], &mut configs).unwrap(),
            );
        }
        let state = queue.state.lock().unwrap();
        assert!(state.pending.is_empty());
        assert!(!state.seen[3]);
        assert!(ImportQueue::for_inputs(&compiler, &jobs, jobs.len()).is_none());
    }

    #[test]
    fn a_source_edit_invalidates_a_finished_worker_parse() {
        let mut compiler = Compiler::new();
        compiler.init_compiler_options_if_testing();
        let config = compiler.get_parser_config(ConfigContext::DEFAULT);
        let job = job(&config, "input.js", "var oldCode;");
        let input = job.input.clone();
        let parsed = parse(&job, &mut Vec::new()).unwrap();
        compiler.preparser = Some(Preparser {
            shared: Arc::new(Shared {
                jobs: vec![job],
                next: AtomicUsize::new(1),
                imports: None,
                stop: AtomicBool::new(false),
                slots: vec![(
                    Mutex::new(Slot::Parsed(Some(Box::new(parsed)))),
                    Condvar::new(),
                )],
            }),
            jobs: [(input.clone(), 0)].into_iter().collect(),
            workers: Vec::new(),
        });
        input.get_source_file().set_code_deprecated("var newCode;");
        let current = input.get_source_file().get_code().unwrap();
        let count = compiler.node_count();
        assert!(take(&mut compiler, &input, &config, &current).is_none());
        assert_eq!(compiler.node_count(), count);
        finish(&mut compiler);
    }
}
