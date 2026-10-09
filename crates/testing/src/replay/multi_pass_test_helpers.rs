/*
 * Copyright 2013 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/MultiPassTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Port of the replay helper `oracle/replay/helpers/.../MultiPassTest_Helpers.java` (DSL name
//! `MultiPassTest_Helpers.GetProcessor`), itself copied from MultiPassTest.java: the field
//! `passes`, `getProcessor` and the passes-building helpers addNormalization ..
//! addInjectTranspilationRuntimeLibraries.
use crate::{
    replay::{
        native_registry::NativePhaseOptimizer,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization,
    compiler_options::{CompilerOptions, Reach},
    compiler_pass::CompilerPass,
    es6_rewrite_arrow_function::Es6RewriteArrowFunction,
    es6_rewrite_destructuring::{self, ObjectDestructuringRewriteMode},
    inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries,
    inline_functions::InlineFunctions,
    inline_object_literals::InlineObjectLiterals,
    inline_variables::{InlineVariables, Mode},
    normalize::Normalize,
    pass_factory::PassFactory,
    peephole_collect_property_assignments::PeepholeCollectPropertyAssignments,
    peephole_fold_constants::PeepholeFoldConstants,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
    peephole_replace_known_methods::PeepholeReplaceKnownMethods,
    peephole_substitute_alternate_syntax::PeepholeSubstituteAlternateSyntax,
    phase_optimizer::PhaseOptimizer,
    remove_unused_code::RemoveUnusedCode,
    validity_check::ValidityCheck,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.MultiPassTest_Helpers";
const GET_PROCESSOR: &str = "com.google.javascript.jscomp.MultiPassTest_Helpers$GetProcessor";

/// The helper holder (one test instance).
pub struct MultiPassTestHelpers {
    passes: Vec<PassFactory>,
    // Generated: the value CompilerTestCase#getName() returns for the test instance.
    test_name: String,
}

impl NativeObject for MultiPassTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl MultiPassTestHelpers {
    // port: MultiPassTest_Helpers#getName
    fn get_name(&self) -> &str {
        &self.test_name
    }

    // port: MultiPassTest_Helpers#addNormalization
    fn add_normalization(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("normalization")
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> =
                        Box::new(Normalize::create_normalize_for_optimizations(compiler));
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addCollapseObjectLiterals
    fn add_collapse_object_literals(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("collapseObjectLiterals")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(InlineObjectLiterals::new(
                        compiler.get_unique_name_id_supplier(),
                    ));
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addInlineFunctions
    fn add_inline_functions(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("inlineFunctions")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(InlineFunctions::new(
                        compiler,
                        compiler.get_unique_name_id_supplier(),
                        Reach::ALL,
                        true,
                        true,
                        CompilerOptions::UNLIMITED_FUN_SIZE_AFTER_INLINING,
                    ));
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addInlineVariables
    fn add_inline_variables(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("inlineVariables")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|_compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(InlineVariables::new(Mode::ALL));
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addPeephole
    fn add_peephole(&mut self) {
        let pass_name = self.get_name().to_string();
        self.passes.push(
            PassFactory::builder()
                .set_name("peepholeOptimizations")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(move |_compiler: &mut AbstractCompiler| {
                    let late = false;
                    let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![
                        Box::new(PeepholeMinimizeConditions::new(late)),
                        Box::new(PeepholeSubstituteAlternateSyntax::new(late)),
                        Box::new(PeepholeReplaceKnownMethods::new(
                            late, /* useTypes= */ false,
                        )),
                        Box::new(PeepholeRemoveDeadCode::new()),
                        Box::new(PeepholeFoldConstants::new(late, false /* useTypes */)),
                        Box::new(PeepholeCollectPropertyAssignments::new()),
                    ];
                    let pass: Box<dyn CompilerPass> = Box::new(PeepholeOptimizationsPass::new(
                        pass_name.clone(),
                        optimizations,
                    ));
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addRemoveUnusedClassProperties
    fn add_remove_unused_class_properties(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("removeUnusedClassProperties")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(
                        RemoveUnusedCode::builder(compiler)
                            .remove_unused_this_properties(true)
                            .remove_unused_object_define_properties_definitions(true)
                            .build(),
                    );
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addRemoveUnusedVars
    fn add_remove_unused_vars(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("removeUnusedVars")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(
                        RemoveUnusedCode::builder(compiler)
                            .remove_local_vars(true)
                            .build(),
                    );
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addDestructuringPass
    fn add_destructuring_pass(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("destructuringPass")
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(
                        es6_rewrite_destructuring::Builder::new(compiler)
                            .set_destructuring_rewrite_mode(
                                ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS,
                            )
                            .build(),
                    );
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addArrowFunctionPass
    fn add_arrow_function_pass(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("arrowFunctionPass")
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> =
                        Box::new(Es6RewriteArrowFunction::new(compiler));
                    pass
                }))
                .build(),
        );
    }

    // port: MultiPassTest_Helpers#addInjectTranspilationRuntimeLibraries
    fn add_inject_transpilation_runtime_libraries(&mut self) {
        self.passes.push(
            PassFactory::builder()
                .set_name("injectTranspilationRuntimeLibraries")
                .set_internal_factory(Arc::new(|compiler: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> =
                        Box::new(InjectTranspilationRuntimeLibraries::new(compiler));
                    pass
                }))
                .build(),
        );
    }
}

/// `private class GetProcessor`: a non-static inner class of the holder.
struct GetProcessor {
    outer: Rc<RefCell<dyn NativeObject>>,
}

impl NativeObject for GetProcessor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        GET_PROCESSOR
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl GetProcessor {
    // port: MultiPassTest_Helpers.GetProcessor#getProcessor
    fn get_processor(&self, compiler: &mut AbstractCompiler) -> Result<DslValue, Throwable> {
        let passes = {
            let mut outer = self.outer.borrow_mut();
            let holder = outer
                .as_any_mut()
                .downcast_mut::<MultiPassTestHelpers>()
                .ok_or_else(bad)?;
            holder.passes.clone()
        };
        let mut phaseopt = PhaseOptimizer::new(compiler, None);
        phaseopt.consume(passes);
        phaseopt.set_validity_check(
            compiler,
            PassFactory::builder()
                .set_name("validityCheck")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|c: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(ValidityCheck::new(c));
                    pass
                }))
                .build(),
        );
        Ok(DslValue::Native(Rc::new(RefCell::new(
            NativePhaseOptimizer(phaseopt),
        ))))
    }
}

// port: MultiPassTest_Helpers#MultiPassTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        MultiPassTestHelpers {
            passes: Vec::new(),
            test_name: String::new(),
        },
    ))))
}

// port: MultiPassTest_Helpers.GetProcessor#GetProcessor
pub fn get_processor_new(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Native(outer),
        DslValue::String(test_name),
        DslValue::Array { items: adders, .. },
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    {
        let mut outer = outer.borrow_mut();
        let holder = outer
            .as_any_mut()
            .downcast_mut::<MultiPassTestHelpers>()
            .ok_or_else(bad)?;
        holder.test_name = test_name.to_string();
        holder.passes = Vec::new();
        for adder in adders {
            let DslValue::String(adder) = adder else {
                return Err(bad());
            };
            match adder.to_string().as_str() {
                "addNormalization" => holder.add_normalization(),
                "addCollapseObjectLiterals" => holder.add_collapse_object_literals(),
                "addInlineFunctions" => holder.add_inline_functions(),
                "addInlineVariables" => holder.add_inline_variables(),
                "addPeephole" => holder.add_peephole(),
                "addRemoveUnusedClassProperties" => holder.add_remove_unused_class_properties(),
                "addRemoveUnusedVars" => holder.add_remove_unused_vars(),
                "addDestructuringPass" => holder.add_destructuring_pass(),
                "addArrowFunctionPass" => holder.add_arrow_function_pass(),
                "addInjectTranspilationRuntimeLibraries" => {
                    holder.add_inject_transpilation_runtime_libraries()
                }
                other => {
                    return Err(Throwable::HarnessError(format!(
                        "unknown MultiPassTest adder {other}"
                    )));
                }
            }
        }
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessor {
        outer: outer.clone(),
    }))))
}

// port: MultiPassTest_Helpers.GetProcessor#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<GetProcessor>()
        .ok_or_else(bad)?;
    receiver.get_processor(&mut compiler.borrow_mut())
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
