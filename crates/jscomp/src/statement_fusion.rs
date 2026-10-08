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
//   src/com/google/javascript/jscomp/StatementFusion.java.

//! Port of `StatementFusion.java`.
//!
//! Tries to fuse all the statements in a block into a one statement by using COMMAs or
//! statements.
//!
//! Because COMMAs has the lowest precedence, we never need to insert extra () around. Once we
//! have only one statement in a block, we can then eliminate a pair of {}'s. Further more, we can
//! also fold a single statement IF into && or create further opportunities for all the other
//! goodies in `PeepholeMinimizeConditions`.
//!
//! NOTE(user): The current compiler assumes that there are more ;'s than ,'s in a real program,
//! and so it makes sense to prefer fusing statements with semicolons rather than commas. This
//! assumption has never been validated on a real program.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    ast_manipulations::AstManipulations,
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument,
    node::{Ast, NodeId},
    token::Token,
};

#[derive(Default)]
pub struct StatementFusion {
    fields: AbstractPeepholeOptimizationFields,
}

impl StatementFusion {
    // port: StatementFusion#StatementFusion
    pub fn new() -> Self {
        Self::default()
    }

    // port: StatementFusion#canFuseIntoOneStatement
    fn can_fuse_into_one_statement(&self, compiler: &mut AbstractCompiler, block: NodeId) -> bool {
        if !block.is_block(compiler) {
            return false;
        }

        // Nothing to do here.
        if !block.has_children(compiler) || block.has_one_child(compiler) {
            return false;
        }

        let last = block.get_last_child(compiler).unwrap();

        let mut c = block.get_first_child(compiler);
        while let Some(cur) = c {
            if !cur.is_expr_result(compiler) && cur != last {
                return false;
            }
            c = cur.get_next(compiler);
        }

        self.is_fusable_control_statement(compiler, last)
    }

    // port: StatementFusion#isFusableControlStatement
    fn is_fusable_control_statement(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        match n.get_token(compiler) {
            Token::IF | Token::THROW | Token::SWITCH | Token::EXPR_RESULT => {
                return true;
            }
            Token::RETURN => {
                // We don't want to add a new return value.
                return n.has_children(compiler);
            }
            Token::FOR => {
                // Avoid cases where we have for(var x;_;_) { ....
                return !NodeUtil::is_name_declaration(compiler, n.get_first_child(compiler));
            }
            Token::FOR_IN => {
                // Avoid cases where we have for(var x = foo() in a) { ....
                let first = n.get_first_child(compiler).unwrap();
                return !self.may_have_side_effects(compiler, first);
            }
            Token::LABEL => {
                let last = n.get_last_child(compiler).unwrap();
                return self.is_fusable_control_statement(compiler, last);
            }
            Token::BLOCK => {
                return (self.is_ast_normalized(compiler)
                    || NodeUtil::can_merge_block(compiler, n))
                    && !n.is_synthetic_block(compiler)
                    && {
                        let first = n.get_first_child(compiler).unwrap();
                        self.is_fusable_control_statement(compiler, first)
                    };
            }
            _ => {}
        }
        false
    }

    /// Given a block, fuse a list of statements with comma's.
    ///
    /// `first` is the first statement to fuse (inclusive), `last` the last statement to fuse
    /// (exclusive). Returns a single statement that contains all the fused statement as one.
    // port: StatementFusion#fuseIntoOneStatement
    fn fuse_into_one_statement(ast: &mut Ast, first: NodeId, last: NodeId) -> NodeId {
        // Nothing to fuse if there is only one statement.
        if first.get_next(ast) == Some(last) {
            return first;
        }

        // Step one: Create a comma tree that contains all the statements.
        let mut comma_tree = first.remove_first_child(ast).unwrap();

        let mut cur = first.get_next(ast).unwrap();
        while cur != last {
            let removed = cur.remove_first_child(ast).unwrap();
            comma_tree = AstManipulations::fuse_expressions(ast, comma_tree, removed);
            let next = cur.get_next(ast).unwrap();
            cur.detach(ast);
            cur = next;
        }

        // Step two: The last EXPR_RESULT will now hold the comma tree with all
        // the fused statements.
        first.add_child_to_back(ast, comma_tree);
        first
    }

    // port: StatementFusion#fuseExpressionIntoControlFlowStatement
    fn fuse_expression_into_control_flow_statement(ast: &mut Ast, before: NodeId, control: NodeId) {
        check_argument!(
            before.is_expr_result(ast),
            "before must be expression result"
        );

        // Now we are just left with two statements. The comma tree of the first
        // n - 1 statements (which can be used in an expression) and the last
        // statement. We perform specific fusion based on the last statement's type.
        match control.get_token(ast) {
            Token::IF
            | Token::RETURN
            | Token::THROW
            | Token::SWITCH
            | Token::EXPR_RESULT
            | Token::FOR => {
                before.detach(ast);
                let exp = before.remove_first_child(ast).unwrap();
                Self::fuse_expression_into_first_child(ast, exp, control);
            }
            Token::FOR_IN => {
                before.detach(ast);
                let exp = before.remove_first_child(ast).unwrap();
                Self::fuse_expression_into_second_child(ast, exp, control);
            }
            Token::LABEL => {
                let last = control.get_last_child(ast).unwrap();
                Self::fuse_expression_into_control_flow_statement(ast, before, last);
            }
            Token::BLOCK => {
                let first = control.get_first_child(ast).unwrap();
                Self::fuse_expression_into_control_flow_statement(ast, before, first);
            }
            _ => panic!("Statement fusion missing."),
        }
    }

    // port: StatementFusion#fuseExpressionIntoFirstChild
    fn fuse_expression_into_first_child(ast: &mut Ast, exp: NodeId, stmt: NodeId) {
        let val = stmt.remove_first_child(ast).unwrap();
        let comma = AstManipulations::fuse_expressions(ast, exp, val);
        stmt.add_child_to_front(ast, comma);
    }

    // port: StatementFusion#fuseExpressionIntoSecondChild
    fn fuse_expression_into_second_child(ast: &mut Ast, exp: NodeId, stmt: NodeId) {
        let val = stmt.get_second_child(ast).unwrap().detach(ast);
        let comma = AstManipulations::fuse_expressions(ast, exp, val);
        let first = stmt.get_first_child(ast).unwrap();
        comma.insert_after(ast, first);
    }
}

impl AbstractPeepholeOptimization for StatementFusion {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.StatementFusion"
    }

    // port: StatementFusion#optimizeSubtree
    fn optimize_subtree(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        if n.get_parent(compiler).unwrap().is_function(compiler)
            || !self.can_fuse_into_one_statement(compiler, n)
        {
            return Some(n);
        }

        let start = n.get_first_child(compiler).unwrap();
        let end = n.get_last_child(compiler).unwrap();
        let result = Self::fuse_into_one_statement(compiler, start, end);
        let last = n.get_last_child(compiler).unwrap();
        Self::fuse_expression_into_control_flow_statement(compiler, result, last);

        self.report_change_to_enclosing_scope(compiler, n);
        Some(n)
    }
}
