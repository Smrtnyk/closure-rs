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
//   src/com/google/javascript/jscomp/lint/CheckUselessBlocks.java.

//! Check for useless blocks. A block is considered useful if it is part of a control structure
//! like if / while / try, or if it contains a block-scoped variable declaration (let, const,
//! class, or function declaration).

#![allow(clippy::collapsible_match)] // Preserve Java switch cases with nested ifs.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};

// port: CheckUselessBlocks#USELESS_BLOCK
pub static USELESS_BLOCK: DiagnosticType =
    DiagnosticType::disabled("JSC_USELESS_BLOCK", "Useless block.");

pub struct CheckUselessBlocks {
    /// Java `Deque<Node> loneBlocks` used as a stack: `push`/`pop`/`peek` act on the back.
    lone_blocks: Vec<NodeId>,
}

impl CheckUselessBlocks {
    // port: CheckUselessBlocks#CheckUselessBlocks
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            lone_blocks: Vec::new(),
        }
    }

    /// A lone block is a non-synthetic, not-added BLOCK that is a direct child of another
    /// non-synthetic, not-added BLOCK or a SCRIPT node.
    // port: CheckUselessBlocks#isLoneBlock
    fn is_lone_block(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast);
        if let Some(parent) = parent
            && (parent.is_script(ast)
                || (parent.is_block(ast)
                    && !parent.is_synthetic_block(ast)
                    && !parent.is_added_block(ast)))
        {
            return !n.is_synthetic_block(ast) && !n.is_added_block(ast);
        }
        false
    }

    /// Remove the enclosing block of a block-scoped declaration from the loneBlocks stack.
    // port: CheckUselessBlocks#allowLoneBlock
    fn allow_lone_block(&mut self, parent: Option<NodeId>) {
        if self.lone_blocks.is_empty() {
            return;
        }
        if self.lone_blocks.last().copied() == parent {
            self.lone_blocks.pop();
        }
    }
}

impl CompilerPass for CheckUselessBlocks {
    // port: CheckUselessBlocks#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckUselessBlocks {
    // port: CheckUselessBlocks#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::BLOCK => {
                if Self::is_lone_block(t, n) {
                    self.lone_blocks.push(n);
                }
            }
            Token::LET | Token::CONST => self.allow_lone_block(parent),
            Token::CLASS => {
                if NodeUtil::is_class_declaration(t, n) {
                    self.allow_lone_block(parent);
                }
            }
            Token::FUNCTION => {
                if NodeUtil::is_function_declaration(t, n) {
                    self.allow_lone_block(parent);
                }
            }
            _ => {}
        }
        true
    }

    // port: CheckUselessBlocks#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_block(t) && !self.lone_blocks.is_empty() && self.lone_blocks.last() == Some(&n) {
            self.lone_blocks.pop();
            t.report(n, &USELESS_BLOCK, &[]);
        }
    }
}
