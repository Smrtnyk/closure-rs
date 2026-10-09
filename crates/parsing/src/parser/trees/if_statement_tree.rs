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
//   src/com/google/javascript/jscomp/parsing/parser/trees/IfStatementTree.java.

//! Port of IfStatementTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct IfStatementTree {
    pub condition: Tree,
    pub if_clause: Tree,
    pub else_clause: Option<Tree>,
}

impl IfStatementTree {
    // port: IfStatementTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        condition: Tree,
        if_clause: Tree,
        else_clause: impl Into<Option<Tree>>,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::IF_STATEMENT,
            location,
            ParseTreeData::IfStatementTree(Box::new(Self {
                condition,
                if_clause,
                else_clause: else_clause.into(),
            })),
        )
    }
}
