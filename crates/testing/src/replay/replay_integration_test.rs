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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/integration/ReplayIntegrationTest.java.

use crate::{
    corpus::LoadedRecord,
    descriptor::Case,
    integration::integration_test_case::IntegrationTestCase,
    json::JsonValue,
    record::{Api, Expected},
    replay::{
        registry::Registry,
        replay_bridge::{self, diagnostic_group, errors, source_files},
        replay_dsl::Ctx,
        replay_main::ReplayResult,
    },
    throwable::Throwable,
};
use closure_rhino::js_string::JsString;
use indexmap::IndexMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
// port: ReplayIntegrationTest#replay
pub fn replay(
    stem: &str,
    record: &LoadedRecord,
    case: Option<&Case>,
    class_map: IndexMap<String, String>,
    registry: Registry,
) -> Result<ReplayResult, Throwable> {
    let r = &record.record;
    let mut t =
        IntegrationTestCase::set_up(source_files(r.inputs.externs.as_deref().unwrap_or(&[])));
    if let Some(prefix) = &r.inputs.input_file_name_prefix {
        t.input_file_name_prefix = prefix.clone();
    }
    if let Some(suffix) = &r.inputs.input_file_name_suffix {
        t.input_file_name_suffix = suffix.clone();
    }
    let mut ctx = Ctx::new(stem.into(), record.raw.clone(), class_map, registry);
    let options = replay_bridge::options_with_case(&r.options, case, &mut ctx)?;
    let originals = strings(r.inputs.originals.as_deref().unwrap_or(&[]));
    let (compiled, groups) = match &r.expected {
        Some(Expected::Integration {
            output,
            diagnostic_groups,
        }) => (
            output.as_ref().map(|v| strings(v)),
            diagnostic_groups
                .as_ref()
                .map(|v| v.iter().map(diagnostic_group).collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
        None => (None, vec![]),
        _ => return Err(Throwable::HarnessError("bad integration expected".into())),
    };
    let run = catch_unwind(AssertUnwindSafe(|| {
        ReplayResult::catch_precondition_run(&record.raw, || match r.api {
            Api::Test => t.test(options, &originals, compiled.as_deref()),
            Api::TestWarning => t.test_warning(
                options,
                &originals,
                compiled.as_deref(),
                groups
                    .first()
                    .cloned()
                    .ok_or_else(|| Throwable::HarnessError("no diagnostic group".into()))?,
            ),
            Api::TestWarnings => t.test_warnings(options, &originals, compiled.as_deref(), &groups),
            Api::TestNoWarnings => t.test_no_warnings(options, &originals),
            Api::TestParseError => t.test_parse_error(options, &originals, compiled.as_deref()),
            Api::Compile => {
                let chunks = replay_bridge::chunks(
                    r.inputs
                        .chunks
                        .as_deref()
                        .ok_or_else(|| Throwable::HarnessError("compile has no chunks".into()))?,
                )?;
                t.compile_chunks(options, &chunks).map(|_| ())
            }
            _ => Err(Throwable::HarnessError("bad integration api".into())),
        })
    }));
    let omitted_unported = first_omitted_unported(&t);
    let mut result = match run {
        Ok(result) => result?,
        // An exception Java did not throw: with an omitted unported pass the runner reports the
        // record `unported`; without one the panic continues.
        Err(payload) if omitted_unported.is_some() => {
            let mut result = ReplayResult::from_panic(payload.as_ref());
            result.omitted_unported = omitted_unported;
            return Ok(result);
        }
        Err(payload) => std::panic::resume_unwind(payload),
    };
    result.omitted_unported = omitted_unported;
    if let Some(c) = t.last_compiler {
        let mut c = c.borrow_mut();
        let output = if c.get_root().is_none() {
            JsonValue::Null
        } else {
            JsonValue::String(crate::json::JsString(
                crate::harness_passes::to_source_all(&mut c)?
                    .as_units()
                    .to_vec(),
            ))
        };
        result.observed = crate::replay::replay_values::object([
            ("errors", errors(&c.get_errors())),
            ("warnings", errors(&c.get_warnings())),
            ("output", output),
        ]);
    }
    Ok(result)
}
/// Rust-only: the Java name of the first unported pass factory the
/// last compile omitted.
fn first_omitted_unported(t: &IntegrationTestCase) -> Option<String> {
    let c = t.last_compiler.as_ref()?.try_borrow().ok()?;
    let class = c.get_omitted_unported_passes().first()?;
    Some(if class.starts_with("com.") {
        (*class).into()
    } else {
        format!("com.google.javascript.jscomp.{}", class.replace('.', "$"))
    })
}
// port: ReplayIntegrationTest#strings
fn strings(values: &[crate::json::JsString]) -> Vec<JsString> {
    values
        .iter()
        .map(|s| JsString::from_units(s.0.clone()))
        .collect()
}
