/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CombinedCompilerPass.java.

//! Replay adapter for CombinedCompilerPass#<init>(AbstractCompiler, NodeTraversal.Callback[]):
//! the recorded native callbacks are shared with the DSL and lent to the real CombinedCompilerPass
//! on each `process`.
use crate::{
    replay::{
        registry::Entry,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    combined_compiler_pass::CombinedCompilerPass,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
};
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.CombinedCompilerPass#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.NodeTraversal$Callback[])" => {
            combined_compiler_pass
        }
        _ => return None,
    })
}

/// A native NodeTraversal.Callback shared with the replay DSL.
struct SharedCallback {
    callback: Rc<RefCell<dyn NativeObject>>,
    is_scoped: bool,
}
impl Callback for SharedCallback {
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let mut callback = self.callback.borrow_mut();
        callback
            .as_traversal_callback()
            .unwrap()
            .should_traverse(t, n, parent)
    }
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let mut callback = self.callback.borrow_mut();
        callback
            .as_traversal_callback()
            .unwrap()
            .visit(t, n, parent);
    }
    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        if self.is_scoped { Some(self) } else { None }
    }
}
impl ScopedCallback for SharedCallback {
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let mut callback = self.callback.borrow_mut();
        let callback = callback.as_traversal_callback().unwrap();
        callback.as_scoped_callback().unwrap().enter_scope(t);
    }
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let mut callback = self.callback.borrow_mut();
        let callback = callback.as_traversal_callback().unwrap();
        callback.as_scoped_callback().unwrap().exit_scope(t);
    }
}

fn combined_compiler_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), DslValue::Array { items, .. }] = args.as_slice() else {
        return Err(bad());
    };
    let mut callbacks: Vec<Box<dyn Callback>> = Vec::new();
    for item in items {
        let DslValue::Native(native) = item else {
            return Err(bad());
        };
        let is_scoped = {
            let mut callback = native.borrow_mut();
            let class = callback.class_name().to_string();
            callback
                .as_traversal_callback()
                .ok_or(Throwable::Unported(class))?
                .as_scoped_callback()
                .is_some()
        };
        callbacks.push(Box::new(SharedCallback {
            callback: native.clone(),
            is_scoped,
        }));
    }
    let _ = c;
    let pass = CombinedCallbacks { callbacks };
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

/// The `new CombinedCompilerPass(compiler, callbacks)` object: owns the callbacks the real
/// CombinedCompilerPass borrows for each `process`.
struct CombinedCallbacks {
    callbacks: Vec<Box<dyn Callback>>,
}
impl CompilerPass for CombinedCallbacks {
    // port: CombinedCompilerPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let callbacks: Vec<&mut dyn Callback> = self
            .callbacks
            .iter_mut()
            .map(|callback| callback.as_mut() as &mut dyn Callback)
            .collect();
        CombinedCompilerPass::new(compiler, callbacks).process(compiler, Some(externs), root);
    }
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
