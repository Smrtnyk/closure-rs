/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/PrebuildAst.java.

//! Port of `PrebuildAst.java`: a helper class to prebuild ASTs from a list of CompilerInputs.
use crate::{abstract_compiler::AbstractCompiler, compiler_input::CompilerInput};

// A submitted task (ListenableFuture<?>), run on the compiler thread.
type Future<'a> = Box<dyn FnOnce(&mut AbstractCompiler) + 'a>;

/// The Java object keeps its AbstractCompiler; Rust callers pass it to `prebuild`.
///
/// Java submits one `input.getAstRoot(compiler)` task per input to a pool of
/// `numParallelThreads` daemon threads and waits for all of them. The compiler's node arena has a
/// single owner (the compiler thread), so the submitted tasks run on it, in submission order.
pub struct PrebuildAst {
    num_parallel_threads: i32,
}
impl PrebuildAst {
    // port: PrebuildAst#PrebuildAst
    pub fn new(num_paralle_threads: i32) -> Self {
        Self {
            num_parallel_threads: num_paralle_threads,
        }
    }

    // port: PrebuildAst#prebuild
    pub fn prebuild(&self, compiler: &mut AbstractCompiler, all_inputs: &[CompilerInput]) {
        // new ThreadPoolExecutor(numParallelThreads, numParallelThreads, ...) rejects a
        // non-positive pool size.
        assert!(
            self.num_parallel_threads > 0,
            "java.lang.IllegalArgumentException"
        );
        let mut future_list: Vec<Future<'_>> = Vec::with_capacity(all_inputs.len());
        // TODO(moz): Support canceling all parsing on the first halting error
        for input in all_inputs {
            future_list.push(Box::new(move |compiler: &mut AbstractCompiler| {
                input.get_ast_root(compiler);
            }));
        }

        // Futures.allAsList(futureList).get()
        for future in future_list {
            future(compiler);
        }
    }
}
