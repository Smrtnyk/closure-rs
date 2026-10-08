/*
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CrossChunkCodeMotion.java,
//   src/com/google/javascript/jscomp/CrossChunkMethodMotion.java,
//   src/com/google/javascript/jscomp/IdGenerator.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the cross-chunk passes: CrossChunkCodeMotion, CrossChunkMethodMotion and
//! IdGenerator constructors as the CrossChunk*Test descriptors call them.
use crate::{
    replay::{
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    cross_chunk_code_motion::CrossChunkCodeMotion,
    cross_chunk_method_motion::{CrossChunkIdGenerator, CrossChunkMethodMotion},
    id_generator::IdGenerator,
};
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by the cross-chunk passes)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.CrossChunkCodeMotion#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.JSChunkGraph,boolean)" => {
            cross_chunk_code_motion
        }
        "com.google.javascript.jscomp.CrossChunkMethodMotion#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.IdGenerator,boolean,boolean)" => {
            cross_chunk_method_motion
        }
        "com.google.javascript.jscomp.IdGenerator#<init>()" => id_generator,
        "com.google.javascript.jscomp.CrossChunkReferenceCollectorTest_Helpers#<init>()" => {
            crate::replay::cross_chunk_reference_collector_helpers::holder
        }
        "com.google.javascript.jscomp.CrossChunkReferenceCollectorTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::cross_chunk_reference_collector_helpers::get_processor
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (constructors while a factory borrows the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        "com.google.javascript.jscomp.CrossChunkMethodMotion#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.IdGenerator,boolean,boolean)" => {
            cross_chunk_method_motion_borrowed
        }
        "com.google.javascript.jscomp.CrossChunkReferenceCollectorTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::cross_chunk_reference_collector_helpers::get_processor_borrowed
        }
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

fn pass(p: impl CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(p))))
}

// port: CrossChunkCodeMotion#CrossChunkCodeMotion
fn cross_chunk_code_motion(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    match args.as_slice() {
        [
            DslValue::Compiler(_),
            _graph,
            DslValue::Bool(parent_chunk_can_see),
        ] => Ok(pass(CrossChunkCodeMotion::new(*parent_chunk_can_see))),
        _ => Err(bad()),
    }
}

/// `new IdGenerator()`; the pass constructor takes it over.
struct NativeIdGenerator {
    id_generator: Option<IdGenerator>,
}
impl NativeObject for NativeIdGenerator {
    // port: ReplayDsl#invoke (IdGenerator receiver)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.IdGenerator"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: IdGenerator#IdGenerator
fn id_generator(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeIdGenerator {
        id_generator: Some(IdGenerator::default()),
    }))))
}

fn method_motion_args(args: &[DslValue]) -> Result<(IdGenerator, bool, bool), Throwable> {
    match args {
        [
            DslValue::Compiler(_),
            DslValue::Native(id_generator),
            DslValue::Bool(can_modify_externs),
            DslValue::Bool(no_stub_functions),
        ] => {
            let mut id_generator = id_generator.borrow_mut();
            let id_generator = id_generator
                .as_any_mut()
                .downcast_mut::<NativeIdGenerator>()
                .and_then(|g| g.id_generator.take())
                .ok_or_else(bad)?;
            Ok((id_generator, *can_modify_externs, *no_stub_functions))
        }
        _ => Err(bad()),
    }
}

// port: CrossChunkMethodMotion#CrossChunkMethodMotion
fn cross_chunk_method_motion(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let (id_generator, can_modify_externs, no_stub_functions) = method_motion_args(&args)?;
    let Some(DslValue::Compiler(c)) = args.first() else {
        return Err(bad());
    };
    let mut compiler = c.borrow_mut();
    Ok(pass(CrossChunkMethodMotion::new(
        &mut compiler,
        CrossChunkIdGenerator::Owned(id_generator),
        can_modify_externs,
        no_stub_functions,
    )))
}

// port: CrossChunkMethodMotion#CrossChunkMethodMotion (compiler borrowed by a factory)
fn cross_chunk_method_motion_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let (id_generator, can_modify_externs, no_stub_functions) = method_motion_args(&args)?;
    Ok(pass(CrossChunkMethodMotion::new(
        compiler,
        CrossChunkIdGenerator::Owned(id_generator),
        can_modify_externs,
        no_stub_functions,
    )))
}
