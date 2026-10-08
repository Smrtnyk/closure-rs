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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ImportSpecifierTree.java.

//! Port of ImportSpecifierTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct ImportSpecifierTree {
    pub imported_name: Token,
    pub destination_name: Option<Token>,
}

impl ImportSpecifierTree {
    // port: ImportSpecifierTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        imported_name: Token,
        destination_name: impl Into<Option<Token>>,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::IMPORT_SPECIFIER,
            location,
            ParseTreeData::ImportSpecifierTree(Box::new(Self {
                imported_name,
                destination_name: destination_name.into(),
            })),
        )
    }
}
