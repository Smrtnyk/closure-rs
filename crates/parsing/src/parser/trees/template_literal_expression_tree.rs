/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/trees/TemplateLiteralExpressionTree.java.

//! Port of TemplateLiteralExpressionTree.java.
use super::*;

// Template literal production in ES6.
#[derive(Clone, Debug)]
pub struct TemplateLiteralExpressionTree {
    pub operand: Option<Tree>,
    pub elements: Vec<Tree>,
}

impl TemplateLiteralExpressionTree {
    // port: TemplateLiteralExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        operand: impl Into<Option<Tree>>,
        elements: Vec<Tree>,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::TEMPLATE_LITERAL_EXPRESSION,
            location,
            ParseTreeData::TemplateLiteralExpressionTree(Box::new(Self {
                operand: operand.into(),
                elements,
            })),
        )
    }
}
