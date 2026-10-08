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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayCompilerTest.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! ReplayCompilerTest's record restore, overridable hooks and opt-in field projection.
#![allow(clippy::collapsible_if)] // Retain the Java branches.
use crate::{
    compiler_test_case::{
        CompilerTestCase, CompilerTestCaseHooks, Diagnostic, Expected, Externs, FlatSources,
        Sources,
    },
    corpus::LoadedRecord,
    derived::ExpectedPipeline,
    descriptor::Case,
    jscomp_api::{CheckLevel, CompilerOptions},
    json::JsonValue,
    record::{Api, Expected as RecordExpected, ExpectedOutput, MessageMode},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, eval, eval_all},
        replay_main::ReplayResult,
        replay_options,
        replay_values::{diagnostic_type, errors, level, source_files},
    },
    throwable::Throwable,
    unit_recorder,
};
use indexmap::IndexMap;
pub struct ReplayCompilerTest {
    pub harness: CompilerTestCase,
    pub hooks: Hooks,
}
pub struct Hooks {
    pub record: LoadedRecord,
    pub case: Case,
    pub ctx: Ctx,
    pub created: Option<CompilerHandle>,
    pub first_processor: Option<DslValue>,
}
impl CompilerTestCaseHooks for Hooks {
    // port: ReplayCompilerTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        self.ctx.compiler = Some(compiler);
        let expr = self
            .case
            .processor
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("case has no processor".into()))?;
        let p = eval(expr, &mut self.ctx)?;
        if self.first_processor.is_none() {
            self.first_processor = Some(p.clone());
        }
        Ok(p)
    }
    // port: ReplayCompilerTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let options = replay_options::build(
            &self.record.record.options,
            self.case.options.as_ref(),
            &mut self.ctx,
        )?;
        if let Some(fields) = &self.record.record.harness.fields_after_get_options {
            for (name, value) in fields {
                harness.restore_field(name, value, &self.ctx.class_map)?;
            }
        }
        Ok(options)
    }
    // port: ReplayCompilerTest#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        self.record
            .record
            .harness
            .effective
            .as_ref()
            .map_or(1, |e| e.get_num_repetitions)
    }
    // port: ReplayCompilerTest#getName
    fn get_name(&self) -> String {
        self.record
            .record
            .harness
            .effective
            .as_ref()
            .map_or_else(|| "ReplayCompilerTest".into(), |e| e.get_name.clone())
    }
    // port: ReplayCompilerTest#createCompiler
    fn create_compiler(&mut self, harness: &CompilerTestCase) -> Result<CompilerHandle, Throwable> {
        if let Some(e) = &self.record.record.harness.effective {
            if e.create_compiler != "com.google.javascript.jscomp.Compiler" {
                return Err(Throwable::Unported(e.create_compiler.clone()));
            }
        }
        let c = harness.create_compiler()?;
        if self.created.is_none() {
            self.created = Some(c.clone());
        }
        let saved = self.ctx.compiler.replace(c.clone());
        let result = eval_all(
            self.case.compiler_setup.as_deref().unwrap_or(&[]),
            &mut self.ctx,
        );
        self.ctx.compiler = saved;
        result?;
        Ok(c)
    }
    // port: ReplayDsl.Ctx#Ctx
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}
impl ReplayCompilerTest {
    // port: ReplayCompilerTest#ReplayCompilerTest
    pub fn new(
        stem: &str,
        record: &LoadedRecord,
        case: &Case,
        class_map: IndexMap<String, String>,
        registry: Registry,
    ) -> Self {
        Self {
            harness: CompilerTestCase::new(""),
            hooks: Hooks {
                record: record.clone(),
                case: case.clone(),
                ctx: Ctx::new(stem.into(), record.raw.clone(), class_map, registry),
                created: None,
                first_processor: None,
            },
        }
    }
    // port: ReplayCompilerTest#restoreHarness
    pub fn restore_harness(&mut self) -> Result<(), Throwable> {
        self.harness.set_up();
        if let Some(fields) = &self.hooks.record.record.harness.fields {
            for (name, value) in fields {
                self.harness
                    .restore_field(name, value, &self.hooks.ctx.class_map)?;
            }
        }
        Ok(())
    }
    // port: ReplayCompilerTest#run
    pub fn run(&mut self, pipeline: Option<&ExpectedPipeline>) -> Result<ReplayResult, Throwable> {
        self.restore_harness()?;
        let r = &self.hooks.record.record;
        let externs = Externs::new(source_files(r.inputs.externs.as_deref().unwrap_or(&[])));
        let sources = if let Some(files) = &r.inputs.sources {
            Sources::Flat(FlatSources {
                sources: source_files(files),
            })
        } else {
            Sources::EncodedChunks(
                r.inputs
                    .chunks
                    .clone()
                    .ok_or_else(|| Throwable::HarnessError("no sources/chunks".into()))?,
            )
        };
        let Some(RecordExpected::CompilerTestCase {
            output,
            diagnostics,
            ..
        }) = &r.expected
        else {
            return Err(Throwable::HarnessError(
                "no compiler_test_case expected".into(),
            ));
        };
        let expected = match output {
            ExpectedOutput::NoCheck => None,
            ExpectedOutput::Same => Some(Expected {
                expected: None,
                same: true,
            }),
            ExpectedOutput::Files(files) => Some(Expected {
                expected: Some(source_files(files)),
                same: false,
            }),
        };
        let mut diags = vec![];
        for d in diagnostics {
            let mut diag =
                Diagnostic::new(level(&d.level)?, diagnostic_type(&d.key, CheckLevel::ERROR));
            if let Some(mode) = d.message_mode {
                let msg = d
                    .message
                    .as_ref()
                    .ok_or_else(|| Throwable::HarnessError("missing diagnostic message".into()))?
                    .to_string_lossy();
                diag = match mode {
                    MessageMode::Contains => diag.with_message_containing(&msg)?,
                    MessageMode::ExactTrimmed => diag.with_message(&msg)?,
                    MessageMode::Unknown => {
                        return Err(Throwable::HarnessError(format!("message predicate {msg}")));
                    }
                };
            }
            if let Some(line) = d.line {
                diag = diag.with_location(line, d.charno.unwrap_or(-1), d.length.unwrap_or(-1));
            }
            diags.push(diag);
        }
        let posts = self
            .hooks
            .case
            .postconditions
            .as_ref()
            .map(|ps| {
                ps.iter()
                    .map(|e| eval(e, &mut self.hooks.ctx))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let api = self.hooks.record.record.api;
        self.hooks.created = None;
        let mut result =
            ReplayResult::catch_precondition_run(&self.hooks.record.raw.clone(), || match api {
                Api::TestInternal => self.harness.test_internal(
                    &mut self.hooks,
                    &externs,
                    &sources,
                    expected.as_ref(),
                    &diags,
                    &posts,
                    pipeline,
                ),
                Api::TestExternChanges => {
                    crate::throwable::check_state(
                        posts.is_empty(),
                        "testExternChanges takes no postconditions",
                    )?;
                    self.harness.test_extern_changes(
                        &mut self.hooks,
                        &externs,
                        &sources,
                        expected.as_ref().ok_or_else(|| {
                            Throwable::HarnessError("expected externs missing".into())
                        })?,
                        &diags,
                        pipeline,
                    )
                }
                _ => Err(Throwable::HarnessError(
                    "invalid CompilerTestCase api".into(),
                )),
            })?;
        let c = if api == Api::TestExternChanges {
            self.hooks.created.clone()
        } else {
            self.harness.get_last_compiler()
        };
        // Rust-only (as for integration records): the first unported
        // pass factory a compile of the processor omitted (a nested compile of
        // TranspileAndOptimizeClosureUnaware runs the SIMPLE pass lists).
        if let Some(c) = &c
            && let Ok(c) = c.try_borrow()
            && let Some(class) = c.get_omitted_unported_passes().first()
        {
            result.omitted_unported = Some(if class.starts_with("com.") {
                (*class).into()
            } else {
                format!("com.google.javascript.jscomp.{}", class.replace('.', "$"))
            });
        }
        let mut observed = IndexMap::new();
        if let Some(c) = &c {
            observed.insert("errors".into(), errors(&c.borrow().get_errors()));
            observed.insert("warnings".into(), errors(&c.borrow().get_warnings()));
        }
        if let Some(tfa) = &self.hooks.case.test_fields_after {
            let holder = eval(tfa, &mut self.hooks.ctx)?;
            let want = expected_test_fields_after(&self.hooks.record.raw, &self.hooks.case)?;
            let mut got = IndexMap::new();
            for name in want.as_object().unwrap().keys() {
                let value = crate::replay::replay_dsl::get_field(&holder, name)?;
                got.insert(
                    name.clone(),
                    canonical_class_names(
                        &unit_recorder::ref_value(&value, 2)?,
                        &self.hooks.ctx.class_map,
                    ),
                );
            }
            observed.insert("testFieldsAfter".into(), JsonValue::Object(got));
            result.test_fields_after_want = Some(want);
        }
        if self.hooks.record.record.post_call.is_some() {
            let mut roots = self.hooks.ctx.once.values().cloned().collect::<Vec<_>>();
            roots.extend(self.hooks.ctx.vars.values().cloned());
            let pc = matches!(&self.hooks.record.record.expected,Some(RecordExpected::CompilerTestCase{postconditions,..}) if *postconditions>0);
            result.post_call = Some(canonical_class_names(
                &unit_recorder::post_call_snapshot(
                    c.as_ref(),
                    self.hooks.first_processor.as_ref(),
                    &roots,
                    if pc {
                        // __recPostCompiler != null ? __recPostCompiler : lastCompiler (the call
                        // can throw before its postconditions run, as testTestTypes does)
                        self.hooks
                            .ctx
                            .postcondition_compiler
                            .as_ref()
                            .or(c.as_ref())
                    } else {
                        None
                    },
                )?,
                &self.hooks.ctx.class_map,
            ));
        }
        result.observed = JsonValue::Object(observed);
        Ok(result)
    }
}
// port: ReplayCompilerTest#expectedTestFieldsAfter
pub fn expected_test_fields_after(record: &JsonValue, case: &Case) -> Result<JsonValue, Throwable> {
    let mut want = record
        .get("testFieldsAfter")
        .and_then(JsonValue::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(skip) = &case.test_fields_after_skip {
        for (name, reason) in skip {
            if reason.trim().is_empty() {
                return Err(Throwable::HarnessError(format!(
                    "testFieldsAfterSkip.{name} needs a reason"
                )));
            }
            want.shift_remove(name);
        }
    }
    if let Some(also) = &case.test_fields_after_also {
        for name in also {
            if !want.contains_key(name) {
                let v = record
                    .get("testFields")
                    .and_then(|o| o.get(name))
                    .ok_or_else(|| {
                        Throwable::HarnessError(format!(
                            "testFieldsAfterAlso: no recorded field {name}"
                        ))
                    })?;
                want.insert(name.clone(), v.clone());
            }
        }
    }
    Ok(JsonValue::Object(want))
}
// port: ReplayCompilerTest#canonicalClassNames
pub fn canonical_class_names(value: &JsonValue, class_map: &IndexMap<String, String>) -> JsonValue {
    let inv = class_map
        .iter()
        .map(|(k, v)| (v.clone(), k.clone()))
        .collect::<IndexMap<_, _>>();
    rename(value, &inv)
}
// port: ReplayCompilerTest#rename
fn rename(value: &JsonValue, inverse: &IndexMap<String, String>) -> JsonValue {
    match value {
        JsonValue::String(s) => s
            .to_string_strict()
            .and_then(|s| inverse.get(&s))
            .map_or_else(|| value.clone(), |r| JsonValue::str(r)),
        JsonValue::Array(items) => {
            JsonValue::Array(items.iter().map(|v| rename(v, inverse)).collect())
        }
        JsonValue::Object(o) => JsonValue::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), rename(v, inverse)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
