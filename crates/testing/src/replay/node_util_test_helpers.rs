/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   test/com/google/javascript/jscomp/NodeUtilTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Port of the replay helper `oracle/replay/helpers/com/google/javascript/jscomp/NodeUtilTest_Helpers.java`
//! (DSL name `NodeUtilTest_Helpers`), itself copied from the nested test class
//! `NodeUtilTest.CreateSynthesizedExternsSymbolTests`: `getProcessor` returns an anonymous
//! CompilerPass that calls `NodeUtil.createSynthesizedExternsSymbol(compiler, "TEST_NAME")`.
use crate::{
    replay::{
        api_passes_helpers::native_pass,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{AbstractCompiler, compiler_pass::CompilerPass, node_util::NodeUtil};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.NodeUtilTest_Helpers";
/// The anonymous `CompilerPass` returned by `getProcessor`.
const GET_PROCESSOR_PASS: &str = "com.google.javascript.jscomp.NodeUtilTest_Helpers$1";

/// `final class NodeUtilTest_Helpers` (no fields).
struct NodeUtilTestHelpers;

impl NativeObject for NodeUtilTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `new CompilerPass() { ... }` in `getProcessor`. Java captures the `compiler` argument of
/// `getProcessor`; the harness passes that same compiler to `process`.
struct CreateSynthesizedExternsSymbolPass;

impl CompilerPass for CreateSynthesizedExternsSymbolPass {
    // port: NodeUtilTest_Helpers#getProcessor (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _srcs: NodeId) {
        let expected_extern = "TEST_NAME";
        NodeUtil::create_synthesized_externs_symbol(compiler, expected_extern);
    }
}

// port: NodeUtilTest_Helpers#NodeUtilTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(NodeUtilTestHelpers))))
}

// port: NodeUtilTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    receiver
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<NodeUtilTestHelpers>()
        .ok_or_else(bad)?;
    Ok(native_pass(
        GET_PROCESSOR_PASS,
        Box::new(CreateSynthesizedExternsSymbolPass),
    ))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
