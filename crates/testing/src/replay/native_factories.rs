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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! Native PassFactory callbacks share replay state while borrowing the executing compiler.
use crate::{
    jscomp_api::{Compiler, CompilerPass, PassFactory},
    replay::replay_dsl::{Ctx, DslValue, Lambda, invoke_lambda_with_compiler, process_in_compiler},
    throwable::Throwable,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

thread_local! {
    static FACTORIES: RefCell<IndexMap<u64,Rc<Lambda>>> = RefCell::new(IndexMap::<_, _>::default());
    static CONTEXTS: RefCell<Vec<Rc<RefCell<Ctx>>>> = const { RefCell::new(Vec::new()) };
}
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
pub struct FactoryToken {
    id: u64,
    thread: std::thread::ThreadId,
}
impl Drop for FactoryToken {
    // port: ReplayDsl#lambda (native callback lifetime)
    fn drop(&mut self) {
        if self.thread == std::thread::current().id() {
            // The thread-local map may already be under destruction (thread exit drops the
            // lambdas, whose captured passes own further tokens): then there is nothing to
            // unregister. The removed lambda is dropped after the map's borrow ends, because
            // dropping it can drop further tokens.
            let _ = FACTORIES.try_with(|factories| {
                let removed = factories
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut factories| factories.shift_remove(&self.id));
                drop(removed);
            });
        }
    }
}
// port: CompilerTestCase#makePassFactory
pub fn create(name: String, lambda: Rc<Lambda>) -> (PassFactory, Arc<FactoryToken>) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    FACTORIES.with(|factories| {
        factories.borrow_mut().insert(id, lambda);
    });
    let token = Arc::new(FactoryToken {
        id,
        thread: std::thread::current().id(),
    });
    let callback = token.clone();
    let factory = PassFactory::builder()
        .set_name(name)
        .set_internal_factory(Arc::new(move |compiler| {
            let lambda = FACTORIES
                .with(|factories| factories.borrow().get(&callback.id).cloned())
                .unwrap_or_else(|| {
                    std::panic::panic_any(Throwable::HarnessError(
                        "PassFactory callback executed outside its replay thread".into(),
                    ))
                });
            let context = current();
            let mut context = context.borrow_mut();
            let value = context
                .compiler
                .as_ref()
                .map_or(DslValue::Null, |compiler| {
                    DslValue::Compiler(compiler.clone())
                });
            let value =
                invoke_lambda_with_compiler(&lambda, vec![value], &mut context, Some(compiler))
                    .unwrap_or_else(|error| std::panic::panic_any(error));
            Box::new(CallbackPass(value))
        }))
        .build();
    (factory, token)
}
struct CallbackPass(DslValue);
impl CompilerPass for CallbackPass {
    // port: ReplayDsl.SequencePass#process
    fn process(&mut self, compiler: &mut Compiler, externs: NodeId, root: NodeId) {
        let context = current();
        process_in_compiler(&self.0, compiler, externs, root, &mut context.borrow_mut())
            .unwrap_or_else(|error| std::panic::panic_any(error));
    }
}
// port: ReplayDsl#lambda (call-time replay context)
fn current() -> Rc<RefCell<Ctx>> {
    CONTEXTS
        .with(|contexts| contexts.borrow().last().cloned())
        .unwrap_or_else(|| {
            std::panic::panic_any(Throwable::HarnessError(
                "PassFactory has no active replay context".into(),
            ))
        })
}
// port: ReplayDsl.SequencePass#process (native factory context scope)
pub fn with_context<T>(context: &mut Ctx, action: impl FnOnce() -> T) -> T {
    let replacement = Ctx::new(
        context.descriptor.clone(),
        crate::json::JsonValue::Null,
        context.class_map.clone(),
        context.registry.clone(),
    );
    let active = Rc::new(RefCell::new(std::mem::replace(context, replacement)));
    CONTEXTS.with(|contexts| contexts.borrow_mut().push(active.clone()));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    CONTEXTS.with(|contexts| {
        contexts.borrow_mut().pop().expect("native factory context");
    });
    *context = Rc::try_unwrap(active)
        .unwrap_or_else(|_| panic!("native factory context escaped its scope"))
        .into_inner();
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
