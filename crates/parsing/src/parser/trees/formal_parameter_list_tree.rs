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
//   src/com/google/javascript/jscomp/parsing/parser/trees/FormalParameterListTree.java.

//! Port of FormalParameterListTree.java.
use super::*;

#[derive(Clone, Debug)]
pub struct FormalParameterListTree {
    pub parameters: Vec<Tree>,
    pub has_trailing_comma: bool,
    pub comma_positions: Vec<SourcePosition>,
}

impl FormalParameterListTree {
    // port: FormalParameterListTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        location: SourceRange,
        parameters: Vec<Tree>,
        has_trailing_comma: bool,
        comma_positions: Vec<SourcePosition>,
    ) -> Tree {
        let num_params = parameters.len() as i32;
        let num_commas = comma_positions.len() as i32;
        assert!(
            num_commas <= num_params && num_commas >= num_params - 1,
            "Unexpected # of comma and formal params.\nparams: {:?}\ncomma positions: {:?}",
            parameters,
            comma_positions
        );
        ParseTree::new(
            ParseTreeType::FORMAL_PARAMETER_LIST,
            location,
            ParseTreeData::FormalParameterListTree(Box::new(Self {
                parameters,
                has_trailing_comma,
                comma_positions,
            })),
        )
    }
}
