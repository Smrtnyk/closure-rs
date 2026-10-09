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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayOptions.java.

//! ReplayOptions builds recorded effective options and descriptor overrides.
use crate::{
    descriptor::CaseOptions,
    jscomp_api::CompilerOptions,
    replay::{
        options_fields,
        replay_dsl::{Ctx, eval_all},
    },
    throwable::Throwable,
    value::FieldDump,
};
use closure_rhino::fast_hash::IndexSet;
use std::{cell::RefCell, rc::Rc};
// port: ReplayOptions#build
pub fn build(
    diff: &FieldDump,
    override_: Option<&CaseOptions>,
    ctx: &mut Ctx,
) -> Result<CompilerOptions, Throwable> {
    let skip = override_
        .and_then(|o| o.skip.as_ref())
        .map(|keys| keys.iter().map(String::as_str).collect::<IndexSet<_>>())
        .unwrap_or_default();
    let options = build_skipping(diff, &skip, ctx)?;
    let handle = Rc::new(RefCell::new(options));
    if let Some(then) = override_.and_then(|o| o.then.as_deref()) {
        let saved = ctx.options.replace(handle.clone());
        let result = eval_all(then, ctx);
        ctx.options = saved;
        result?;
    }
    let options = handle.borrow().clone();
    Ok(options)
}
// port: ReplayOptions#buildSkipping
fn build_skipping(
    diff: &FieldDump,
    skip: &IndexSet<&str>,
    ctx: &Ctx,
) -> Result<CompilerOptions, Throwable> {
    let mut o = CompilerOptions::new();
    if let Some(crate::value::Value::String(c)) = diff.get("@class") {
        let class = ctx.map_class(&c.to_string_lossy());
        if class != "com.google.javascript.jscomp.CompilerOptions" {
            return Err(Throwable::Unported(class));
        }
    }
    for (name, value) in diff {
        if name == "@class" || skip.contains(name.as_str()) {
            continue;
        }
        options_fields::set_field(&mut o, name, value, &ctx.class_map)?;
    }
    Ok(o)
}
