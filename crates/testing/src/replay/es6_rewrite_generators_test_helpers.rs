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
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/Es6RewriteGeneratorsTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../Es6RewriteGeneratorsTest_Helpers.java`
//! (DSL name `Es6RewriteGeneratorsTest_Helpers.GetProcessor`), itself copied from
//! Es6RewriteGeneratorsTest.java: the holder field `extraRuntimeLibFields` and `getProcessor`.
use crate::{
    replay::{
        native_registry::NativePhaseOptimizer,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass, es6_rewrite_generators::Es6RewriteGenerators,
    inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries,
    pass_factory::PassFactory, phase_optimizer::PhaseOptimizer,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.Es6RewriteGeneratorsTest_Helpers";
const GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.Es6RewriteGeneratorsTest_Helpers$GetProcessor";

/// The helper holder: `private ImmutableList<String> extraRuntimeLibFields = ImmutableList.of();`
pub struct Es6RewriteGeneratorsTestHelpers {
    extra_runtime_lib_fields: Vec<String>,
}

impl NativeObject for Es6RewriteGeneratorsTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "extraRuntimeLibFields".into(),
            DslValue::List(
                self.extra_runtime_lib_fields
                    .iter()
                    .map(|f| DslValue::String(f.as_str().into()))
                    .collect(),
            ),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        // A recorded collection decodes with its runtime class (RegularImmutableList).
        let value = match value {
            DslValue::Typed { value, .. } => *value,
            value => value,
        };
        match (name, value) {
            ("extraRuntimeLibFields", DslValue::List(items)) => {
                let mut fields = Vec::with_capacity(items.len());
                for item in items {
                    let DslValue::String(s) = item else {
                        return Err(bad());
                    };
                    fields.push(s.to_string());
                }
                self.extra_runtime_lib_fields = fields;
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

/// `private final class GetProcessor extends CompilerTestCase`: a non-static inner class of the
/// holder.
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
    // port: Es6RewriteGeneratorsTest_Helpers.GetProcessor#getProcessor
    fn get_processor(&self, compiler: &crate::jscomp_api::Compiler) -> Result<DslValue, Throwable> {
        let extra_runtime_lib_fields = {
            let mut outer = self.outer.borrow_mut();
            let holder = outer
                .as_any_mut()
                .downcast_mut::<Es6RewriteGeneratorsTestHelpers>()
                .ok_or_else(bad)?;
            holder.extra_runtime_lib_fields.clone()
        };
        let mut optimizer = PhaseOptimizer::new(compiler, None);
        optimizer.add_one_time_pass(
            PassFactory::builder()
                .set_name("extraRuntimeLibraries")
                .set_internal_factory(Arc::new(move |_c: &mut crate::jscomp_api::Compiler| {
                    let extra_runtime_lib_fields = extra_runtime_lib_fields.clone();
                    let pass: Box<dyn CompilerPass> = Box::new(
                        move |c: &mut crate::jscomp_api::Compiler,
                              _externs: NodeId,
                              _js: NodeId| {
                            for field in &extra_runtime_lib_fields {
                                let manager = c.get_runtime_js_lib_manager();
                                manager.lock().unwrap().inject_lib_for_field(c, field);
                            }
                        },
                    );
                    pass
                }))
                .build(),
        );
        optimizer.add_one_time_pass(
            PassFactory::builder()
                .set_name("injectTranspilationRuntimeLibraries")
                .set_internal_factory(Arc::new(|c: &mut crate::jscomp_api::Compiler| {
                    let pass: Box<dyn CompilerPass> =
                        Box::new(InjectTranspilationRuntimeLibraries::new(c));
                    pass
                }))
                .build(),
        );
        optimizer.add_one_time_pass(
            PassFactory::builder()
                .set_name("es6RewriteGenerators")
                .set_internal_factory(Arc::new(|c: &mut crate::jscomp_api::Compiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(Es6RewriteGenerators::new(c));
                    pass
                }))
                .build(),
        );
        Ok(DslValue::Native(Rc::new(RefCell::new(
            NativePhaseOptimizer(optimizer),
        ))))
    }
}

// port: Es6RewriteGeneratorsTest_Helpers#Es6RewriteGeneratorsTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        Es6RewriteGeneratorsTestHelpers {
            extra_runtime_lib_fields: vec![],
        },
    ))))
}

// port: Es6RewriteGeneratorsTest_Helpers.GetProcessor#GetProcessor
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

// port: Es6RewriteGeneratorsTest_Helpers.GetProcessor#getProcessor
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
