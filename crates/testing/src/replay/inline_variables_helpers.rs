/*
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/FlowSensitiveInlineVariables.java,
//   src/com/google/javascript/jscomp/InlineVariables.java.

//! Replay adapters for the constructors the InlineVariablesTest, InlineVariablesConstantsTest and
//! FlowSensitiveInlineVariablesTest descriptors call.
use crate::{
    replay::replay_dsl::{Ctx, DslValue},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass, flow_sensitive_inline_variables::FlowSensitiveInlineVariables,
    inline_variables::InlineVariables, inline_variables::Mode,
};
use std::{cell::RefCell, rc::Rc};

const MODE: &str = "com.google.javascript.jscomp.InlineVariables$Mode";

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("{what} arguments"))
}

// port: InlineVariables#InlineVariables
pub fn inline_variables(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), DslValue::Enum { class, name }] = args.as_slice() else {
        return Err(bad("InlineVariables"));
    };
    if class != MODE {
        return Err(bad("InlineVariables"));
    }
    let mode = match name.as_str() {
        "CONSTANTS_ONLY" => Mode::CONSTANTS_ONLY,
        "LOCALS_ONLY" => Mode::LOCALS_ONLY,
        "ALL" => Mode::ALL,
        _ => return Err(bad("InlineVariables")),
    };
    let pass: Box<dyn CompilerPass> = Box::new(InlineVariables::new(mode));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: FlowSensitiveInlineVariables#FlowSensitiveInlineVariables
pub fn flow_sensitive_inline_variables(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("FlowSensitiveInlineVariables"));
    };
    let pass: Box<dyn CompilerPass> = Box::new(FlowSensitiveInlineVariables::new());
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
