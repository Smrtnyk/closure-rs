/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CheckGoogJsImportTest.java,
//   test/com/google/javascript/jscomp/RewriteGoogJsImportsTest.java.

//! Port of the unit-corpus helpers `RewriteGoogJsImportsTest_Helpers.java` and
//! `CheckGoogJsImportTest_Helpers.java` (oracle/replay/helpers): the `GetProcessorPass` passes
//! copied from the getProcessor lambdas of RewriteGoogJsImportsTest and CheckGoogJsImportTest. The
//! passes are constructed inside process(), after the earlier passes ran: ModuleMapCreator reads
//! compiler.getModuleMetadataMap() and RewriteGoogJsImports reads compiler.getModuleMap() at
//! construction time.
use crate::{
    replay::replay_dsl::{Ctx, DslValue},
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    gather_module_metadata::GatherModuleMetadata,
    modules::module_map_creator::ModuleMapCreator,
    rewrite_goog_js_imports::{Mode, RewriteGoogJsImports},
};
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

/// `RewriteGoogJsImportsTest_Helpers.GetProcessorPass` (mode LINT_AND_REWRITE) and
/// `CheckGoogJsImportTest_Helpers.GetProcessorPass` (mode LINT_ONLY): the two bodies differ only
/// in the RewriteGoogJsImports mode.
struct GetProcessorPass {
    mode: Mode,
}

// port: RewriteGoogJsImportsTest_Helpers.GetProcessorPass#GetProcessorPass
pub fn rewrite_goog_js_imports_test_get_processor_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    get_processor_pass(args, Mode::LINT_AND_REWRITE)
}

// port: CheckGoogJsImportTest_Helpers.GetProcessorPass#GetProcessorPass
pub fn check_goog_js_import_test_get_processor_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    get_processor_pass(args, Mode::LINT_ONLY)
}

fn get_processor_pass(args: Vec<DslValue>, mode: Mode) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "GetProcessorPass#<init>(Compiler) arguments".into(),
        ));
    };
    let pass: Box<dyn CompilerPass> = Box::new(GetProcessorPass { mode });
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

impl CompilerPass for GetProcessorPass {
    // port: RewriteGoogJsImportsTest_Helpers.GetProcessorPass#process
    // port: CheckGoogJsImportTest_Helpers.GetProcessorPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut gmm = GatherModuleMetadata::new(
            /* processCommonJsModules= */ false,
            ResolutionMode::BROWSER,
        );
        gmm.process(compiler, externs, root);
        let module_metadata_map = compiler
            .get_module_metadata_map()
            .cloned()
            .expect("java.lang.NullPointerException");
        let mut mmc = ModuleMapCreator::new(module_metadata_map);
        mmc.process(compiler, externs, root);
        RewriteGoogJsImports::new(self.mode, compiler.get_module_map().cloned())
            .process(compiler, externs, root);
    }
}
