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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CompilerTestCaseUtils.java.

use crate::jscomp_api::CompilerHarnessAccess;
use crate::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    jscomp_api::{Compiler, CompilerOptions, SourceFile},
    throwable::Throwable,
};
use std::sync::Arc;
// port: CompilerTestCaseUtils#multistageSerializeAndDeserialize
pub fn multistage_serialize_and_deserialize(
    hooks: &mut impl CompilerTestCaseHooks,
    test_case: &mut CompilerTestCase,
    compiler: &mut Compiler,
    externs: &[Arc<SourceFile>],
    inputs: &[Arc<SourceFile>],
) -> Result<crate::replay::replay_dsl::CompilerHandle, Throwable> {
    let e = compiler.get_externs_root().unwrap();
    let r = compiler.get_js_root().unwrap();
    crate::harness_passes::remove_cast_nodes(compiler, e, r)?;
    let error_manager = compiler.get_error_manager_handle();
    let mut baos: Vec<u8> = Vec::new();
    compiler.remove_recent_change_handler();
    compiler.disable_threads();
    crate::harness_passes::save_state(compiler, &mut baos)?;

    let new_compiler = hooks.create_compiler(test_case)?;
    {
        let options = hooks.get_options(test_case)?;
        let mut c = new_compiler.borrow_mut();
        c.disable_threads();
        c.init(externs, inputs, options);
        crate::harness_passes::restore_state(&mut c, &baos)?;
        c.set_error_manager_handle(error_manager);
        // Java hands the test case's own CodeChangeHandler to the restored compiler.
        let change_handler = compiler.get_change_tracker().get_recent_change();
        c.get_change_tracker().set_recent_change(change_handler);
        c.add_recent_change_handler();
    }
    Ok(new_compiler)
}
// port: CompilerTestCaseUtils#setDebugLogDirectoryOn
pub fn set_debug_log_directory_on(_options: &mut CompilerOptions) {}
