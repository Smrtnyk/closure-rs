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
//   src/com/google/javascript/jscomp/parsing/parser/trees/NewExpressionTree.java.

//! Port of NewExpressionTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct NewExpressionTree {
    pub operand: Tree,
    pub arguments: Option<Tree>,
    pub has_trailing_comma: bool,
}

impl NewExpressionTree {
    // port: NewExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        operand: Tree,
        arguments: impl Into<Option<Tree>>,
        has_trailing_comma: bool,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::NEW_EXPRESSION,
            location,
            ParseTreeData::NewExpressionTree(Box::new(Self {
                operand,
                arguments: arguments.into(),
                has_trailing_comma,
            })),
        )
    }
}
