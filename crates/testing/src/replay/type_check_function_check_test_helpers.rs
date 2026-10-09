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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/TypeCheckFunctionCheckTest.java.

//! Port of the replay helper `oracle/replay/helpers/com/google/javascript/jscomp/TypeCheckFunctionCheckTest_Helpers.java`
//! (DSL name `TypeCheckFunctionCheckTest_Helpers.GetProcessorPass`): the anonymous no-op CompilerPass TypeCheckFunctionCheckTest$1 returned by getProcessor (TypeCheckFunctionCheckTest.java lines 37-42).
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const PASS: &str =
    "com.google.javascript.jscomp.TypeCheckFunctionCheckTest_Helpers$GetProcessorPass";

/// `TypeCheckFunctionCheckTest_Helpers.GetProcessorPass implements CompilerPass` (it captures nothing).
struct GetProcessorPass;

impl NativeObject for GetProcessorPass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        PASS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: TypeCheckFunctionCheckTest_Helpers.GetProcessorPass#process
    fn process(
        &mut self,
        _compiler: &mut Compiler,
        _externs: NodeId,
        _root: NodeId,
    ) -> Result<(), Throwable> {
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: TypeCheckFunctionCheckTest_Helpers.GetProcessorPass#<init>
pub fn new_pass(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessorPass))))
}
