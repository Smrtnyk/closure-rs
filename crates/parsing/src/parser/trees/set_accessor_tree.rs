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
//   src/com/google/javascript/jscomp/parsing/parser/trees/SetAccessorTree.java.

//! Port of SetAccessorTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct SetAccessorTree {
    pub property_name: Token,
    pub parameter: Tree,
    pub is_class_member: bool,
    pub is_static: bool,
    pub body: Tree,
}

impl SetAccessorTree {
    // port: SetAccessorTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        property_name: Token,
        is_class_member: bool,
        is_static: bool,
        parameter: Tree,
        body: Tree,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::SET_ACCESSOR,
            location,
            ParseTreeData::SetAccessorTree(Box::new(Self {
                property_name,
                parameter,
                is_class_member,
                is_static,
                body,
            })),
        )
    }
}
