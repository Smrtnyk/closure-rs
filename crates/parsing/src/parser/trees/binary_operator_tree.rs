/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/trees/BinaryOperatorTree.java.

//! Port of BinaryOperatorTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct BinaryOperatorTree {
    pub left: Tree,
    pub operator: Token,
    pub right: Tree,
}

impl BinaryOperatorTree {
    // port: BinaryOperatorTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, left: Tree, operator: Token, right: Tree) -> Tree {
        ParseTree::new(
            ParseTreeType::BINARY_OPERATOR,
            location,
            ParseTreeData::BinaryOperatorTree(Box::new(Self {
                left,
                operator,
                right,
            })),
        )
    }
}
