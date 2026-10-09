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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ForStatementTree.java.

//! Port of ForStatementTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct ForStatementTree {
    pub initializer: Option<Tree>,
    pub condition: Option<Tree>,
    pub increment: Option<Tree>,
    pub body: Tree,
}

impl ForStatementTree {
    // port: ForStatementTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        variables: impl Into<Option<Tree>>,
        condition: impl Into<Option<Tree>>,
        increment: impl Into<Option<Tree>>,
        body: Tree,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::FOR_STATEMENT,
            location,
            ParseTreeData::ForStatementTree(Box::new(Self {
                initializer: variables.into(),
                condition: condition.into(),
                increment: increment.into(),
                body,
            })),
        )
    }
}
