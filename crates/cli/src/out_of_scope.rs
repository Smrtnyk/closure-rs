/*
 * Copyright 2026 The closure-rs Authors.
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

//! Not in Java: closure-rs stops, instead of compiling, when a flag or the input turns on a part of
//! Closure Compiler that is outside the port (DECISIONS.md D-028; the flags are the `out` rows of
//! `scope/flags.txt`). The refusal is a `FlagUsageException`, so it is reported like the Java
//! compiler's own flag errors: the message alone on stderr, exit code 255.
use crate::{abstract_command_line_runner::FlagUsageException, command_line_runner::Flags};
use closure_jscomp::{compiler::Compiler, j2cl_source_utils::J2clSourceUtils};

fn unsupported(what: &str, reason: &str) -> String {
    format!("closure-rs does not support {what}: {reason} not part of the port.")
}

/// The flags given with a value that turns on an out-of-scope feature, one message line each.
/// Checked once the options are created, before the inputs are read, so the Java compiler's own
/// flag errors (for example `--instrument_for_coverage_option=PRODUCTION` without
/// `--instrument_mapping_report`) come first and stay as they are. The other out-of-scope flags
/// only configure these features: `--instrument_mapping_report` and
/// `--production_instrumentation_array_name` are read only by PRODUCTION instrumentation, and
/// `--remove_j2cl_asserts` only by the J2CL passes ([`refuse_j2cl_passes`]).
pub fn refuse_flags(flags: &Flags) -> Result<(), FlagUsageException> {
    let mut messages = Vec::new();
    if flags.instrument_code_parsed != closure_jscomp::compiler_options::InstrumentOption::NONE {
        messages.push(unsupported(
            &format!(
                "--instrument_for_coverage_option={}",
                flags.instrument_for_coverage_option
            ),
            "code coverage instrumentation is",
        ));
    }
    if flags.polymer_version.is_some() {
        messages.push(unsupported("--polymer_version", "the Polymer passes are"));
    }
    if flags.chrome_pass {
        messages.push(unsupported(
            "--chrome_pass",
            "the Chrome coding convention and passes are",
        ));
    }
    // Java turns serialization on for any value, the empty string included.
    if flags.typed_ast_output_file.is_some() {
        messages.push(unsupported(
            "--typed_ast_output_file",
            "writing TypedAST files is",
        ));
    }
    if messages.is_empty() {
        Ok(())
    } else {
        Err(FlagUsageException(messages.join("\n")))
    }
}

/// The J2CL passes run when `--j2cl_pass` is AUTO (the default) and the checks found an input
/// whose name ends in `.java.js` (`J2clSourceFileChecker`); every J2CL pass returns at once
/// otherwise. Only the optimizations hold J2CL passes that are not ported, so this is checked
/// before they would run: a J2CL input still compiles with `--checks_only` or WHITESPACE_ONLY,
/// as the Java compiler runs no unported code then.
pub fn refuse_j2cl_passes(compiler: &Compiler) -> Result<(), FlagUsageException> {
    if !compiler.run_j2cl_passes() || !compiler.get_options().should_optimize() {
        return Ok(());
    }
    let input = compiler
        .get_inputs_in_order()
        .into_iter()
        .map(|input| input.get_source_file().get_original_path().to_string())
        .find(|name| J2clSourceUtils::is_j2cl_source(Some(name)));
    let input = input.map_or_else(String::new, |name| format!(" ({name})"));
    Err(FlagUsageException(format!(
        "{} --j2cl_pass=AUTO (the default) runs them for J2CL output, an input whose name ends in \
         .java.js{input}. Use --j2cl_pass=OFF to compile it without them.",
        unsupported("the J2CL passes", "they are")
    )))
}
