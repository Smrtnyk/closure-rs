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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayBridge.java.

//! ReplayBridge's public decoder/options entry points for other harness packages.
use crate::{
    descriptor::Case,
    jscomp_api::{CompilerOptions, DiagnosticGroup, JSChunk, JSError, SourceFile},
    json::JsonValue,
    record::Chunk,
    replay::{
        options_fields,
        replay_dsl::{self, Ctx, DslValue},
        replay_options, replay_values,
    },
    throwable::Throwable,
    value::{DiagnosticGroupRef, FieldDump, Value},
};
use std::sync::Arc;

// port: ReplayBridge#options(JsonObject)
pub fn options(diff: &FieldDump, ctx: &mut Ctx) -> Result<CompilerOptions, Throwable> {
    replay_options::build(diff, None, ctx)
}

// port: ReplayBridge#options(JsonObject,JsonObject,JsonObject)
pub fn options_with_case(
    diff: &FieldDump,
    case: Option<&Case>,
    ctx: &mut Ctx,
) -> Result<CompilerOptions, Throwable> {
    replay_options::build(diff, case.and_then(|c| c.options.as_ref()), ctx)
}

// port: ReplayBridge#setField
pub fn set_field(
    instance: &DslValue,
    _class: &str,
    name: &str,
    value: &Value,
    ctx: &Ctx,
) -> Result<(), Throwable> {
    if let DslValue::Options(options) = instance {
        return options_fields::set_field(&mut options.borrow_mut(), name, value, &ctx.class_map);
    }
    let value = replay_values::decode(value, "java.lang.Object", &ctx.class_map)?;
    replay_dsl::set_field(instance, name, value, ctx)
}

// port: ReplayBridge#diagnosticGroup
pub fn diagnostic_group(group: &DiagnosticGroupRef) -> Arc<DiagnosticGroup> {
    replay_values::diagnostic_group(group)
}

// port: ReplayBridge#chunks
pub fn chunks(records: &[Chunk]) -> Result<Vec<JSChunk>, Throwable> {
    replay_values::chunks(records)
}

// port: ReplayBridge#sourceFiles
pub fn source_files(files: &[crate::value::SourceFile]) -> Vec<Arc<SourceFile>> {
    replay_values::source_files(files)
}

// port: ReplayBridge#errors
pub fn errors(errors: &[JSError]) -> JsonValue {
    replay_values::errors(errors)
}
