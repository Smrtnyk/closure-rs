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
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/InferConstsTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../InferConstsTest_Helpers.java` (DSL name
//! `InferConstsTest_Helpers`), itself copied from InferConstsTest.java: the holder fields
//! `constFinder` and `names`, `getProcessor` with its anonymous CompilerPass, and FindConstants.
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    infer_consts::InferConsts,
    node_traversal::{AbstractPostOrderCallback, Callback, NodeTraversal},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.InferConstsTest_Helpers";
const PROCESSOR: &str = "com.google.javascript.jscomp.InferConstsTest_Helpers$1";
const FIND_CONSTANTS: &str = "com.google.javascript.jscomp.InferConstsTest_Helpers$FindConstants";

/// `final class InferConstsTest_Helpers`.
pub struct InferConstsTestHelpers {
    const_finder: Option<Rc<RefCell<FindConstants>>>,
    names: DslValue,
}

impl NativeObject for InferConstsTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "constFinder".into(),
            self.const_finder.as_ref().map_or(DslValue::Null, |f| {
                let f: Rc<RefCell<dyn NativeObject>> = f.clone();
                DslValue::Native(f)
            }),
        );
        fields.insert("names".into(), self.names.clone());
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match name {
            "names" => {
                self.names = value;
                Ok(())
            }
            _ => Err(Throwable::Unported(format!("{HOLDER}#{name}"))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: InferConstsTest_Helpers#InferConstsTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        InferConstsTestHelpers {
            const_finder: None,
            names: DslValue::Null,
        },
    ))))
}

// port: InferConstsTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<InferConstsTestHelpers>()
        .ok_or_else(bad)?;
    let const_finder = Rc::new(RefCell::new(FindConstants::new(this.names.clone())?));
    this.const_finder = Some(const_finder.clone());
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessorPass {
        const_finder,
    }))))
}

/// The anonymous `new CompilerPass() { ... }` returned by getProcessor
/// (`InferConstsTest_Helpers$1`). It captures `compiler` and the holder; the Rust pass receives
/// the compiler at process time and keeps the holder's `constFinder` it was created with.
struct GetProcessorPass {
    const_finder: Rc<RefCell<FindConstants>>,
}

impl NativeObject for GetProcessorPass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        PROCESSOR
    }
    // port: ReplayValues#findField (native object adapter)
    // The recorder lists no instance field of the anonymous class (its captures are synthetic).
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: InferConstsTest_Helpers$1#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        InferConsts::new(compiler).process(compiler, externs, root);
        let mut const_finder = self.const_finder.borrow_mut();
        NodeTraversal::traverse(compiler, root, &mut *const_finder);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `private static class FindConstants extends NodeTraversal.AbstractPostOrderCallback`.
pub struct FindConstants {
    names_value: DslValue,
    names: Vec<JsString>,
    declared_nodes: Vec<JsString>,
    inferred_nodes: Vec<JsString>,
}

impl FindConstants {
    // port: InferConstsTest_Helpers.FindConstants#FindConstants
    fn new(names_value: DslValue) -> Result<Self, Throwable> {
        let names = match names_value.untyped() {
            DslValue::List(items) => items
                .iter()
                .map(|v| match v {
                    DslValue::String(s) => Ok(s.clone()),
                    _ => Err(bad()),
                })
                .collect::<Result<Vec<_>, _>>()?,
            // `for (String name : names)` on a null list.
            DslValue::Null => {
                return Err(Throwable::Exception {
                    class: "java.lang.NullPointerException".into(),
                    message: None,
                });
            }
            _ => return Err(bad()),
        };
        Ok(Self {
            names_value,
            names,
            declared_nodes: vec![],
            inferred_nodes: vec![],
        })
    }
}

// port: HashSet#add
fn hash_set_add(set: &mut Vec<JsString>, name: &JsString) {
    if !set.contains(name) {
        set.push(name.clone());
    }
}

impl Callback for FindConstants {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        AbstractPostOrderCallback::new(()).should_traverse(t, n, parent)
    }
    // port: InferConstsTest_Helpers.FindConstants#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        for name in &self.names {
            if (n.is_name(t) || n.is_import_star(t)) && n.matches_qualified_name(t, name.clone()) {
                if n.is_declared_constant_var(t) {
                    hash_set_add(&mut self.declared_nodes, name);
                }
                if n.is_inferred_constant_var(t) {
                    hash_set_add(&mut self.inferred_nodes, name);
                }
            }
        }
    }
}

impl NativeObject for FindConstants {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        FIND_CONSTANTS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let set = |items: &[JsString]| DslValue::Typed {
            class: "java.util.HashSet".into(),
            value: Box::new(DslValue::Set(
                items.iter().cloned().map(DslValue::String).collect(),
            )),
        };
        let mut fields = IndexMap::<_, _>::default();
        fields.insert("names".into(), self.names_value.clone());
        fields.insert("declaredNodes".into(), set(&self.declared_nodes));
        fields.insert("inferredNodes".into(), set(&self.inferred_nodes));
        Ok(fields)
    }
    // port: ReplayDsl#invoke (native traversal callback receiver)
    fn as_traversal_callback(&mut self) -> Option<&mut dyn Callback> {
        Some(self)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
