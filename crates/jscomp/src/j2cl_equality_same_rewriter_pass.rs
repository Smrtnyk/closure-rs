/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/J2clEqualitySameRewriterPass.java.

//! Port of `J2clEqualitySameRewriterPass.java` as a named, gated no-op (J2CL is out of scope, docs/PORTING.md §2).
//!
//! An optimization pass to re-write J2CL Equality.$same. The class keeps Java's gate: `beginTraversal` reads
//! `J2clSourceFileChecker#shouldRunJ2clPasses`, and `optimizeSubtree` returns its input
//! unchanged when the gate is off, which is always the case without J2CL (`*.java.js`) inputs.
//! The J2CL rewriting itself is not ported: with the gate on the node is also returned
//! unchanged.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    j2cl_source_file_checker::J2clSourceFileChecker,
};
use closure_rhino::node::NodeId;

pub struct J2clEqualitySameRewriterPass {
    fields: AbstractPeepholeOptimizationFields,
    use_types: bool,
    should_run_j2cl_passes: bool,
}

impl J2clEqualitySameRewriterPass {
    // port: J2clEqualitySameRewriterPass#J2clEqualitySameRewriterPass(boolean)
    pub fn new(use_types: bool) -> Self {
        Self {
            fields: AbstractPeepholeOptimizationFields::new(),
            use_types,
            should_run_j2cl_passes: false,
        }
    }

    /// Java's `useTypes` (read only by the J2CL rewriting, which is out of scope).
    pub fn uses_types(&self) -> bool {
        self.use_types
    }
}

impl AbstractPeepholeOptimization for J2clEqualitySameRewriterPass {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.J2clEqualitySameRewriterPass"
    }

    // port: J2clEqualitySameRewriterPass#beginTraversal
    fn begin_traversal(&mut self, compiler: &mut AbstractCompiler) {
        let fields = self.fields_mut();
        fields.begin_traversal(compiler);
        self.should_run_j2cl_passes = J2clSourceFileChecker::should_run_j2cl_passes(compiler);
    }

    // port: J2clEqualitySameRewriterPass#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        _compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if !self.should_run_j2cl_passes {
            return Some(node);
        }
        // J2CL rewriting is out of scope (docs/PORTING.md §2): a no-op.
        Some(node)
    }
}
