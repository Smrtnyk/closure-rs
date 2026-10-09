/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DestructuringGlobalNameExtractor.java.

//! Port of `DestructuringGlobalNameExtractor.java`.

use crate::{
    abstract_compiler::AbstractCompiler,
    global_namespace::{AstChange, Ref, RefBasedAstChange},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    check_state,
    ir::IR,
    node::{Ast, NodeId},
    token::Token,
};

/// Helper for changing the value of an lvalue in a destructuring pattern. Intended for use by
/// `InlineAndCollapseProperties.CollapseProperties` and
/// `InlineAndCollapseProperties.AggressiveInlineAliases` only. This class makes some assumptions
/// that don't generally hold in order to preserve `GlobalNamespace` validity and avoid creating
/// temporary variables.
pub struct DestructuringGlobalNameExtractor;

impl DestructuringGlobalNameExtractor {
    /// Given an lvalue in a destructuring pattern, and a detached subtree, rewrites the AST to
    /// assign the lvalue to the subtree instead of its previous value, while preserving the rest
    /// of the destructuring pattern.
    ///
    /// `new_nodes` is optionally a set to add new AstChanges to, if you need to keep the
    /// GlobalNamespace up-to-date. Otherwise `None`.
    // port: DestructuringGlobalNameExtractor#reassignDestructringLvalue
    pub fn reassign_destructring_lvalue(
        string_key: NodeId,
        new_name: NodeId,
        mut new_nodes: Option<&mut IndexSet<AstChange>>,
        r#ref: Ref,
        compiler: &mut AbstractCompiler,
    ) {
        let pattern = string_key.get_parent(compiler).unwrap();
        let assignment_type = pattern.get_parent(compiler).unwrap();
        check_state!(
            assignment_type.is_assign(compiler) || assignment_type.is_destructuring_lhs(compiler),
            "%s",
            assignment_type.to_string(compiler)
        );
        let original_rvalue = pattern.get_next(compiler).unwrap(); // e.g. `original`
        check_state!(original_rvalue.is_qualified_name(compiler)); // don't handle rvalues with side effects

        // Create a new assignment using the provided qualified name, e.g. `const y = new.name;`
        let lvalue_to_reassign = if string_key
            .get_only_child(compiler)
            .is_default_value(compiler)
        {
            string_key
                .get_only_child(compiler)
                .get_first_child(compiler)
                .unwrap()
        } else {
            string_key.get_only_child(compiler)
        };
        if let Some(new_nodes) = new_nodes.as_deref_mut() {
            new_nodes.insert(AstChange::RefBasedAstChange(RefBasedAstChange::new(
                r#ref, new_name,
            )));
        }
        let rvalue = Self::make_new_rvalue_for_destructuring_key(
            compiler,
            string_key,
            new_name,
            new_nodes.as_deref_mut(),
            r#ref,
        );

        // Add that new assignment to the AST after the original destructuring pattern
        if string_key.get_previous(compiler).is_none() && string_key.get_next(compiler).is_some() {
            // Remove the original item completely if the given string key doesn't have any
            // preceding string keys, /and/ it has succeeding string keys.
            let detached = lvalue_to_reassign.detach(compiler);
            Self::replace_destructuring_assignment(compiler, pattern, detached, rvalue);
        } else {
            let detached = lvalue_to_reassign.detach(compiler);
            Self::add_after(compiler, pattern, detached, rvalue);
        }

        // Remove any lvalues in the pattern that are after the given stringKey, and put them in
        // a second destructuring pattern. This is necessary in case they depend on side effects
        // from assigning `lvalueToReassign`. e.g. create `const {z} = original;`
        if string_key.get_next(compiler).is_some() {
            let new_pattern =
                Self::create_new_object_pattern_from_successive_keys(compiler, string_key)
                    .srcref(compiler, pattern);

            // reuse the original rvalue if we don't need it for earlier keys, otherwise make a
            // copy.
            let new_rvalue = if string_key.get_previous(compiler).is_none() {
                original_rvalue.detach(compiler)
            } else {
                let new_rvalue = original_rvalue.clone_tree(compiler);
                if let Some(new_nodes) = new_nodes.as_mut() {
                    new_nodes.insert(AstChange::RefBasedAstChange(RefBasedAstChange::new(
                        r#ref, new_rvalue,
                    )));
                }
                new_rvalue
            };
            Self::add_after(compiler, lvalue_to_reassign, new_pattern, new_rvalue);
        }

        string_key.detach(compiler);
        compiler.report_change_to_enclosing_scope(lvalue_to_reassign);
    }

    /// Adds the new assign or name declaration after the original assign or name declaration
    // port: DestructuringGlobalNameExtractor#addAfter
    fn add_after(ast: &mut Ast, original_lvalue: NodeId, new_lvalue: NodeId, new_rvalue: NodeId) {
        let mut new_lvalue = new_lvalue;
        let parent = original_lvalue.get_parent(ast).unwrap();
        if parent.is_assign(ast) {
            // create `(<originalLvalue = ...>, <newLvalue = newRvalue>)`
            let new_assign = IR::assign(ast, new_lvalue, new_rvalue).srcref(ast, parent);
            let new_comma = ast.new_node_with_child(Token::COMMA, new_assign);
            parent.replace_with(ast, new_comma);
            new_comma.add_child_to_front(ast, parent);
            return;
        }
        // This must have been in a var/let/const.
        if new_lvalue.is_destructuring_pattern(ast) {
            new_lvalue = ast
                .new_node_with_children2(Token::DESTRUCTURING_LHS, new_lvalue, new_rvalue)
                .srcref(ast, parent);
        } else {
            new_lvalue.add_child_to_back(ast, new_rvalue);
        }
        let declaration = if parent.is_destructuring_lhs(ast) {
            original_lvalue.get_grandparent(ast).unwrap()
        } else {
            parent
        };
        check_state!(
            NodeUtil::is_name_declaration(ast, Some(declaration)),
            "%s",
            declaration.to_string(ast)
        );
        if NodeUtil::is_statement_parent(ast, declaration.get_parent(ast).unwrap()) {
            // `const {} = originalRvalue; const newLvalue = newRvalue;`
            // create an entirely new statement
            let token = declaration.get_token(ast);
            let new_declaration = ast.new_node(token).srcref(ast, declaration);
            new_declaration.add_child_to_back(ast, new_lvalue);
            new_declaration.insert_after(ast, declaration);
        } else {
            // `const {} = originalRvalue, newLvalue = newRvalue;`
            // The Normalize pass tries to ensure name declarations are always in statement
            // blocks, but currently has not implemented normalization for `for (let x = 0; ...`
            // so we can't add a new statement
            declaration.add_child_to_back(ast, new_lvalue);
        }
    }

    /// Replaces the given assignment or declaration with the new lvalue/rvalue
    // port: DestructuringGlobalNameExtractor#replaceDestructuringAssignment
    fn replace_destructuring_assignment(
        ast: &mut Ast,
        pattern: NodeId,
        new_lvalue: NodeId,
        new_rvalue: NodeId,
    ) {
        let parent = pattern.get_parent(ast).unwrap();
        if parent.is_assign(ast) {
            let new_assign = IR::assign(ast, new_lvalue, new_rvalue).srcref(ast, parent);
            parent.replace_with(ast, new_assign);
        } else if new_lvalue.is_name(ast) {
            check_state!(parent.is_destructuring_lhs(ast));
            parent.replace_with(ast, new_lvalue);
            new_lvalue.add_child_to_back(ast, new_rvalue);
        } else {
            pattern.get_next(ast).unwrap().detach(ast);
            pattern.detach(ast);
            parent.add_child_to_back(ast, new_lvalue);
            parent.add_child_to_back(ast, new_rvalue);
        }
    }

    /// Makes a default value expression from the rvalue, or otherwise just returns it
    ///
    /// e.g. for `const {x = defaultValue} = y;`, and the new rvalue `rvalue`, returns
    /// `void 0 === rvalue ? defaultValue : rvalue`
    // port: DestructuringGlobalNameExtractor#makeNewRvalueForDestructuringKey
    fn make_new_rvalue_for_destructuring_key(
        ast: &mut Ast,
        string_key: NodeId,
        rvalue: NodeId,
        new_nodes: Option<&mut IndexSet<AstChange>>,
        r#ref: Ref,
    ) -> NodeId {
        let mut rvalue = rvalue;
        if string_key.get_only_child(ast).is_default_value(ast) {
            let default_value = string_key
                .get_first_child(ast)
                .unwrap()
                .get_second_child(ast)
                .unwrap()
                .detach(ast);
            // Assume `rvalue` has no side effects since it's a qname, and we can create multiple
            // references to it. This ignores getters/setters.
            let rvalue_for_sheq = rvalue.clone_tree(ast);
            if let Some(new_nodes) = new_nodes {
                new_nodes.insert(AstChange::RefBasedAstChange(RefBasedAstChange::new(
                    r#ref,
                    rvalue_for_sheq,
                )));
            }
            // `void 0 === rvalue ? defaultValue : rvalue`
            let undefined = NodeUtil::new_undefined_node(ast, Some(rvalue));
            let sheq = IR::sheq(ast, undefined, rvalue_for_sheq);
            rvalue = IR::hook(ast, sheq, default_value, rvalue).srcref_tree(ast, default_value);
        }
        rvalue
    }

    /// Removes any keys after the given key, and adds them in order to a new object pattern
    // port: DestructuringGlobalNameExtractor#createNewObjectPatternFromSuccessiveKeys
    fn create_new_object_pattern_from_successive_keys(ast: &mut Ast, string_key: NodeId) -> NodeId {
        let new_pattern = string_key.get_parent(ast).unwrap().clone_node(ast); // copies the JSType
        let mut next = string_key.get_next(ast);
        while let Some(new_key) = next {
            next = new_key.get_next(ast);
            let detached = new_key.detach(ast);
            new_pattern.add_child_to_back(ast, detached);
        }
        new_pattern
    }
}
