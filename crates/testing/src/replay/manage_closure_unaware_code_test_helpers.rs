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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/ManageClosureUnawareCodeTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../ManageClosureUnawareCodeTest_Helpers.java`
//! (DSL name `ManageClosureUnawareCodeTest_Helpers.GetProcessor`), itself copied from
//! ManageClosureUnawareCodeTest.java: the holder fields `runUnwrapPass` and
//! `gatheredShadowNodeRoot`, and `getProcessor`.
use crate::{
    replay::{
        native_registry::NativePhaseOptimizer,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass, manage_closure_unaware_code::ManageClosureUnawareCode,
    node_util::NodeUtil, pass_factory::PassFactory, phase_optimizer::PhaseOptimizer,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{ir::IR, node::Ast, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.ManageClosureUnawareCodeTest_Helpers";
const GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.ManageClosureUnawareCodeTest_Helpers$GetProcessor";

/// The helper holder. `gatheredShadowNodeRoot` is a detached `IR.root()`; Rust nodes live in the
/// compiler's arena, so the holder creates it in the arena of the compiler under test.
pub struct ManageClosureUnawareCodeTestHelpers {
    run_unwrap_pass: bool,
    gathered_shadow_node_root: NodeId,
}

impl NativeObject for ManageClosureUnawareCodeTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert("runUnwrapPass".into(), DslValue::Bool(self.run_unwrap_pass));
        fields.insert(
            "gatheredShadowNodeRoot".into(),
            DslValue::Node(self.gathered_shadow_node_root),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value) {
            ("runUnwrapPass", DslValue::Bool(value)) => {
                self.run_unwrap_pass = value;
                Ok(())
            }
            ("gatheredShadowNodeRoot", DslValue::Node(value)) => {
                self.gathered_shadow_node_root = value;
                Ok(())
            }
            (name, _) => Err(Throwable::Unported(format!("{HOLDER}#{name}"))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `private final class GetProcessor`: a non-static inner class of the holder.
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
    // port: ManageClosureUnawareCodeTest_Helpers.GetProcessor#getProcessor
    fn get_processor(&self, compiler: &crate::jscomp_api::Compiler) -> Result<DslValue, Throwable> {
        let (run_unwrap_pass, gathered_shadow_node_root) = {
            let mut outer = self.outer.borrow_mut();
            let holder = outer
                .as_any_mut()
                .downcast_mut::<ManageClosureUnawareCodeTestHelpers>()
                .ok_or_else(bad)?;
            (holder.run_unwrap_pass, holder.gathered_shadow_node_root)
        };
        let mut phaseopt = PhaseOptimizer::new(compiler, None);
        let mut passes: Vec<PassFactory> = vec![];
        passes.push(
            PassFactory::builder()
                .set_name("gatherShadowNodes")
                .set_internal_factory(Arc::new(move |_c: &mut crate::jscomp_api::Compiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(
                        move |_compiler: &mut crate::jscomp_api::Compiler,
                              _externs: NodeId,
                              root: NodeId| {
                            NodeUtil::visit_pre_order(
                                _compiler,
                                root,
                                &mut |ast: &mut Ast, node: NodeId| {
                                    let shadow = node.get_closure_unaware_shadow(ast);
                                    if let Some(shadow) = shadow {
                                        if !run_unwrap_pass {
                                            node.set_closure_unaware_shadow(ast, None);
                                        }
                                        let clone = shadow.clone_tree(ast);
                                        gathered_shadow_node_root.add_child_to_back(ast, clone);
                                    }
                                },
                            );
                        },
                    );
                    pass
                }))
                .build(),
        );

        if run_unwrap_pass {
            passes.push(
                PassFactory::builder()
                    .set_name("unwrapClosureUnawareCode")
                    .set_internal_factory(Arc::new(|_c: &mut crate::jscomp_api::Compiler| {
                        let pass: Box<dyn CompilerPass> =
                            Box::new(ManageClosureUnawareCode::unwrap());
                        pass
                    }))
                    .build(),
            );
        }
        phaseopt.consume(passes);
        Ok(DslValue::Native(Rc::new(RefCell::new(
            NativePhaseOptimizer(phaseopt),
        ))))
    }
}

// port: ManageClosureUnawareCodeTest_Helpers#ManageClosureUnawareCodeTest_Helpers
pub fn holder(ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    // private Node gatheredShadowNodeRoot = IR.root();
    let compiler = ctx.compiler.as_ref().ok_or_else(bad)?;
    let gathered_shadow_node_root = IR::root(&mut compiler.borrow_mut().ast, &[]);
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ManageClosureUnawareCodeTestHelpers {
            run_unwrap_pass: true,
            gathered_shadow_node_root,
        },
    ))))
}

// port: ManageClosureUnawareCodeTest_Helpers.GetProcessor#GetProcessor
pub fn get_processor_new(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer)] = args.as_slice() else {
        return Err(bad());
    };
    if outer.borrow().class_name() != HOLDER {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessor {
        outer: outer.clone(),
    }))))
}

// port: ManageClosureUnawareCodeTest_Helpers.GetProcessor#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<GetProcessor>()
        .ok_or_else(bad)?;
    receiver.get_processor(&compiler.borrow())
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
