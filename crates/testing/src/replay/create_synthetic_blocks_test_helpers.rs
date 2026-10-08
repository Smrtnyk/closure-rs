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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CreateSyntheticBlocksTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Port of the replay helper `oracle/replay/helpers/.../CreateSyntheticBlocksTest_Helpers.java`
//! (DSL name `CreateSyntheticBlocksTest_Helpers.GetProcessor`), itself copied from
//! CreateSyntheticBlocksTest.java: the constants START_MARKER and END_MARKER and `getProcessor`
//! (an anonymous CompilerPass). The generated wrapper's `getName()` returns the recorded
//! `harness.effective.getName`, its constructor argument.
use crate::{
    replay::{
        api_passes_helpers::native_pass,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    Compiler, abstract_peephole_optimization::AbstractPeepholeOptimization,
    compiler_pass::CompilerPass, create_synthetic_blocks::CreateSyntheticBlocks,
    denormalize::Denormalize, minimize_exit_points::MinimizeExitPoints, normalize::Normalize,
    peephole_fold_constants::PeepholeFoldConstants,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.CreateSyntheticBlocksTest_Helpers$GetProcessor";
/// The anonymous `CompilerPass` returned by `getProcessor`.
const GET_PROCESSOR_PASS: &str =
    "com.google.javascript.jscomp.CreateSyntheticBlocksTest_Helpers$GetProcessor$1";

// port: CreateSyntheticBlocksTest_Helpers#START_MARKER
const START_MARKER: &str = "startMarker";
// port: CreateSyntheticBlocksTest_Helpers#END_MARKER
const END_MARKER: &str = "endMarker";

/// `private static final class GetProcessor`.
struct GetProcessor {
    name: String,
}

impl NativeObject for GetProcessor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        GET_PROCESSOR
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert("name".into(), DslValue::String(self.name.as_str().into()));
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl GetProcessor {
    // port: CreateSyntheticBlocksTest_Helpers.GetProcessor#getName
    fn get_name(&self) -> String {
        self.name.clone()
    }

    // port: CreateSyntheticBlocksTest_Helpers.GetProcessor#getProcessor
    fn get_processor(&self) -> Box<dyn CompilerPass> {
        let name = self.get_name();
        Box::new(
            move |compiler: &mut Compiler, externs: NodeId, js: NodeId| {
                CreateSyntheticBlocks::new(START_MARKER, Some(END_MARKER.into()))
                    .process(compiler, externs, js);
                Normalize::create_normalize_for_optimizations(compiler)
                    .process(compiler, externs, js);
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![
                    Box::new(MinimizeExitPoints::new()),
                    Box::new(PeepholeRemoveDeadCode::new()),
                    Box::new(PeepholeMinimizeConditions::new(/* late= */ true)),
                    Box::new(PeepholeFoldConstants::new(true, false /* useTypes */)),
                ];
                PeepholeOptimizationsPass::new(name.clone(), optimizations)
                    .process(compiler, externs, js);
                Denormalize::new(compiler, FeatureSet::BARE_MINIMUM).process(compiler, externs, js);
            },
        )
    }
}

// port: CreateSyntheticBlocksTest_Helpers.GetProcessor#GetProcessor
pub fn get_processor_new(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let name = match args.as_slice() {
        [DslValue::String(name)] => name.to_string(),
        [DslValue::Typed { value, .. }] => match &**value {
            DslValue::String(name) => name.to_string(),
            _ => return Err(bad()),
        },
        _ => return Err(bad()),
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessor {
        name,
    }))))
}

// port: CreateSyntheticBlocksTest_Helpers.GetProcessor#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<GetProcessor>()
        .ok_or_else(bad)?;
    Ok(native_pass(GET_PROCESSOR_PASS, receiver.get_processor()))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
