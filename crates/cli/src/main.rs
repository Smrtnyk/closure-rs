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
//   src/com/google/javascript/jscomp/CommandLineRunner.java.

#![forbid(unsafe_code)]
use closure_cli::{
    abstract_command_line_runner::SystemExitCodeReceiver, command_line_runner::CommandLineRunner,
};
// port: CommandLineRunner#main
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut runner = match CommandLineRunner::new_system(&args) {
        Ok(runner) => runner,
        Err(error) => {
            eprintln!(
                "Exception in thread \"main\" com.google.javascript.jscomp.FlagUsageException: {error}\n\tat com.google.javascript.jscomp.CommandLineRunner.initConfigFromFlags(CommandLineRunner.java:1657)\n\tat com.google.javascript.jscomp.CommandLineRunner.<init>(CommandLineRunner.java:1479)\n\tat com.google.javascript.jscomp.CommandLineRunner.main(CommandLineRunner.java:2254)"
            );
            std::process::exit(1);
        }
    };
    let mut code = 0;
    if runner.should_run_compiler() {
        code = runner.run();
    }
    if runner.has_errors() {
        code = -1;
    }
    use std::io::Write;
    let _ = runner.base.default_js_output.flush();
    let _ = runner.base.err.flush();
    std::process::exit(SystemExitCodeReceiver::apply(code));
}
