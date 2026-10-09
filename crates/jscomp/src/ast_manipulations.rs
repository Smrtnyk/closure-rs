/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AstManipulations.java.

//! Port of `AstManipulations.java`: utilities for manipulating the AST.

use closure_rhino::{
    check_argument,
    node::{Ast, NodeId},
    token::Token,
};

// port: AstManipulations
pub struct AstManipulations;

impl AstManipulations {
    /// Returns a single node equivalent to executing `<expr1, expr2>`.
    ///
    /// Requires that expr1 and expr2 are detached (i.e. have no parent nodes). If exp2 is EMPTY
    /// this just returns exp1.
    // port: AstManipulations#fuseExpressions
    pub fn fuse_expressions(ast: &mut Ast, exp1: NodeId, exp2: NodeId) -> NodeId {
        check_argument!(
            exp1.get_parent(ast).is_none(),
            "Expected detached node, got %s",
            exp1.to_string(ast)
        );
        check_argument!(
            exp2.get_parent(ast).is_none(),
            "Expected detached node, got %s",
            exp2.to_string(ast)
        );
        if exp2.is_empty(ast) {
            return exp1;
        }
        let comma = ast.new_node_with_child(Token::COMMA, exp1);
        comma.srcref_if_missing(ast, exp2);

        // We can just join the new comma expression with another comma but
        // lets keep all the comma's in a straight line. That way we can use
        // tree comparison.
        if exp2.is_comma(ast) {
            let mut left_most_child = exp2;
            while left_most_child.is_comma(ast) {
                left_most_child = left_most_child.get_first_child(ast).unwrap();
            }
            let parent = left_most_child.get_parent(ast).unwrap();
            let detached = left_most_child.detach(ast);
            comma.add_child_to_back(ast, detached);
            parent.add_child_to_front(ast, comma);
            exp2
        } else {
            comma.add_child_to_back(ast, exp2);
            comma
        }
    }
}
