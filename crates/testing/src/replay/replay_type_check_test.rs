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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayTypeCheckTest.java.

use crate::{
    compiler_type_test_case::CompilerTypeTestCase,
    corpus::LoadedRecord,
    descriptor::Case,
    record::{Api, Expected},
    replay::{
        registry::Registry,
        replay_dsl::{Ctx, eval_all},
        replay_main::ReplayResult,
        replay_options,
        replay_values::{
            diagnostic_group, diagnostic_type, errors, level, object, source_file, source_files,
        },
    },
    throwable::Throwable,
    type_check_test_case::{TypeTestBuilder, parse_and_type_check_with_scope},
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;
// port: ReplayTypeCheckTest#compiler
fn compiler(
    stem: &str,
    record: &LoadedRecord,
    case: Option<&Case>,
    class_map: IndexMap<String, String>,
    registry: Registry,
) -> Result<crate::replay::replay_dsl::CompilerHandle, Throwable> {
    let c = CompilerTypeTestCase::create_compiler()?;
    let mut ctx = Ctx::new(stem.into(), record.raw.clone(), class_map, registry);
    let options = replay_options::build(
        &record.record.options,
        case.and_then(|c| c.options.as_ref()),
        &mut ctx,
    )?;
    c.borrow_mut().init_options(options);
    c.borrow_mut()
        .mark_feature_not_allowed(closure_parsing::parser::feature_set::Feature::MODULES);
    ctx.compiler = Some(c.clone());
    eval_all(
        case.and_then(|c| c.compiler_setup.as_deref())
            .unwrap_or(&[]),
        &mut ctx,
    )?;
    Ok(c)
}
// port: ReplayTypeCheckTest#replay
pub fn replay(
    stem: &str,
    record: &LoadedRecord,
    case: Option<&Case>,
    class_map: IndexMap<String, String>,
    registry: Registry,
) -> Result<ReplayResult, Throwable> {
    let r = &record.record;
    let c = compiler(stem, record, case, class_map, registry)?;
    let mut result = ReplayResult::catch_precondition_run(&record.raw, || match r.api {
        Api::TypeTestBuilderRun => {
            let default = r
                .inputs
                .default_externs
                .as_ref()
                .map_or_else(|| JsString::from(""), |s| JsString::from_units(s.0.clone()));
            let mut b = TypeTestBuilder::new(c.clone(), default);
            b.sources = source_files(r.inputs.sources.as_deref().unwrap_or(&[]));
            b.externs = r
                .inputs
                .extern_strings
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|s| JsString::from_units(s.0.clone()))
                .collect();
            b.include_default_externs = r.inputs.include_default_externs.unwrap_or(false);
            b.diagnostics_are_errors = r.harness.diagnostics_are_errors.unwrap_or(false);
            b.report_unknown_types = r.harness.report_unknown_types.unwrap_or(false);
            b.suppress = r
                .harness
                .suppress
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(diagnostic_group)
                .collect();
            if let Some(Expected::TypeCheck {
                diagnostic_types,
                diagnostic_descriptions,
            }) = &r.expected
            {
                for t in diagnostic_types {
                    b.add_diagnostic(diagnostic_type(&t.key, level(&t.level)?));
                }
                for s in diagnostic_descriptions {
                    b.add_diagnostic_description(&s.to_string_lossy());
                }
            }
            b.run()
        }
        Api::ParseAndTypeCheckWithScope => {
            let externs = r
                .inputs
                .externs
                .as_ref()
                .and_then(|fs| fs.first())
                .ok_or_else(|| {
                    Throwable::HarnessError("direct type check has no externs".into())
                })?;
            let externs = source_file(externs)
                .get_code()
                .map_err(|e| Throwable::HarnessError(e.to_string()))?;
            parse_and_type_check_with_scope(
                c.clone(),
                externs,
                &source_files(r.inputs.sources.as_deref().unwrap_or(&[])),
                r.harness.report_unknown_types.unwrap_or(false),
            )
            .map(|_| ())
        }
        _ => Err(Throwable::HarnessError("invalid type_check api".into())),
    })?;
    result.observed = object([
        ("errors", errors(&c.borrow().get_errors())),
        ("warnings", errors(&c.borrow().get_warnings())),
    ]);
    Ok(result)
}
