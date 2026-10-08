/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ScopedAliasesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch).

//! Port of the unit-corpus helper `ScopedAliasesTest_Helpers.java` (oracle/replay/helpers): the
//! `TypeVerifyingPass` copied from ScopedAliasesTest (lines 1534-1574), which the descriptor's
//! VERIFY_TYPES postcondition runs over the compiler's externs and JS roots.
use crate::{
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::node_traversal::{Callback, NodeTraversal};
use closure_rhino::{node::NodeId, testing::node_subject::assert_node};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const TYPE_VERIFYING_PASS: &str =
    "com.google.javascript.jscomp.ScopedAliasesTest_Helpers$TypeVerifyingPass";

/// ScopedAliasesTest_Helpers.TypeVerifyingPass. Java's assertions throw out of the traversal;
/// here `visit` keeps the first failure and stops checking, and `process` throws it.
pub struct TypeVerifyingPass {
    compiler: CompilerHandle,
    actual_types: Option<Vec<NodeId>>,
    failure: Option<Throwable>,
}

// port: ScopedAliasesTest_Helpers.TypeVerifyingPass#TypeVerifyingPass
pub fn type_verifying_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "ScopedAliasesTest_Helpers$TypeVerifyingPass arguments".into(),
        ));
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(TypeVerifyingPass {
        compiler: compiler.clone(),
        actual_types: None,
        failure: None,
    }))))
}

// port: ScopedAliasesTest_Helpers.TypeVerifyingPass#process
pub fn process(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Native(pass),
        DslValue::Node(_externs),
        DslValue::Node(root),
    ] = args.as_slice()
    else {
        return Err(Throwable::HarnessError(
            "ScopedAliasesTest_Helpers$TypeVerifyingPass#process arguments".into(),
        ));
    };
    let mut pass = pass.borrow_mut();
    let pass = pass
        .as_any_mut()
        .downcast_mut::<TypeVerifyingPass>()
        .ok_or_else(|| Throwable::HarnessError("not a TypeVerifyingPass".into()))?;
    let compiler = pass.compiler.clone();
    NodeTraversal::traverse(&mut compiler.borrow_mut(), *root, pass);
    match pass.failure.take() {
        Some(failure) => Err(failure),
        None => Ok(DslValue::Null),
    }
}

impl Callback for TypeVerifyingPass {
    // port: ScopedAliasesTest_Helpers.TypeVerifyingPass#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ScopedAliasesTest_Helpers.TypeVerifyingPass#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if self.failure.is_some() {
            return;
        }
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            let type_nodes = info.get_type_nodes();
            if !type_nodes.is_empty() {
                if let Some(actual_types) = &self.actual_types {
                    let mut expected_types: Vec<NodeId> = Vec::new();
                    expected_types.extend(info.get_type_nodes());
                    if actual_types.len() != expected_types.len() {
                        // assertWithMessage(..).that(actualTypes.size()).isEqualTo(..) (Truth)
                        self.failure = Some(Throwable::Assertion {
                            message: format!(
                                "Wrong number of JsDoc types\nexpected: {}\nbut was : {}",
                                expected_types.len(),
                                actual_types.len()
                            ),
                        });
                        return;
                    }
                    for i in 0..expected_types.len() {
                        if let Err(message) =
                            assert_node(actual_types[i]).check_equal_to(t, expected_types[i], false)
                        {
                            self.failure = Some(Throwable::Assertion { message });
                            return;
                        }
                    }
                } else {
                    let mut actual_types = Vec::new();
                    actual_types.extend(info.get_type_nodes());
                    self.actual_types = Some(actual_types);
                }
            }
        }
    }
}

impl NativeObject for TypeVerifyingPass {
    fn class_name(&self) -> &str {
        TYPE_VERIFYING_PASS
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        // compiler and actualTypes: neither reaches a recorded result producer.
        Ok(IndexMap::new())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
