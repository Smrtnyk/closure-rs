/*
 * Copyright 2013 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/MinimizedCondition.java.

//! Port of `MinimizedCondition.java`.
//!
//! A class that represents a minimized conditional expression. This is a conditional expression
//! that has been massaged according to DeMorgan's laws in order to minimize the length of the
//! source representation.
//!
//! Depending on the context, a leading NOT node in front of the conditional may or may not be
//! counted as a cost, so this class provides ways to access minimized versions of both of those
//! abstract syntax trees (ASTs).
//!
//! Java's `MeasuredNode` objects are immutable and share their children arrays; here a
//! `MeasuredNode` is a cheap-clone value whose children array is an `Rc`, which keeps that
//! sharing.

use crate::node_util::NodeUtil;
use closure_rhino::{
    check_not_null, check_state,
    node::{Ast, NodeId},
    token::Token,
};
use std::rc::Rc;

/// Definitions of the style of minimization preferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimizationStyle {
    /// Compute the length of the minimized condition as including any leading NOT node, if
    /// present.
    PREFER_UNNEGATED,
    /// Compute the length of the minimized condition without penalizing a leading NOT node, if
    /// present.
    ALLOW_LEADING_NOT,
}

pub struct MinimizedCondition {
    /// A representation equivalent to the original condition.
    positive: MeasuredNode,

    /// A representation equivalent to the negation of the original condition.
    negative: MeasuredNode,
}

impl MinimizedCondition {
    // port: MinimizedCondition#MinimizedCondition
    fn new(p: MeasuredNode, n: MeasuredNode) -> Self {
        Self {
            positive: p,
            negative: n.change(),
        }
    }

    /// Returns a MinimizedCondition that represents the condition node after minimization.
    // port: MinimizedCondition#fromConditionNode
    pub fn from_condition_node(ast: &mut Ast, n: NodeId) -> MinimizedCondition {
        check_state!(n.has_parent(ast));
        match n.get_token(ast) {
            Token::NOT | Token::AND | Token::OR | Token::HOOK | Token::COMMA => {
                Self::compute_minimized_condition(ast, n)
            }
            _ => Self::unoptimized(ast, n),
        }
    }

    /// Return the shorter representation of the original condition node.
    ///
    /// Depending on the context, this may require to either penalize or not the existence of a
    /// leading NOT node.
    ///
    /// - When `style` is `PREFER_UNNEGATED`, simply try to minimize the total length of the
    ///   conditional.
    /// - When `style` is `ALLOW_LEADING_NOT`, prefer the right side in cases such as:
    ///   `!x || !y || z  ==>  !(x && y && !z)`. This is useful in contexts such as IFs or HOOKs
    ///   where subsequent optimizations can efficiently deal with leading NOTs.
    ///
    /// Returns the minimized condition MeasuredNode, with equivalent semantics to that passed to
    /// `from_condition_node`.
    // port: MinimizedCondition#getMinimized
    pub fn get_minimized(&self, ast: &mut Ast, style: MinimizationStyle) -> MeasuredNode {
        if style == MinimizationStyle::PREFER_UNNEGATED
            || self.positive.node.unwrap().is_not(ast)
            || self.positive.length <= self.negative.length
        {
            self.positive.clone()
        } else {
            self.negative.add_not(ast)
        }
    }

    /// Return a MeasuredNode of the given condition node, without minimizing the result.
    ///
    /// Since a MinimizedCondition necessarily must contain two trees, this method sets the
    /// negative side to a invalid node with an unreasonably high length so that it will never be
    /// chosen by `get_minimized`.
    // port: MinimizedCondition#unoptimized
    pub fn unoptimized(ast: &Ast, n: NodeId) -> MinimizedCondition {
        check_not_null!(n.get_parent(ast));
        let pos = MeasuredNode::new(Some(n), None, 0, false);
        let neg = MeasuredNode::new(None, None, i32::MAX, true);
        MinimizedCondition::new(pos, neg)
    }

    /// return the best, prefer unchanged
    // port: MinimizedCondition#pickBest
    pub fn pick_best(a: MeasuredNode, b: MeasuredNode) -> MeasuredNode {
        if a.length == b.length {
            return if b.is_changed() { a } else { b };
        }

        if a.length < b.length { a } else { b }
    }

    /// Minimize the condition at the given node.
    // port: MinimizedCondition#computeMinimizedCondition
    fn compute_minimized_condition(ast: &mut Ast, n: NodeId) -> MinimizedCondition {
        match n.get_token(ast) {
            Token::NOT => {
                let first = n.get_first_child(ast).unwrap();
                let subtree = Self::compute_minimized_condition(ast, first);
                let positive = Self::pick_best(
                    MeasuredNode::add_node(ast, n, std::slice::from_ref(&subtree.positive)),
                    subtree.negative.clone(),
                );
                let negative = Self::pick_best(
                    // since parent node `n` is a NOT, we need to negate the subtree's
                    // computed `negative` to obtain the parent `n`'s real negative.
                    subtree.negative.negate(ast),
                    subtree.positive,
                );
                MinimizedCondition::new(positive, negative)
            }
            Token::AND | Token::OR => {
                let complement_node = ast
                    .new_node(if n.is_and(ast) { Token::OR } else { Token::AND })
                    .srcref(ast, n);
                let first = n.get_first_child(ast).unwrap();
                let left_subtree = Self::compute_minimized_condition(ast, first);
                let last = n.get_last_child(ast).unwrap();
                let right_subtree = Self::compute_minimized_condition(ast, last);
                let positive = Self::pick_best(
                    MeasuredNode::add_node(
                        ast,
                        n,
                        &[
                            left_subtree.positive.clone(),
                            right_subtree.positive.clone(),
                        ],
                    ),
                    MeasuredNode::add_node(
                        ast,
                        complement_node,
                        &[
                            left_subtree.negative.clone(),
                            right_subtree.negative.clone(),
                        ],
                    )
                    .negate(ast),
                );
                let negative = Self::pick_best(
                    MeasuredNode::add_node(
                        ast,
                        n,
                        &[left_subtree.positive, right_subtree.positive],
                    )
                    .negate(ast),
                    MeasuredNode::add_node(
                        ast,
                        complement_node,
                        &[left_subtree.negative, right_subtree.negative],
                    )
                    .change(),
                );
                MinimizedCondition::new(positive, negative)
            }
            Token::HOOK => {
                let cond = n.get_first_child(ast).unwrap();
                let then_node = cond.get_next(ast).unwrap();
                let else_node = then_node.get_next(ast).unwrap();
                let then_subtree = Self::compute_minimized_condition(ast, then_node);
                let else_subtree = Self::compute_minimized_condition(ast, else_node);
                let positive = MeasuredNode::add_node(
                    ast,
                    n,
                    &[
                        MeasuredNode::for_node(cond),
                        then_subtree.positive,
                        else_subtree.positive,
                    ],
                );
                let negative = MeasuredNode::add_node(
                    ast,
                    n,
                    &[
                        MeasuredNode::for_node(cond),
                        then_subtree.negative,
                        else_subtree.negative,
                    ],
                );
                MinimizedCondition::new(positive, negative)
            }
            Token::COMMA => {
                let lhs = n.get_first_child(ast).unwrap();
                let rhs = lhs.get_next(ast).unwrap();
                let rhs_subtree = Self::compute_minimized_condition(ast, rhs);
                let positive = MeasuredNode::add_node(
                    ast,
                    n,
                    &[MeasuredNode::for_node(lhs), rhs_subtree.positive],
                );
                let negative = MeasuredNode::add_node(
                    ast,
                    n,
                    &[MeasuredNode::for_node(lhs), rhs_subtree.negative],
                );
                MinimizedCondition::new(positive, negative)
            }
            _ => {
                let pos = MeasuredNode::for_node(n);
                let neg = pos.negate(ast);
                MinimizedCondition::new(pos, neg)
            }
        }
    }
}

/// An AST-node along with some additional metadata.
#[derive(Clone)]
pub struct MeasuredNode {
    node: Option<NodeId>,
    length: i32,
    changed: bool,
    children: Option<Rc<[MeasuredNode]>>,
}

impl MeasuredNode {
    // port: MinimizedCondition.MeasuredNode#MeasuredNode
    fn new(n: Option<NodeId>, children: Option<Rc<[MeasuredNode]>>, len: i32, ch: bool) -> Self {
        Self {
            node: n,
            children,
            length: len,
            changed: ch,
        }
    }

    /// Rust-only: Java's package-private `node` field, read by `PeepholeMinimizeConditions`.
    pub fn get_node(&self) -> Option<NodeId> {
        self.node
    }

    /// Rust-only: Java's package-private `length` field, read by `PeepholeMinimizeConditions`.
    pub fn get_length(&self) -> i32 {
        self.length
    }

    // port: MinimizedCondition.MeasuredNode#isChanged
    pub fn is_changed(&self) -> bool {
        self.changed
    }

    // port: MinimizedCondition.MeasuredNode#isNot
    pub fn is_not(&self, ast: &Ast) -> bool {
        self.node.unwrap().is_not(ast)
    }

    // port: MinimizedCondition.MeasuredNode#withoutNot
    pub fn without_not(&self, ast: &Ast) -> MeasuredNode {
        check_state!(self.is_not(ast));
        let children = Self::normalize_children(ast, self.node.unwrap(), self.children.clone());
        children.unwrap()[0].change()
    }

    // port: MinimizedCondition.MeasuredNode#negate
    fn negate(&self, ast: &mut Ast) -> MeasuredNode {
        match self.node.unwrap().get_token(ast) {
            Token::EQ => self.update_token(ast, Token::NE),
            Token::NE => self.update_token(ast, Token::EQ),
            Token::SHEQ => self.update_token(ast, Token::SHNE),
            Token::SHNE => self.update_token(ast, Token::SHEQ),
            Token::NOT => self.without_not(ast),
            _ => self.add_not(ast),
        }
    }

    // port: MinimizedCondition.MeasuredNode#normalizeChildren
    pub fn normalize_children(
        ast: &Ast,
        node: NodeId,
        children: Option<Rc<[MeasuredNode]>>,
    ) -> Option<Rc<[MeasuredNode]>> {
        if children.is_some() || !node.has_children(ast) {
            children
        } else {
            let mut measured_children: Vec<MeasuredNode> =
                Vec::with_capacity(node.get_child_count(ast) as usize);
            let mut c = node.get_first_child(ast);
            while let Some(cur) = c {
                measured_children.push(Self::for_node(cur));
                c = cur.get_next(ast);
            }
            Some(measured_children.into())
        }
    }

    // port: MinimizedCondition.MeasuredNode#updateToken
    fn update_token(&self, ast: &mut Ast, token: Token) -> MeasuredNode {
        let node = self.node.unwrap();
        let new_node = ast.new_node(token).srcref(ast, node);
        MeasuredNode::new(
            Some(new_node),
            Self::normalize_children(ast, node, self.children.clone()),
            self.length,
            true,
        )
    }

    // port: MinimizedCondition.MeasuredNode#addNot
    fn add_not(&self, ast: &mut Ast) -> MeasuredNode {
        let not = ast.new_node(Token::NOT).srcref(ast, self.node.unwrap());
        Self::add_node(ast, not, std::slice::from_ref(self)).change()
    }

    // port: MinimizedCondition.MeasuredNode#change
    fn change(&self) -> MeasuredNode {
        if self.is_changed() {
            self.clone()
        } else {
            MeasuredNode::new(self.node, self.children.clone(), self.length, true)
        }
    }

    /// Estimate the number of characters in the textual representation of the given node and
    /// that will be devoted to negation or parentheses. Since these are the only characters that
    /// flipping a condition according to De Morgan's rule can affect, these are the only ones we
    /// count. Not nodes are counted by the NOT node itself, whereas parentheses around an
    /// expression are counted by the parent node.
    ///
    /// Returns the number of negations and parentheses in the node.
    // port: MinimizedCondition.MeasuredNode#estimateCostOneLevel
    fn estimate_cost_one_level(ast: &Ast, n: NodeId, children: &[MeasuredNode]) -> i32 {
        let mut cost: i32 = 0;
        if n.is_not(ast) {
            cost += 1; // A negation is needed.
        }
        let parent_precedence = NodeUtil::precedence(n.get_token(ast));
        for child in children {
            if child.is_lower_precedence_than(ast, parent_precedence) {
                cost += 2; // A pair of parenthesis is needed.
            }
        }
        cost
    }

    /// Whether the node type has lower precedence than "precedence"
    // port: MinimizedCondition.MeasuredNode#isLowerPrecedenceThan
    pub fn is_lower_precedence_than(&self, ast: &Ast, precedence: i32) -> bool {
        NodeUtil::precedence(self.node.unwrap().get_token(ast)) < precedence
    }

    /// The returned MeasuredNode is only marked as changed if the children are marked as changed.
    // port: MinimizedCondition.MeasuredNode#addNode
    fn add_node(ast: &Ast, parent: NodeId, children: &[MeasuredNode]) -> MeasuredNode {
        let mut cost: i32 = 0;
        let mut changed = false;
        for child in children {
            cost = cost.wrapping_add(child.length);
            changed = changed || child.changed;
        }
        cost = cost.wrapping_add(Self::estimate_cost_one_level(ast, parent, children));
        MeasuredNode::new(Some(parent), Some(children.into()), cost, changed)
    }

    /// Return a MeasuredNode for a non-participating AST Node. This is used for leaf expression
    /// nodes.
    // port: MinimizedCondition.MeasuredNode#forNode
    fn for_node(n: NodeId) -> MeasuredNode {
        MeasuredNode::new(Some(n), None, 0, false)
    }

    /// Whether the MeasuredNode is a change from the original. This can either be a change within
    /// the original AST tree or a replacement of the original node.
    // port: MinimizedCondition.MeasuredNode#willChange
    pub fn will_change(&self, original: NodeId) -> bool {
        Some(original) != self.node || self.is_changed()
    }

    /// Update the AST for the result of this MeasuredNode. This can either be a change within the
    /// original AST tree or a replacement of the original node.
    // port: MinimizedCondition.MeasuredNode#applyTo
    pub fn apply_to(&self, ast: &mut Ast, original: NodeId) -> NodeId {
        check_state!(self.will_change(original));
        let replacement = self.build_replacement(ast);
        if original != replacement {
            Self::safe_detach(ast, replacement);
            original.replace_with(ast, replacement);
        }
        replacement
    }

    /// Detach a node only IIF it is in the tree
    // port: MinimizedCondition.MeasuredNode#safeDetach
    fn safe_detach(ast: &mut Ast, n: NodeId) -> NodeId {
        if n.has_parent(ast) { n.detach(ast) } else { n }
    }

    /// Build the final AST structure, detaching component Nodes as necessary from the original
    /// AST. The root Node, if currently attached is left attached to avoid the need to keep track
    /// of its position.
    // port: MinimizedCondition.MeasuredNode#buildReplacement
    pub fn build_replacement(&self, ast: &mut Ast) -> NodeId {
        let node = self.node.unwrap();
        if let Some(children) = &self.children {
            node.detach_children(ast);
            for child in children.iter() {
                let built = child.build_replacement(ast);
                let replacement_child = Self::safe_detach(ast, built);
                node.add_child_to_back(ast, replacement_child);
            }
        }
        node
    }
}
