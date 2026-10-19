/*
 * Copyright 2015 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 9954c6d:
//   src/com/google/javascript/jscomp/CompilerExecutor.java.

use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, ThreadId},
    time::Duration,
};
#[derive(Clone)]
pub struct CompilerExecutor {
    compiler_thread: Arc<Mutex<Option<ThreadId>>>,
    use_threads: bool,
    timeout: i32,
    debug_message: Option<String>,
}
pub const COMPILER_STACK_SIZE: usize = 1 << 26;
thread_local! {
    /// Rust-only (performance divergence): whether this thread is the one
    /// `run_process_on_compiler_thread` made, on which `run_in_compiler_thread` runs its callable
    /// in place.
    static PROCESS_COMPILER_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
impl Default for CompilerExecutor {
    fn default() -> Self {
        Self {
            compiler_thread: Arc::new(Mutex::new(None)),
            use_threads: true,
            timeout: 0,
            debug_message: None,
        }
    }
}
impl CompilerExecutor {
    // port: CompilerExecutor#getExecutorService
    pub fn get_executor_service(&self) -> thread::Builder {
        Self::get_default_executor_service()
    }
    // port: CompilerExecutor#getDefaultExecutorService
    // port: CompilerExecutor.<anonymous>#newThread
    pub fn get_default_executor_service() -> thread::Builder {
        thread::Builder::new()
            .name("jscompiler".into())
            .stack_size(COMPILER_STACK_SIZE)
    }
    // port: CompilerExecutor#disableThreads
    pub fn disable_threads(&mut self) {
        self.use_threads = false;
    }
    // port: CompilerExecutor#setTimeout
    pub fn set_timeout(&mut self, timeout: i32) {
        self.timeout = timeout;
    }
    // port: CompilerExecutor#setDebugMessage
    pub fn set_debug_message(&mut self, message: String) {
        self.debug_message = Some(message);
    }
    /// Rust-only, not in Java (performance divergence): runs `f` on a thread with the compiler's
    /// stack size and returns what it returns (a panic in `f` resumes here). While `f` runs, every
    /// `run_in_compiler_thread` on that thread runs its callable in place, with the same
    /// bookkeeping (the compiler thread id, the tracer's thread trace), instead of starting a
    /// thread of its own as Java's single-thread executor does for every call. With one heap
    /// for the whole run, the allocator reuses the memory a stage frees in the next stage,
    /// instead of abandoning each thread's heap when it exits and freeing its blocks from
    /// another thread. The thread stays parked after `f` so that its heap is never collected:
    /// the process ends soon after, and the OS reclaims the memory.
    pub fn run_process_on_compiler_thread<T: Send + 'static>(
        f: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        let (sender, receiver) = mpsc::channel();
        Self::get_default_executor_service()
            .spawn(move || {
                PROCESS_COMPILER_THREAD.with(|flag| flag.set(true));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
                let _ = sender.send(result);
                loop {
                    thread::park();
                }
            })
            .expect("Unable to start compiler thread");
        match receiver
            .recv()
            .expect("Compiler thread terminated without a result")
        {
            Ok(result) => result,
            Err(error) => std::panic::resume_unwind(error),
        }
    }
    /// The body of a compiler-thread call: `CompilerExecutor.<anonymous>#call` with the
    /// thread trace handling of `runInCompilerThread`.
    fn run_as_compiler_thread<T>(
        compiler_thread: &Mutex<Option<ThreadId>>,
        callable: impl FnOnce() -> T,
        dump_trace_report: bool,
    ) -> std::thread::Result<T> {
        *compiler_thread.lock().unwrap() = Some(thread::current().id());
        if dump_trace_report {
            crate::tracer::Tracer::init_current_thread_trace();
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callable));
        *compiler_thread.lock().unwrap() = None;
        if dump_trace_report {
            crate::tracer::Tracer::log_current_thread_trace();
        }
        crate::tracer::Tracer::clear_current_thread_trace();
        result
    }
    // port: CompilerExecutor#runInCompilerThread
    pub fn run_in_compiler_thread<T: Send>(
        &self,
        callable: impl FnOnce() -> T + Send,
        dump_trace_report: bool,
    ) -> T {
        let current = thread::current().id();
        let compiler_thread = *self.compiler_thread.lock().unwrap();
        assert!(
            compiler_thread.is_none() || compiler_thread == Some(current),
            "Please do not share the Compiler across threads"
        );
        let result = if self.use_threads
            && compiler_thread.is_none()
            && PROCESS_COMPILER_THREAD.with(|flag| flag.get())
        {
            // Rust-only (performance divergence): already on a thread of the compiler's stack size.
            Self::run_as_compiler_thread(&self.compiler_thread, callable, dump_trace_report)
        } else if self.use_threads && compiler_thread.is_none() {
            thread::scope(|scope| {
                let (sender, receiver) = mpsc::channel();
                let compiler_thread = self.compiler_thread.clone();
                let handle = self
                    .get_executor_service()
                    .spawn_scoped(scope, move || {
                        let result = Self::run_as_compiler_thread(
                            &compiler_thread,
                            callable,
                            dump_trace_report,
                        );
                        let _ = sender.send(result);
                    })
                    .expect("Unable to start compiler thread");
                let result = if self.timeout > 0 {
                    receiver
                        .recv_timeout(Duration::from_secs(self.timeout as u64))
                        .unwrap_or_else(|error| match error {
                            mpsc::RecvTimeoutError::Timeout => {
                                panic!("java.util.concurrent.TimeoutException")
                            }
                            mpsc::RecvTimeoutError::Disconnected => {
                                panic!("java.util.concurrent.ExecutionException")
                            }
                        })
                } else {
                    receiver
                        .recv()
                        .expect("Compiler thread terminated without a result")
                };
                handle.join().unwrap();
                result
            })
        } else {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(callable))
        };
        match result {
            Ok(result) => result,
            Err(error) => {
                if let Some(message) = &self.debug_message {
                    panic!("Exception during compilation: {message}");
                }
                std::panic::resume_unwind(error)
            }
        }
    }
}
