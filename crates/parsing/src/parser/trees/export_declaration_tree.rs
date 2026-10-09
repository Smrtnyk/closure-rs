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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ExportDeclarationTree.java.

//! Port of ExportDeclarationTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct ExportDeclarationTree {
    // VariableStatement, Declaration (let, const, function or class),
    // or default with assignmentExpression
    pub is_default: bool,
    pub is_export_all: bool,
    pub declaration: Option<Tree>,
    pub export_specifier_list: Option<Vec<Tree>>,
    pub from: Option<Token>,
}

impl ExportDeclarationTree {
    // port: ExportDeclarationTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        is_default: bool,
        is_export_all: bool,
        declaration: impl Into<Option<Tree>>,
        export_specifier_list: impl Into<Option<Vec<Tree>>>,
        from: impl Into<Option<Token>>,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::EXPORT_DECLARATION,
            location,
            ParseTreeData::ExportDeclarationTree(Box::new(Self {
                is_default,
                is_export_all,
                declaration: declaration.into(),
                export_specifier_list: export_specifier_list.into(),
                from: from.into(),
            })),
        )
    }
}
