/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/MinimizeExitPoints.java.

//! Port of `MinimizeExitPoints.java`.
//!
//! Transform the structure of the AST so that the number of explicit exits are minimized and
//! instead flows to implicit exits conditions.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    jscomp_base::Tri,
    node::{Ast, NodeId},
    token::Token,
};

#[derive(Default)]
pub struct MinimizeExitPoints {
    fields: AbstractPeepholeOptimizationFields,
}

impl MinimizeExitPoints {
    // port: MinimizeExitPoints#MinimizeExitPoints
    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPeepholeOptimization for MinimizeExitPoints {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.MinimizeExitPoints"
    }

    // port: MinimizeExitPoints#optimizeSubtree
    fn optimize_subtree(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        check_state!(
            self.is_ast_normalized(compiler),
            "MinimizeExitPoints requires the AST to be normalized"
        );
        match n.get_token(compiler) {
            Token::LABEL => {
                let last_child = n.get_last_child(compiler).unwrap();
                let label_name = n.get_first_child(compiler).unwrap().get_string(compiler);
                self.try_minimize_exits(compiler, last_child, Token::BREAK, Some(&label_name));
            }
            Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF | Token::WHILE => {
                let code_block = NodeUtil::get_loop_code_block(compiler, n).unwrap();
                self.try_minimize_exits(compiler, code_block, Token::CONTINUE, None);
            }
            Token::DO => {
                let code_block = NodeUtil::get_loop_code_block(compiler, n).unwrap();
                self.try_minimize_exits(compiler, code_block, Token::CONTINUE, None);

                let cond = NodeUtil::get_condition_expression(compiler, n).unwrap();
                if self.get_side_effect_free_boolean_value(compiler, cond) == Tri::FALSE {
                    // Normally, we wouldn't be able to optimize BREAKs inside a loop
                    // but as we know the condition will always be false, we can treat them
                    // as we would a CONTINUE.
                    let first_child = n.get_first_child(compiler).unwrap();
                    self.try_minimize_exits(compiler, first_child, Token::BREAK, None);
                }
            }
            Token::BLOCK
                if n.has_parent(compiler)
                    && n.get_parent(compiler).unwrap().is_function(compiler) =>
            {
                self.try_minimize_exits(compiler, n, Token::RETURN, None);
            }
            Token::SWITCH => self.try_minimize_switch_exits(compiler, n, Token::BREAK, None),
            // TODO(johnlenz): Minimize any block that ends in a optimizable statements:
            //   break, continue, return
            _ => {}
        }
        Some(n)
    }
}

impl MinimizeExitPoints {
    /// Attempts to minimize the number of explicit exit points in a control structure to take
    /// advantage of the implied exit at the end of the structure. This is accomplished by
    /// removing redundant statements, and moving statements following a qualifying IF node into
    /// that node. For example:
    ///
    /// ```text
    /// function () {
    ///   if (x) return;
    ///   else blah();
    ///   foo();
    /// }
    /// ```
    ///
    /// becomes:
    ///
    /// ```text
    /// function () {
    ///  if (x) ;
    ///  else {
    ///    blah();
    ///    foo();
    ///  }
    /// ```
    ///
    /// `n` is the execution node of a parent to inspect, `exit_type` the type of exit to look
    /// for, and `label_name` (if parent is a label) the name of the label to look for, `None`
    /// otherwise. Non-null only for breaks within labels.
    // port: MinimizeExitPoints#tryMinimizeExits
    pub fn try_minimize_exits(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        exit_type: Token,
        label_name: Option<&JsString>,
    ) {
        // Just an 'exit'.
        if Self::matching_exit_node(compiler, n, exit_type, label_name) {
            self.report_change_to_enclosing_scope(compiler, n);
            let parent = n.get_parent(compiler).unwrap();
            NodeUtil::remove_child(compiler, parent, n);
            return;
        }

        // Just an 'if'.
        if n.is_if(compiler) {
            let if_block = n.get_second_child(compiler).unwrap();
            self.try_minimize_exits(compiler, if_block, exit_type, label_name);
            let else_block = if_block.get_next(compiler);
            if let Some(else_block) = else_block {
                self.try_minimize_exits(compiler, else_block, exit_type, label_name);
            }
            return;
        }

        // Just a 'try/catch/finally'.
        if n.is_try(compiler) {
            let try_block = n.get_first_child(compiler).unwrap();
            self.try_minimize_exits(compiler, try_block, exit_type, label_name);
            let all_catch_nodes = NodeUtil::get_catch_block(compiler, n);
            if NodeUtil::has_catch_handler(compiler, all_catch_nodes) {
                check_state!(all_catch_nodes.has_one_child(compiler));
                let catch_node = all_catch_nodes.get_first_child(compiler).unwrap();
                let catch_code_block = catch_node.get_last_child(compiler).unwrap();
                self.try_minimize_exits(compiler, catch_code_block, exit_type, label_name);
            }
            /* Don't try to minimize the exits of finally blocks, as this
             * can cause problems if it changes the completion type of the finally
             * block. See ECMA 262 Sections 8.9 & 12.14
             */
        }

        // Just a 'label'.
        if n.is_label(compiler) {
            let label_block = n.get_last_child(compiler).unwrap();
            self.try_minimize_exits(compiler, label_block, exit_type, label_name);
        }

        // We can only minimize switch cases if we are not trying to remove unlabeled breaks.
        if n.is_switch(compiler) && (exit_type != Token::BREAK || label_name.is_some()) {
            self.try_minimize_switch_exits(compiler, n, exit_type, label_name);
            return;
        }

        // The rest assumes a block with at least one child, bail on anything else.
        if !n.is_block(compiler) || !n.has_children(compiler) {
            return;
        }

        // Multiple if-exits can be converted in a single pass.
        // Convert "if (blah) break;  if (blah2) break; other_stmt;" to
        // become "if (blah); else { if (blah2); else { other_stmt; } }"
        // which will get converted to "if (!blah && !blah2) { other_stmt; }".
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            // An 'if' block to process below.
            if cur.is_if(compiler) {
                let if_tree = cur;

                // First, the true condition block.
                let mut true_block = if_tree.get_second_child(compiler).unwrap();
                let mut false_block = true_block.get_next(compiler);
                self.try_minimize_if_block_exits(
                    compiler,
                    true_block,
                    false_block,
                    if_tree,
                    exit_type,
                    label_name,
                );

                // Now the else block.
                // The if blocks may have changed, get them again.
                true_block = if_tree.get_second_child(compiler).unwrap();
                false_block = true_block.get_next(compiler);
                if let Some(false_block) = false_block {
                    self.try_minimize_if_block_exits(
                        compiler,
                        false_block,
                        Some(true_block),
                        if_tree,
                        exit_type,
                        label_name,
                    );
                }
            }

            if Some(cur) == n.get_last_child(compiler) {
                break;
            }
            c = cur.get_next(compiler);
        }

        // Now try to minimize the exits of the last child, if it is removed
        // look at what has become the last child.
        let mut c = n.get_last_child(compiler);
        while let Some(cur) = c {
            self.try_minimize_exits(compiler, cur, exit_type, label_name);
            // If the node is still the last child, we are done.
            if Some(cur) == n.get_last_child(compiler) {
                break;
            }
            c = n.get_last_child(compiler);
        }
    }

    // port: MinimizeExitPoints#tryMinimizeSwitchExits
    pub fn try_minimize_switch_exits(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        exit_type: Token,
        label_name: Option<&JsString>,
    ) {
        check_state!(n.is_switch(compiler));
        // Skipping the switch condition, visit all the children.
        let switch_body = n.get_second_child(compiler).unwrap();
        let mut c = switch_body.get_first_child(compiler);
        while let Some(cur) = c {
            if Some(cur) != switch_body.get_last_child(compiler) {
                self.try_minimize_switch_case_exits(compiler, cur, exit_type, label_name);
            } else {
                // Last case, the last case block can be optimized more aggressively.
                let last_child = cur.get_last_child(compiler).unwrap();
                self.try_minimize_exits(compiler, last_child, exit_type, label_name);
            }
            c = cur.get_next(compiler);
        }
    }

    /// Attempt to remove explicit exits from switch cases that also occur implicitly after the
    /// switch.
    // port: MinimizeExitPoints#tryMinimizeSwitchCaseExits
    pub fn try_minimize_switch_case_exits(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        exit_type: Token,
        label_name: Option<&JsString>,
    ) {
        check_state!(NodeUtil::is_switch_case(compiler, n));

        check_state!(Some(n) != n.get_parent(compiler).unwrap().get_last_child(compiler));
        let block = n.get_last_child(compiler).unwrap();
        let maybe_break = block.get_last_child(compiler);
        let Some(maybe_break) = maybe_break else {
            // Can not minimize exits from a case without an explicit break from the switch.
            return;
        };
        if !maybe_break.is_break(compiler) || maybe_break.has_children(compiler) {
            // Can not minimize exits from a case without an explicit break from the switch.
            return;
        }

        // Now try to minimize the exits of the last child before the break, if it is removed
        // look at what has become the child before the break.
        let mut child_before_break = maybe_break.get_previous(compiler);
        while let Some(c) = child_before_break {
            self.try_minimize_exits(compiler, c, exit_type, label_name);
            // If the node is still the last child, we are done.
            child_before_break = maybe_break.get_previous(compiler);
            if Some(c) == child_before_break {
                break;
            }
        }
    }

    /// Look for exits (returns, breaks, or continues, depending on the context) at the end of a
    /// block and removes them by moving the if node's siblings, if any, into the opposite
    /// condition block.
    ///
    /// `src_block` is the block to inspect, `dest_block` the block to move sibling nodes into,
    /// `if_node` the if node to work with, `exit_type` the type of exit to look for and
    /// `label_name` the name associated with the exit, if any. `None` for anything excepted for
    /// named-break associated with a label.
    // port: MinimizeExitPoints#tryMinimizeIfBlockExits
    fn try_minimize_if_block_exits(
        &self,
        compiler: &mut AbstractCompiler,
        src_block: NodeId,
        dest_block: Option<NodeId>,
        if_node: NodeId,
        exit_type: Token,
        label_name: Option<&JsString>,
    ) {
        let exit_node_parent: NodeId;
        let exit_node: NodeId;

        // Pick an exit node candidate.
        if src_block.is_block(compiler) {
            if !src_block.has_children(compiler) {
                return;
            }
            exit_node_parent = src_block;
            exit_node = exit_node_parent.get_last_child(compiler).unwrap();
        } else {
            // Just a single statement, if it isn't an exit bail.
            exit_node_parent = if_node;
            exit_node = src_block;
        }
        let _ = exit_node_parent;

        // Verify the candidate.
        if !Self::matching_exit_node(compiler, exit_node, exit_type, label_name) {
            return;
        }

        // Ensure no block-scoped declarations are moved into an inner block.
        if !Self::try_convert_all_block_scoped_following(compiler, if_node) {
            return;
        }

        // Take case of the if nodes siblings, if any.
        if if_node.get_next(compiler).is_some() {
            // Move siblings of the if block into the opposite
            // logic block of the exit.
            let mut new_dest_block = IR::block(compiler).srcref(compiler, if_node);
            match dest_block {
                None => {
                    // Only possible if this is the false block.
                    if_node.add_child_to_back(compiler, new_dest_block);
                }
                Some(dest_block) if dest_block.is_empty(compiler) => {
                    // Use the new block.
                    dest_block.replace_with(compiler, new_dest_block);
                }
                Some(dest_block) if dest_block.is_block(compiler) => {
                    // Reuse the existing block.
                    new_dest_block = dest_block;
                }
                Some(dest_block) => {
                    // Add the existing statement to the new block.
                    dest_block.replace_with(compiler, new_dest_block);
                    new_dest_block.add_child_to_back(compiler, dest_block);
                }
            }

            // Move all the if node's following siblings.
            Self::move_all_following(compiler, if_node, new_dest_block);
            self.report_change_to_enclosing_scope(compiler, if_node);
        }
    }

    /// Determines if n matches the type and name for the following types of "exits":
    ///    - return without values
    ///    - continues and breaks with or without names.
    ///
    /// `n` is the node to inspect, `type_` the Token type to look for and `label_name` the name
    /// that must be associated with the exit type (non-null only for breaks associated with
    /// labels). Returns whether the node matches the specified block-exit type.
    // port: MinimizeExitPoints#matchingExitNode
    fn matching_exit_node(
        ast: &Ast,
        n: NodeId,
        type_: Token,
        label_name: Option<&JsString>,
    ) -> bool {
        if n.get_token(ast) == type_ {
            if type_ == Token::RETURN {
                // only returns without expressions.
                return !n.has_children(ast);
            } else {
                match label_name {
                    None => return !n.has_children(ast),
                    Some(label_name) => {
                        return n.has_children(ast)
                            && *label_name == n.get_first_child(ast).unwrap().get_string(ast);
                    }
                }
            }
        }
        false
    }

    /// Move all the child nodes following start in srcParent to the end of destParent's child
    /// list.
    ///
    /// `start` is the start point in the srcParent child list and `dest_parent` the destination
    /// node.
    // port: MinimizeExitPoints#moveAllFollowing
    fn move_all_following(ast: &mut Ast, start: NodeId, dest_parent: NodeId) {
        let mut n = start.get_next(ast);
        while let Some(cur) = n {
            let is_function_declaration = NodeUtil::is_function_declaration(ast, cur);
            cur.detach(ast);
            if is_function_declaration {
                dest_parent.add_child_to_front(ast, cur);
            } else {
                dest_parent.add_child_to_back(ast, cur);
            }
            n = start.get_next(ast);
        }
    }

    /// Convert all let/const declarations following the start node to var declarations if
    /// possible.
    ///
    /// See the unit tests for examples of why this is necessary before moving code into an inner
    /// block, and why this is unsafe to do to declarations inside a loop.
    ///
    /// Returns whether all block-scoped declarations have been converted.
    // port: MinimizeExitPoints#tryConvertAllBlockScopedFollowing
    fn try_convert_all_block_scoped_following(ast: &mut Ast, start: NodeId) -> bool {
        if NodeUtil::is_within_loop(ast, start) {
            // If in a loop, don't convert anything to a var. Return true only if there are no
            // let/consts.
            return !Self::has_block_scoped_vars_following(ast, start);
        }
        let mut n = start.get_next(ast);
        while let Some(cur) = n {
            if cur.is_let(ast) || cur.is_const(ast) {
                cur.set_token(ast, Token::VAR);
            }
            n = cur.get_next(ast);
        }
        true
    }

    /// Detect any block-scoped declarations that are younger siblings of the given starting
    /// point.
    // port: MinimizeExitPoints#hasBlockScopedVarsFollowing
    fn has_block_scoped_vars_following(ast: &Ast, start: NodeId) -> bool {
        let mut n = start.get_next(ast);
        while let Some(cur) = n {
            if cur.is_let(ast) || cur.is_const(ast) {
                return true;
            }
            n = cur.get_next(ast);
        }
        false
    }
}
