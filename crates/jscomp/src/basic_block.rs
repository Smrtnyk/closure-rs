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
//   src/com/google/javascript/jscomp/BasicBlock.java.

//! Port of `BasicBlock.java`.

use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::rc::Rc;

/// Represents a section of code that is uninterrupted by control structures (conditional or
/// iterative logic).
///
/// Java compares blocks by identity; blocks are shared as `Rc<BasicBlock>` and compared with
/// `Rc::ptr_eq`. A reference collection stays on the thread that makes it, so its blocks are
/// counted without atomic operations (DECISIONS.md D-025).
#[derive(Debug)]
pub struct BasicBlock {
    parent: Option<Rc<BasicBlock>>,

    root: NodeId,

    /// Whether this block denotes a function scope.
    is_function: bool,

    /// Whether this block denotes a loop.
    is_loop: bool,
}

impl BasicBlock {
    /// Creates a new block.
    ///
    /// `parent`: The containing block. `root`: The root node of the block.
    // port: BasicBlock#BasicBlock
    pub fn new(ast: &Ast, parent: Option<Rc<BasicBlock>>, root: NodeId) -> Rc<BasicBlock> {
        let is_function = root.is_function(ast);

        let is_loop = if root.has_parent(ast) {
            let p_type = root.get_parent(ast).unwrap().get_token(ast);
            p_type == Token::DO
                || p_type == Token::WHILE
                || p_type == Token::FOR
                || p_type == Token::FOR_OF
                || p_type == Token::FOR_AWAIT_OF
                || p_type == Token::FOR_IN
        } else {
            false
        };
        Rc::new(BasicBlock {
            parent,
            root,
            is_function,
            is_loop,
        })
    }

    // port: BasicBlock#getParent
    pub fn get_parent(&self) -> Option<&Rc<BasicBlock>> {
        self.parent.as_ref()
    }

    /// Determines whether this block is equivalent to the very first block that is created when
    /// reference collection traversal enters global scope. Note that when traversing a single
    /// script in a hot-swap fashion a new instance of `BasicBlock` is created.
    // port: BasicBlock#isGlobalScopeBlock
    pub fn is_global_scope_block(&self) -> bool {
        self.get_parent().is_none()
    }

    /// Determines whether this block is guaranteed to begin executing before the given block
    /// does.
    // port: BasicBlock#provablyExecutesBefore
    pub fn provably_executes_before(&self, that_block: &BasicBlock) -> bool {
        // If thatBlock is a descendant of this block, and there are no hoisted
        // blocks between them, then this block must start before thatBlock.
        let mut current_block: Option<&BasicBlock> = Some(that_block);
        while let Some(block) = current_block {
            if std::ptr::eq(block, self) {
                break;
            }
            current_block = block.get_parent().map(|p| &**p);
        }

        if current_block.is_some_and(|block| std::ptr::eq(block, self)) {
            return true;
        }
        self.is_global_scope_block() && that_block.is_global_scope_block()
    }

    // port: BasicBlock#isFunction
    pub fn is_function(&self) -> bool {
        self.is_function
    }

    // port: BasicBlock#isLoop
    pub fn is_loop(&self) -> bool {
        self.is_loop
    }

    // port: BasicBlock#getRoot
    pub fn get_root(&self) -> NodeId {
        self.root
    }

    // port: BasicBlock#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        format!("BasicBlock @ {}", self.root.to_string(ast))
    }
}
