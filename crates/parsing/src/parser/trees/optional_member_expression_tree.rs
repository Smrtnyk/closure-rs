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
//   src/com/google/javascript/jscomp/parsing/parser/trees/OptionalMemberExpressionTree.java.

//! Port of OptionalMemberExpressionTree.java.
use super::*;

// Creates a OptionalMemberExpressionTree that represents a getprop expression within an optional
// chain. Has an isOptionalChainStart field to indicate whether it is the start of an optional
// chain.
#[derive(Clone, Debug)]
pub struct OptionalMemberExpressionTree {
    pub operand: Tree,
    pub member_name: Option<Token>,
    pub is_start_of_optional_chain: bool,
}

impl OptionalMemberExpressionTree {
    // port: OptionalMemberExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        operand: Tree,
        member_name: impl Into<Option<Token>>,
        is_start_of_optional_chain: bool,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::OPT_CHAIN_MEMBER_EXPRESSION,
            location,
            ParseTreeData::OptionalMemberExpressionTree(Box::new(Self {
                operand,
                member_name: member_name.into(),
                is_start_of_optional_chain,
            })),
        )
    }
}
