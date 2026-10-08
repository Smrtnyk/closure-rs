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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ImportDeclarationTree.java.

//! Port of ImportDeclarationTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct ImportDeclarationTree {
    // The identifier for the "default" import from the module.
    pub default_binding_identifier: Option<Token>,

    // The list of names imported from the module when using the
    // `import {...} from 'specifier'` form.
    pub import_specifier_list: Option<Vec<Tree>>,

    // The namespace into which imported names are put when using the
    // `import * as nameSpaceImportIdentifier from 'specifier'` form.
    pub name_space_import_identifier: Option<Token>,

    // The string identifying the module.
    pub module_specifier: Option<Token>,
}

impl ImportDeclarationTree {
    // port: ImportDeclarationTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        default_binding_identifier: impl Into<Option<Token>>,
        import_specifier_list: impl Into<Option<Vec<Tree>>>,
        name_space_import_identifier: impl Into<Option<Token>>,
        module_specifier: impl Into<Option<Token>>,
    ) -> Tree {
        ParseTree::new(
            ParseTreeType::IMPORT_DECLARATION,
            location,
            ParseTreeData::ImportDeclarationTree(Box::new(Self {
                default_binding_identifier: default_binding_identifier.into(),
                import_specifier_list: import_specifier_list.into(),
                name_space_import_identifier: name_space_import_identifier.into(),
                module_specifier: module_specifier.into(),
            })),
        )
    }
}
