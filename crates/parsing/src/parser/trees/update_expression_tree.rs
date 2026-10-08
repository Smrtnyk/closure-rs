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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/parsing/parser/trees/UpdateExpressionTree.java.

//! Port of UpdateExpressionTree.java.
use super::*;

// Position of the operator relative to the operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorPosition {
    PREFIX,
    POSTFIX,
}

// Represents UpdateExpression productions from the spec.
//
// <pre><code>
// UpdateExpression :=
//     { ++ | -- } UnaryExpression
//     LeftHandSideExpression [no LineTerminator here] { ++ | -- }
// </code></pre>
#[derive(Clone, Debug)]
pub struct UpdateExpressionTree {
    pub operator: Token,
    pub operator_position: OperatorPosition,
    pub operand: Tree,
}

impl UpdateExpressionTree {
    // port: UpdateExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        operator: Token,
        operator_position: OperatorPosition,
        operand: Tree,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::UPDATE_EXPRESSION,
            location,
            ParseTreeData::UpdateExpressionTree(Box::new(Self {
                operator,
                operator_position,
                operand,
            })),
        )
    }
    // port: UpdateExpressionTree#prefix
    pub fn prefix(location: SourceRange, operator: Token, operand: Tree) -> Tree {
        Self::new(location, operator, OperatorPosition::PREFIX, operand)
    }
    // port: UpdateExpressionTree#postfix
    pub fn postfix(location: SourceRange, operator: Token, operand: Tree) -> Tree {
        Self::new(location, operator, OperatorPosition::POSTFIX, operand)
    }
}
