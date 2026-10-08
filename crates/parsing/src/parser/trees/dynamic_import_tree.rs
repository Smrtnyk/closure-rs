/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/trees/DynamicImportTree.java.

//! Port of DynamicImportTree.java.
use super::*;

// Represents a dynamic import expression.
//
// <p>e.g. `import(myModuleName)` in `const modulePromise = import(myModuleName)`.
#[derive(Clone, Debug)]
pub struct DynamicImportTree {
    pub argument: Tree,
}

impl DynamicImportTree {
    // port: DynamicImportTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, argument: Tree) -> Tree {
        ParseTree::new(
            ParseTreeType::DYNAMIC_IMPORT_EXPRESSION,
            location,
            ParseTreeData::DynamicImportTree(Box::new(Self { argument })),
        )
    }
}
