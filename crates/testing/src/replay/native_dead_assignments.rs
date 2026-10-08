/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/DeadPropertyAssignmentElimination.java,
//   test/com/google/javascript/jscomp/CoalesceVariableNamesTest.java,
//   test/com/google/javascript/jscomp/DeadAssignmentsEliminationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for DeadAssignmentsElimination, DeadPropertyAssignmentElimination and
//! CoalesceVariableNames, including the oracle replay helpers
//! `DeadAssignmentsEliminationTest_Helpers` and `CoalesceVariableNamesTest_Helpers`.
use crate::{
    jscomp_api::Compiler,
    replay::{
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    coalesce_variable_names::CoalesceVariableNames, compiler_pass::CompilerPass,
    dead_assignments_elimination::DeadAssignmentsElimination,
    dead_property_assignment_elimination::DeadPropertyAssignmentElimination,
    node_traversal::NodeTraversal, normalize::Normalize,
};
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.DeadAssignmentsEliminationTest_Helpers$GetProcessorLambda#<init>(com.google.javascript.jscomp.Compiler)" => {
            dead_assignments_elimination_get_processor_lambda
        }
        "com.google.javascript.jscomp.DeadPropertyAssignmentElimination#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            dead_property_assignment_elimination
        }
        "com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers#<init>()" => {
            coalesce_variable_names_test_helpers
        }
        "com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers)" => {
            coalesce_variable_names_get_processor
        }
        "com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            coalesce_variable_names_get_processor_get_processor
        }
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError("unexpected replay arguments".into())
}

/// `DeadAssignmentsEliminationTest_Helpers.GetProcessorLambda`: the getProcessor lambda;
/// `compiler` is the captured getProcessor argument.
struct GetProcessorLambda {
    compiler: CompilerHandle,
}
impl NativeObject for GetProcessorLambda {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.DeadAssignmentsEliminationTest_Helpers$GetProcessorLambda"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: ReplayValues#findField (the helper's declared field `compiler`)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "compiler".to_string(),
            DslValue::Compiler(self.compiler.clone()),
        )]))
    }
    // port: DeadAssignmentsEliminationTest_Helpers.GetProcessorLambda#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        _externs: NodeId,
        js: NodeId,
    ) -> Result<(), Throwable> {
        NodeTraversal::traverse(compiler, js, &mut DeadAssignmentsElimination::new());
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: DeadAssignmentsEliminationTest_Helpers.GetProcessorLambda#GetProcessorLambda
fn dead_assignments_elimination_get_processor_lambda(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        GetProcessorLambda {
            compiler: compiler.clone(),
        },
    ))))
}

// port: DeadPropertyAssignmentElimination#DeadPropertyAssignmentElimination
fn dead_property_assignment_elimination(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let pass: Box<dyn CompilerPass> = Box::new(DeadPropertyAssignmentElimination::new());
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

/// `CoalesceVariableNamesTest_Helpers`: the outer holder of the test's `usePseudoName` field.
struct CoalesceVariableNamesTestHelpers {
    use_pseudo_name: bool,
}
impl NativeObject for CoalesceVariableNamesTestHelpers {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers"
    }
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "usePseudoName".to_string(),
            DslValue::Bool(self.use_pseudo_name),
        )]))
    }
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value) {
            ("usePseudoName", DslValue::Bool(value)) => {
                self.use_pseudo_name = value;
                Ok(())
            }
            _ => Err(Throwable::Unported(format!("{}#{name}", self.class_name()))),
        }
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: CoalesceVariableNamesTest_Helpers#CoalesceVariableNamesTest_Helpers
fn coalesce_variable_names_test_helpers(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(
        CoalesceVariableNamesTestHelpers {
            use_pseudo_name: false,
        },
    ))))
}

/// `CoalesceVariableNamesTest_Helpers.GetProcessor`, a non-static inner class of the holder.
struct GetProcessor {
    outer: Rc<RefCell<dyn NativeObject>>,
}
impl NativeObject for GetProcessor {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers$GetProcessor"
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: CoalesceVariableNamesTest_Helpers.GetProcessor#GetProcessor
fn coalesce_variable_names_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessor {
        outer: outer.clone(),
    }))))
}

/// The anonymous CompilerPass returned by `GetProcessor#getProcessor`.
struct CoalesceVariableNamesProcessor {
    outer: Rc<RefCell<dyn NativeObject>>,
}
impl NativeObject for CoalesceVariableNamesProcessor {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CoalesceVariableNamesTest_Helpers$GetProcessor$1"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: ReplayValues#findField (the anonymous class declares no fields; its captures are
    // synthetic)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: CoalesceVariableNamesTest_Helpers.GetProcessor#getProcessor (anonymous CompilerPass#process)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        // enableNormalize would require output of CoalesceVariableNames to be normalized,
        // so we just manually normalize the input instead.
        let mut normalize = Normalize::create_normalize_for_optimizations(compiler);
        normalize.process(compiler, externs, root);
        let use_pseudo_name = {
            let mut outer = self.outer.borrow_mut();
            match outer
                .as_any_mut()
                .downcast_mut::<CoalesceVariableNamesTestHelpers>()
            {
                Some(helpers) => helpers.use_pseudo_name,
                None => return Err(bad()),
            }
        };
        CoalesceVariableNames::new(compiler, use_pseudo_name).process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: CoalesceVariableNamesTest_Helpers.GetProcessor#getProcessor
fn coalesce_variable_names_get_processor_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let outer = {
        let mut receiver = receiver.borrow_mut();
        match receiver.as_any_mut().downcast_mut::<GetProcessor>() {
            Some(get_processor) => get_processor.outer.clone(),
            None => return Err(bad()),
        }
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        CoalesceVariableNamesProcessor { outer },
    ))))
}
