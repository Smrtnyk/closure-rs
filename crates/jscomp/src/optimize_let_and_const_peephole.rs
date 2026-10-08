/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/OptimizeLetAndConstPeephole.java.

//! Port of `OptimizeLetAndConstPeephole.java`.
//!
//! Late-stage compiler pass that lowers `const`/`let` to `var` where safe, and `const` to `let`,
//! improving keyword homogeneity for better compression.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};

pub struct OptimizeLetAndConstPeephole {
    fields: AbstractPeepholeOptimizationFields,
    assume_output_is_wrapped: bool,
}

impl OptimizeLetAndConstPeephole {
    // port: OptimizeLetAndConstPeephole#OptimizeLetAndConstPeephole
    pub fn new(assume_output_is_wrapped: bool) -> Self {
        Self {
            fields: AbstractPeepholeOptimizationFields::new(),
            assume_output_is_wrapped,
        }
    }

    // port: OptimizeLetAndConstPeephole#processLet
    fn process_let(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        if !self.assume_output_is_wrapped && n.get_parent(compiler).unwrap().is_script(compiler) {
            return n;
        }

        if Self::is_directly_in_hoist_scope(compiler, n) {
            n.set_token(compiler, Token::VAR);
            self.report_change_to_enclosing_scope(compiler, n);
        }
        n
    }

    // port: OptimizeLetAndConstPeephole#processConst
    fn process_const(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        if !self.assume_output_is_wrapped && n.get_parent(compiler).unwrap().is_script(compiler) {
            return n;
        }

        if Self::is_directly_in_hoist_scope(compiler, n) {
            n.set_token(compiler, Token::VAR);
        } else {
            n.set_token(compiler, Token::LET);
            self.add_feature_to_enclosing_script(Feature::LET_DECLARATIONS);
        }
        self.report_change_to_enclosing_scope(compiler, n);
        n
    }

    // port: OptimizeLetAndConstPeephole#isDirectlyInHoistScope
    fn is_directly_in_hoist_scope(ast: &Ast, n: NodeId) -> bool {
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            if !p.is_label(ast) {
                break;
            }
            parent = p.get_parent(ast);
        }
        let Some(parent) = parent else {
            return false;
        };

        if parent.is_script(ast) || parent.is_module_body(ast) {
            return true;
        }

        if parent.is_block(ast) {
            let grandparent = parent.get_parent(ast);
            if let Some(grandparent) = grandparent
                && (grandparent.is_function(ast) || grandparent.is_class_members(ast))
            {
                return true;
            }
        }

        false
    }
}

impl AbstractPeepholeOptimization for OptimizeLetAndConstPeephole {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.OptimizeLetAndConstPeephole"
    }

    // port: OptimizeLetAndConstPeephole#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        Some(match subtree.get_token(compiler) {
            Token::LET => self.process_let(compiler, subtree),
            Token::CONST => self.process_const(compiler, subtree),
            _ => subtree,
        })
    }
}
