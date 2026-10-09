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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CompilerInput.java.

//! Rust-only, not in Java (DECISIONS.md D-025): parses the inputs of a compilation on worker
//! threads while the compiler asks for their ASTs one by one.
//!
//! Java parses each input when `CompilerInput#getAstRoot` first needs it, on the compiler thread
//! (or, with `--num_parallel_threads`, in `PrebuildAst`). Here `start` sets worker threads to
//! parse every input that the compiler may ask for, in order, each into an arena of its own
//! (`Ast::new_for_preparse`). When `CompilerInput#parse` then runs for an input, `take` waits for
//! that input's parse and moves its nodes into the compiler's arena (`Ast::append_preparsed`)
//! instead of parsing, provided the parse used the same code, parser configuration and source
//! kind that parsing now would. The appended nodes get the ids, and the errors the order, that
//! parsing at that point would have given them, so the compilation is unchanged. An input no
//! worker has started yet, or whose parse failed or panicked on a worker, is parsed by the
//! compiler itself; inputs the compiler never asks for are dropped by `finish`.

use crate::{
    abstract_compiler::{AbstractCompiler, ConfigContext},
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
use std::sync::{
    Arc, Condvar, Mutex, Once,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// The most worker threads one compilation uses.
const MAX_THREADS: usize = 8;
const THREAD_NAME: &str = "closure-preparse";

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
    stop: AtomicBool,
    slots: Vec<(Mutex<Slot>, Condvar)>,
}

struct Job {
    input: CompilerInput,
    config: Config,
    kind: SourceKind,
    code: JsString,
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
}

/// Starts parsing `inputs` that have no AST yet on worker threads, unless there is too little
/// to do.
pub(crate) fn start(compiler: &mut AbstractCompiler, inputs: &[CompilerInput]) {
    finish(compiler);
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(MAX_THREADS);
    // ASTs that come from a TypedAST filesystem are not parsed.
    if threads < 2 || inputs.len() < 2 || compiler.has_typed_ast_filesystem() {
        return;
    }
    let mut jobs = Vec::with_capacity(inputs.len());
    let mut job_of = IndexMap::<_, _>::default();
    for input in inputs {
        if input.is_parsed() || job_of.contains_key(input) {
            continue;
        }
        let source_file = input.get_source_file();
        let Ok(code) = source_file.load_code_uncached() else {
            continue;
        };
        let context = if source_file.is_extern() {
            ConfigContext::EXTERNS
        } else {
            ConfigContext::DEFAULT
        };
        let config = compiler.get_parser_config(context);
        let kind = StaticSourceFile::get_kind(source_file);
        job_of.insert(input.clone(), jobs.len());
        jobs.push(Job {
            input: input.clone(),
            config,
            kind,
            code,
        });
    }
    if jobs.len() < 2 {
        return;
    }
    silence_worker_panics();
    let shared = Arc::new(Shared {
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

fn work(shared: &Shared) {
    while !shared.stop.load(Ordering::Relaxed) {
        let i = shared.next.fetch_add(1, Ordering::Relaxed);
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
            closure_rhino::rhino_string_pool::RhinoStringPool::with_thread_cache(|| parse(job))
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
        for worker in preparser.workers {
            let _ = worker.join();
        }
    }
}

// port: CompilerInput.JsAst#parse (the ParserRunner call, into an arena of its own)
fn parse(job: &Job) -> Option<Preparsed> {
    let mut ast = Ast::new_for_preparse();
    let unshared_config = job.config.unshared_copy();
    let mut errors = crate::rhino_error_reporter::RecordingErrorHandler::default();
    let mut reporter = crate::rhino_error_reporter::RhinoErrorReporter::for_old_rhino(&mut errors);
    let result = ParserRunner::try_parse(
        &mut ast,
        job.input.get_source_file_arc(),
        job.code.clone(),
        &unshared_config,
        &mut reporter,
    )
    .ok()?;
    Some(Preparsed {
        ast,
        result,
        errors: errors.errors,
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
    if job.config != *config || job.kind != kind || job.code != *code {
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
