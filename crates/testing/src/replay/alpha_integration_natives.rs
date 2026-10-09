/*
 * Copyright 2025 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/TranspileAndOptimizeClosureUnaware.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the integration classes: the @closureUnaware nested compiler pass as the
//! TranspileAndOptimizeClosureUnawareTest descriptor constructs it.
use crate::{
    replay::{
        registry::Entry,
        replay_dsl::{Ctx, DslValue},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    nested_compiler_runner::Mode,
    transpile_and_optimize_closure_unaware::TranspileAndOptimizeClosureUnaware,
};
use std::{cell::RefCell, rc::Rc};

const TRANSPILE_AND_OPTIMIZE_CLOSURE_UNAWARE_INIT: &str = "com.google.javascript.jscomp.TranspileAndOptimizeClosureUnaware#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.NestedCompilerRunner$Mode)";
const MODE_CLASS: &str = "com.google.javascript.jscomp.NestedCompilerRunner$Mode";

// port: ReplayDsl#invoke (resolved signatures backed by the alpha-integration classes)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        TRANSPILE_AND_OPTIMIZE_CLOSURE_UNAWARE_INIT => transpile_and_optimize_closure_unaware,
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: TranspileAndOptimizeClosureUnaware#TranspileAndOptimizeClosureUnaware
fn transpile_and_optimize_closure_unaware(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    match args.as_slice() {
        [DslValue::Compiler(_), DslValue::Enum { class, name }] if class == MODE_CLASS => {
            let mode = match name.as_str() {
                "TRANSPILE_AND_OPTIMIZE" => Mode::TRANSPILE_AND_OPTIMIZE,
                "TRANSPILE_ONLY" => Mode::TRANSPILE_ONLY,
                _ => return Err(bad()),
            };
            Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
                TranspileAndOptimizeClosureUnaware::new(mode),
            )))))
        }
        _ => Err(bad()),
    }
}
