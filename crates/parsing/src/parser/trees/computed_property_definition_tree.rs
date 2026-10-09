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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/parsing/parser/trees/ComputedPropertyDefinitionTree.java.

//! Port of ComputedPropertyDefinitionTree.java.
use super::*;

// Represents a computed property in an object literal.
#[derive(Clone, Debug)]
pub struct ComputedPropertyDefinitionTree {
    pub property: Tree,
    pub value: Tree,
}

impl ComputedPropertyDefinitionTree {
    // port: ComputedPropertyDefinitionTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, property: Tree, value: Tree) -> Tree {
        ParseTree::new(
            ParseTreeType::COMPUTED_PROPERTY_DEFINITION,
            location,
            ParseTreeData::ComputedPropertyDefinitionTree(Box::new(Self { property, value })),
        )
    }
}
