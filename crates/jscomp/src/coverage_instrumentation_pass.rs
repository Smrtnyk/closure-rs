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

// STAND-IN: instrumentation/CoverageInstrumentationPass is out of scope (docs/PORTING.md §2); not ported
use crate::{
    abstract_compiler::AbstractCompiler, compiler_options::InstrumentOption,
    compiler_pass::CompilerPass,
};
use closure_rhino::node::NodeId;
pub struct CoverageInstrumentationPass;
impl CoverageInstrumentationPass {
    pub fn new(_option: InstrumentOption, _array_name: &str) -> Self {
        Self
    }
}
impl CompilerPass for CoverageInstrumentationPass {
    fn process(&mut self, _compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {}
}
