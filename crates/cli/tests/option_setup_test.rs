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

use closure_cli::command_line_runner::CommandLineRunner;
#[test]
fn option_setup_parity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cli_golden");
    // Java captures paths relative to this directory. This process has one setup test.
    std::env::set_current_dir(root).unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("data/option_setup.json")).unwrap();
    let model: serde_json::Value =
        serde_json::from_str(include_str!("../tools/options_model.json")).unwrap();
    let keys = model["modelled"].as_array().unwrap();
    assert_eq!(keys.len(), 186);
    assert_eq!(model["unmodelled"].as_array().unwrap().len(), 19);
    for row in &rows {
        let args: Vec<String> = serde_json::from_value(row["argv"].clone()).unwrap();
        let java = &row["setup"];
        let runner = CommandLineRunner::new(
            &args,
            Box::new(std::io::empty()),
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
        );
        let mut runner = match runner {
            Ok(runner) => runner,
            Err(error) => {
                assert_eq!(
                    java["exception"], "com.google.javascript.jscomp.FlagUsageException",
                    "argv={args:?}"
                );
                assert_eq!(java["message"], error.0, "argv={args:?}");
                continue;
            }
        };
        assert_eq!(
            java["shouldRunCompiler"],
            runner.should_run_compiler(),
            "argv={args:?}"
        );
        assert_eq!(java["hasErrors"], runner.has_errors(), "argv={args:?}");
        if !runner.should_run_compiler() {
            continue;
        }
        assert_eq!(
            java["config"],
            runner.base.config.to_json(),
            "config argv={args:?}"
        );
        runner.base.compiler = Some(runner.create_compiler());
        let result = runner.create_options().and_then(|mut options| {
            runner.base.set_run_options(&mut options)?;
            Ok(options)
        });
        let options = match result {
            Ok(options) => {
                assert!(
                    java.get("exception").is_none(),
                    "argv={args:?}, java={java}"
                );
                options
            }
            Err(error) => {
                assert_eq!(
                    java["exception"], "com.google.javascript.jscomp.FlagUsageException",
                    "argv={args:?}"
                );
                assert_eq!(java["message"], error.0, "argv={args:?}");
                continue;
            }
        };
        let rust = closure_cli::option_setup::diff_against_defaults(&options, &runner.base.config);
        let expected = serde_json::Value::Object(
            java["options"]
                .as_object()
                .unwrap()
                .iter()
                .filter(|(key, _)| keys.iter().any(|k| k.as_str() == Some(key.as_str())))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        assert_eq!(expected, rust, "options argv={args:?}");
    }
}
