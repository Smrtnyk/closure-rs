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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ForAwaitOfStatementTree.java.

//! Port of ForAwaitOfStatementTree.java.
use super::*;

// Represents a for-await-of statement, which was added in ES2018
#[derive(Clone, Debug)]
pub struct ForAwaitOfStatementTree {
    pub initializer: Tree,
    pub collection: Tree,
    pub body: Tree,
}

impl ForAwaitOfStatementTree {
    // port: ForAwaitOfStatementTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, initializer: Tree, collection: Tree, body: Tree) -> Tree {
        ParseTree::new(
            ParseTreeType::FOR_AWAIT_OF_STATEMENT,
            location,
            ParseTreeData::ForAwaitOfStatementTree(Box::new(Self {
                initializer,
                collection,
                body,
            })),
        )
    }
}
