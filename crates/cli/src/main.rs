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
//   src/com/google/javascript/jscomp/CommandLineRunner.java.

#![forbid(unsafe_code)]
use closure_cli::{
    abstract_command_line_runner::SystemExitCodeReceiver, command_line_runner::CommandLineRunner,
    java_frames,
};

// Not in Java (the JVM has its own allocator): mimalloc is much faster than the system allocator
// for the compiler's many small, short-lived allocations. It changes no output (crates/DESIGN.md,
// "Performance divergences").
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// port: CommandLineRunner#main
// Returns the exit code instead of calling std::process::exit as Java calls System.exit: on
// Windows process::exit ends the process without running the C runtime's exit handlers, and the
// profile of an instrumented build (scripts/pgo_build.sh) is written by one of them.
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Rust-only (performance divergence): the whole run happens on one thread of the compiler's
    // stack size, so that the compiler's stages run in place instead of each on a thread of its
    // own.
    let code = closure_jscomp::compiler_executor::CompilerExecutor::run_process_on_compiler_thread(
        move || run(&args),
    );
    // SystemExitCodeReceiver::apply gives 0..=255
    std::process::ExitCode::from(SystemExitCodeReceiver::apply(code) as u8)
}

/// The body of `CommandLineRunner#main` up to `System.exit`: the exit code.
fn run(args: &[String]) -> i32 {
    let runner = match CommandLineRunner::new_system(args) {
        Ok(runner) => runner,
        Err(error) => {
            eprintln!(
                "Exception in thread \"main\" com.google.javascript.jscomp.FlagUsageException: {error}\n\tat {}\n\tat {}\n\tat {}",
                java_frames::COMMAND_LINE_RUNNER_INIT_CONFIG_FLAG_USAGE,
                java_frames::COMMAND_LINE_RUNNER_INIT_INIT_CONFIG,
                java_frames::COMMAND_LINE_RUNNER_MAIN_NEW,
            );
            return 1;
        }
    };
    runner.run_for_process_exit()
}
