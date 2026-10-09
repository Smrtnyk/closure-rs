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
//   src/com/google/javascript/jscomp/parsing/parser/trees/NewTargetExpressionTree.java.

//! Port of NewTargetExpressionTree.java.
use super::*;

// Represents 'new.target'.
#[derive(Clone, Debug)]
pub struct NewTargetExpressionTree {}

impl NewTargetExpressionTree {
    // port: NewTargetExpressionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange) -> Tree {
        ParseTree::new(
            ParseTreeType::NEW_TARGET_EXPRESSION,
            location,
            ParseTreeData::NewTargetExpressionTree(Box::new(Self {})),
        )
    }
}
