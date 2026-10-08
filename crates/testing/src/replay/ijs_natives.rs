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
/*
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/ExtraRequireRemover.java,
//   src/com/google/javascript/jscomp/ijs/ConvertToTypedInterface.java.

//! Replay adapters for the .i.js passes (ConvertToTypedInterface, ExtraRequireRemover).
use crate::{
    replay::{
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass, extra_require_remover::ExtraRequireRemover,
    ijs::convert_to_typed_interface::ConvertToTypedInterface,
};
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.ijs.ConvertToTypedInterface#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            convert_to_typed_interface
        }
        "com.google.javascript.jscomp.ExtraRequireRemover#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            extra_require_remover
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(Throwable::HarnessError(
            "native method arguments do not match the resolved Java signature".into(),
        ))
    }
}

// port: ReplayDsl#invoke (CompilerPass return value)
fn pass(pass: impl CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(pass))))
}

// port: ConvertToTypedInterface#ConvertToTypedInterface
fn convert_to_typed_interface(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(ConvertToTypedInterface::new()))
}

// port: ExtraRequireRemover#ExtraRequireRemover
fn extra_require_remover(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(ExtraRequireRemover::new()))
}
