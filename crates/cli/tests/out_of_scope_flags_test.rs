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

//! The flags and inputs that turn on parts of Closure Compiler outside the port are refused
//! (DECISIONS.md D-028), and the same flags with their inactive values compile as the Java
//! compiler does. The expected outputs of the inactive cases were recorded from the Java compiler
//! at the pinned release.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CODE: &str = "function f(a){ return a + 1; }\nconsole.log(f(2));\n";
const SIMPLE_OUT: &str = "function f(a){return a+1}console.log(f(2));\n";

/// A directory of the test's own (the tests run in parallel) with the input files.
fn dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("out-of-scope-flags-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.js"), CODE).unwrap();
    std::fs::write(dir.join("b.java.js"), CODE).unwrap();
    std::fs::write(dir.join("c.js"), "console.log(0);\n").unwrap();
    std::fs::write(
        dir.join("typed.java.js"),
        "/** @type {number} */ var n = \"s\";\n",
    )
    .unwrap();
    dir
}

/// Runs closure-rs in the test directory: (exit code, stdout, stderr).
fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_closure-rs"))
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn refused(what: &str, reason: &str) -> String {
    format!("closure-rs does not support {what}: {reason} not part of the port.\n")
}

fn j2cl_refused(input: &str) -> String {
    format!(
        "closure-rs does not support the J2CL passes: they are not part of the port. \
         --j2cl_pass=AUTO (the default) runs them for J2CL output, an input whose name ends in \
         .java.js ({input}). Use --j2cl_pass=OFF to compile it without them.\n"
    )
}

#[test]
fn activating_flags_are_refused() {
    let dir = dir("activating");
    let instrumentation = |value: &str| {
        refused(
            &format!("--instrument_for_coverage_option={value}"),
            "code coverage instrumentation is",
        )
    };
    let polymer = refused("--polymer_version", "the Polymer passes are");
    let chrome = refused(
        "--chrome_pass",
        "the Chrome coding convention and passes are",
    );
    let typed_ast = refused("--typed_ast_output_file", "writing TypedAST files is");
    let cases: Vec<(Vec<&str>, String)> = vec![
        (
            vec!["--instrument_for_coverage_option=LINE"],
            instrumentation("LINE"),
        ),
        (
            vec!["--instrument_for_coverage_option=branch"],
            instrumentation("branch"),
        ),
        (
            vec![
                "--instrument_for_coverage_option=PRODUCTION",
                "--instrument_mapping_report=mapping.txt",
                "--production_instrumentation_array_name=arr",
            ],
            instrumentation("PRODUCTION"),
        ),
        // Refused in every compilation level, even where the Java compiler would not use it.
        (
            vec![
                "--instrument_for_coverage_option=LINE",
                "--compilation_level=WHITESPACE_ONLY",
            ],
            instrumentation("LINE"),
        ),
        (vec!["--polymer_version=1"], polymer.clone()),
        (vec!["--polymer_version=2"], polymer.clone()),
        (vec!["--chrome_pass"], chrome.clone()),
        (vec!["--chrome_pass=true"], chrome.clone()),
        (vec!["--chrome_pass", "--third_party"], chrome.clone()),
        (
            vec!["--typed_ast_output_file=out.typedast"],
            typed_ast.clone(),
        ),
        // Java serializes for the empty name too (and fails to create the file).
        (vec!["--typed_ast_output_file="], typed_ast.clone()),
        (
            vec![
                "--polymer_version=1",
                "--chrome_pass",
                "--typed_ast_output_file=x",
            ],
            format!("{polymer}{chrome}{typed_ast}"),
        ),
    ];
    for (flags, stderr) in cases {
        let mut args = vec!["--js=a.js", "--js_output_file=out.js"];
        args.extend(&flags);
        assert_eq!(run(&dir, &args), (255, String::new(), stderr), "{flags:?}");
        assert!(!dir.join("out.js").exists(), "{flags:?}");
        assert!(!dir.join("out.typedast").exists(), "{flags:?}");
    }
}

#[test]
fn j2cl_input_is_refused_when_the_j2cl_passes_would_run() {
    let dir = dir("j2cl");
    for flags in [
        vec![],
        vec!["--j2cl_pass=AUTO"],
        vec!["--j2cl_pass="],
        vec!["--remove_j2cl_asserts"],
        vec!["--compilation_level=ADVANCED"],
        vec!["--js=c.js", "--chunk=m0:1", "--chunk=m1:1:m0"],
    ] {
        let mut args = vec!["--js=b.java.js"];
        args.extend(&flags);
        assert_eq!(
            run(&dir, &args),
            (255, String::new(), j2cl_refused("b.java.js")),
            "{flags:?}"
        );
    }
    // A warning promoted to an error does not halt the compilation, so the Java compiler runs the
    // J2CL passes after these checks.
    assert_eq!(
        run(
            &dir,
            &[
                "--js=typed.java.js",
                "-O",
                "ADVANCED",
                "--jscomp_error=checkTypes"
            ]
        ),
        (255, String::new(), j2cl_refused("typed.java.js"))
    );
}

#[test]
fn inactive_values_compile_as_in_java() {
    let dir = dir("inactive");
    let simple = (0, SIMPLE_OUT.to_string(), String::new());
    let a_js: &[&[&str]] = &[
        &["--instrument_for_coverage_option=NONE"],
        &["--instrument_for_coverage_option=none"],
        &["--instrument_mapping_report="],
        &["--production_instrumentation_array_name="],
        &["--production_instrumentation_array_name=arr"],
        &["--chrome_pass=false"],
        &["--j2cl_pass=OFF"],
        &["--j2cl_pass=AUTO"],
        &["--j2cl_pass="],
        &["--remove_j2cl_asserts"],
    ];
    for flags in a_js {
        let mut args = vec!["--js=a.js"];
        args.extend(*flags);
        assert_eq!(run(&dir, &args), simple, "{flags:?}");
    }
    // J2CL input where the Java compiler runs no J2CL pass that is outside the port.
    let j2cl: &[(&[&str], &str)] = &[
        (&["--j2cl_pass=OFF"], SIMPLE_OUT),
        (&["--j2cl_pass=off", "--remove_j2cl_asserts"], SIMPLE_OUT),
        (&["-O", "ADVANCED", "--j2cl_pass=OFF"], "console.log(3);\n"),
        (&["--compilation_level=WHITESPACE_ONLY"], SIMPLE_OUT),
        (&["--checks_only"], ""),
    ];
    for (flags, stdout) in j2cl {
        let mut args = vec!["--js=b.java.js"];
        args.extend(*flags);
        assert_eq!(
            run(&dir, &args),
            (0, stdout.to_string(), String::new()),
            "{flags:?}"
        );
    }
    // The Java compiler's own flag errors come before the refusal.
    assert_eq!(
        run(&dir, &["--js=a.js", "--instrument_mapping_report=m"]),
        (
            255,
            String::new(),
            "Expected --instrument_for_coverage_option to be passed with PRODUCTION when \
             --instrument_mapping_report is set\n"
                .to_string()
        )
    );
    assert_eq!(
        run(
            &dir,
            &["--js=a.js", "--instrument_for_coverage_option=PRODUCTION"]
        ),
        (
            255,
            String::new(),
            "Expected --instrument_mapping_report to be set when \
             --instrument_for_coverage_option is set to Production\n"
                .to_string()
        )
    );
}
