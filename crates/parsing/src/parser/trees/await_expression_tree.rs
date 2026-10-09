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
//   src/com/google/javascript/jscomp/parsing/parser/trees/AwaitExpressionTree.java.

//! Port of AwaitExpressionTree.java.
use super::*;

// Represents an {@literal await} expression.
//
// <pre>
//   <i>AwaitExpression</i> : <b>await</b> <i>UnaryExpression</i>
// </pre>
#[derive(Clone, Debug)]
pub struct AwaitExpressionTree {
    pub expression: Tree,
}

impl AwaitExpressionTree {
    // port: AwaitExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, expression: Tree) -> Tree {
        ParseTree::new(
            ParseTreeType::AWAIT_EXPRESSION,
            location,
            ParseTreeData::AwaitExpressionTree(Box::new(Self { expression })),
        )
    }
}
