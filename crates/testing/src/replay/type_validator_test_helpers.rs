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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/TypeValidatorTest.java.

//! Port of the replay helper `oracle/replay/helpers/com/google/javascript/jscomp/TypeValidatorTest_Helpers.java`
//! (DSL name `TypeValidatorTest_Helpers.Anon1`): the anonymous no-op CompilerPass TypeValidatorTest$1 returned by getProcessor (TypeValidatorTest.java lines 50-53).
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const PASS: &str = "com.google.javascript.jscomp.TypeValidatorTest_Helpers$Anon1";

/// `TypeValidatorTest_Helpers.Anon1 implements CompilerPass`: "Do nothing: we're in it for the type-checking."
struct Anon1;

impl NativeObject for Anon1 {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        PASS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: TypeValidatorTest_Helpers.Anon1#process
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

// port: TypeValidatorTest_Helpers.Anon1#<init>
pub fn new_pass(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(Anon1))))
}
