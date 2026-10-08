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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ObjectPatternTree.java.

//! Port of ObjectPatternTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct ObjectPatternTree {
    pub fields: Vec<Tree>,
}

impl ObjectPatternTree {
    // port: ObjectPatternTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, fields: Vec<Tree>) -> Tree {
        ParseTree::new(
            ParseTreeType::OBJECT_PATTERN,
            location,
            ParseTreeData::ObjectPatternTree(Box::new(Self { fields })),
        )
    }
}
