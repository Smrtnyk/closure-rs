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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/parsing/parser/trees/OptChainCallExpressionTree.java.

//! Port of OptChainCallExpressionTree.java.
use super::*;

// Creates a OptChainCallExpressionTree that represents a call expression within an optional chain.
// Has an isOptionalChainStart field to indicate whether it is the start of an optional chain.
#[derive(Clone, Debug)]
pub struct OptChainCallExpressionTree {
    pub operand: Tree,
    pub arguments: Tree,
    pub is_start_of_optional_chain: bool,
    pub has_trailing_comma: bool,
}

impl OptChainCallExpressionTree {
    // port: OptChainCallExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        operand: Tree,
        arguments: Tree,
        is_start_of_optional_chain: bool,
        has_trailing_comma: bool,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::OPT_CHAIN_CALL_EXPRESSION,
            location,
            ParseTreeData::OptChainCallExpressionTree(Box::new(Self {
                operand,
                arguments,
                is_start_of_optional_chain,
                has_trailing_comma,
            })),
        )
    }
}
