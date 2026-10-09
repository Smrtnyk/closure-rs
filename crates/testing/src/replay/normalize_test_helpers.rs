/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NormalizeTest.java.

//! Port of the unit-corpus helper `NormalizeTest_Helpers.java` (oracle/replay/helpers): the
//! anonymous CompilerTestCase of NormalizeTest#testRenamingConstantProperties (instanceClass
//! NormalizeTest$1), whose `getProcessor` the NormalizeTest descriptor calls.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_options::{ChunkOutputType, PropertyCollapseLevel},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    es6_normalize_classes::Es6NormalizeClasses,
    inline_and_collapse_properties::InlineAndCollapseProperties,
    pass_factory::PassFactory,
    phase_optimizer::PhaseOptimizer,
};
use closure_rhino::fast_hash::IndexMap;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const TESTER: &str =
    "com.google.javascript.jscomp.NormalizeTest_Helpers$RenamingConstantPropertiesTester";

// port: NormalizeTest_Helpers.RenamingConstantPropertiesTester#RenamingConstantPropertiesTester
pub fn tester(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: TESTER.into(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    }))))
}

// port: NormalizeTest_Helpers.RenamingConstantPropertiesTester#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(_), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "NormalizeTest_Helpers$RenamingConstantPropertiesTester#getProcessor arguments".into(),
        ));
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    // makePassFactory("es6NormalizeClasses", Es6NormalizeClasses::new)
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("es6NormalizeClasses")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(Es6NormalizeClasses::new(compiler))
            }))
            .build(),
    );
    // makePassFactory(
    //     "inlineAndCollapseProperties",
    //     (comp) ->
    //         InlineAndCollapseProperties.builder(compiler)
    //             .setPropertyCollapseLevel(PropertyCollapseLevel.ALL)
    //             .setChunkOutputType(ChunkOutputType.GLOBAL_NAMESPACE)
    //             .setHaveModulesBeenRewritten(false)
    //             .setModuleResolutionMode(ResolutionMode.BROWSER)
    //             .build())
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("inlineAndCollapseProperties")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(
                    InlineAndCollapseProperties::builder(compiler)
                        .set_property_collapse_level(PropertyCollapseLevel::ALL)
                        .set_chunk_output_type(ChunkOutputType::GLOBAL_NAMESPACE)
                        .set_have_modules_been_rewritten(false)
                        .set_module_resolution_mode(ResolutionMode::BROWSER)
                        .build(),
                )
            }))
            .build(),
    );
    let pass: Box<dyn CompilerPass> = Box::new(optimizer);
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
