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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ThrowStatementTree.java.

//! Port of ThrowStatementTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct ThrowStatementTree {
    pub value: Option<Tree>,
}

impl ThrowStatementTree {
    // port: ThrowStatementTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, value: impl Into<Option<Tree>>) -> Tree {
        ParseTree::new(
            ParseTreeType::THROW_STATEMENT,
            location,
            ParseTreeData::ThrowStatementTree(Box::new(Self {
                value: value.into(),
            })),
        )
    }
}
