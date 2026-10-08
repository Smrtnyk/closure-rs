/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   John Lenz
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/IR.java.

// Keep the Java control flow visible for side-by-side port review.
#![allow(clippy::needless_return, clippy::match_like_matches_macro)]
use crate::{
    check_state,
    java_lang::double_to_string,
    js_string::JsString,
    jscomp_base::JSCompDoubles,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    token::Token,
};
use num_bigint::{BigInt, Sign};
use std::sync::Arc;
pub struct IR;
impl IR {
    // port: IR#IR
    fn new() -> Self {
        Self
    }
    // port: IR#empty()
    pub fn empty(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::EMPTY);
    }
    // port: IR#export(Node declaration)
    pub fn export(ast: &mut Ast, declaration: NodeId) -> NodeId {
        return ast.new_node_with_child(Token::EXPORT, declaration);
    }
    // port: IR#importNode(Node name, Node importSpecs, Node moduleIdentifier)
    pub fn import_node(
        ast: &mut Ast,
        name: NodeId,
        import_specs: NodeId,
        module_identifier: NodeId,
    ) -> NodeId {
        check_state!(
            name.is_name(ast) || name.is_empty(ast),
            "%s",
            name.to_string(ast)
        );
        check_state!(
            import_specs.is_import_spec(ast)
                || import_specs.is_import_star(ast)
                || import_specs.is_empty(ast),
            "%s",
            import_specs.to_string(ast)
        );
        check_state!(
            module_identifier.is_string_lit(ast),
            "%s",
            module_identifier.to_string(ast)
        );
        return ast.new_node_with_children3(Token::IMPORT, name, import_specs, module_identifier);
    }
    // port: IR#importStar(String name)
    pub fn import_star(ast: &mut Ast, name: impl Into<JsString>) -> NodeId {
        let name = name.into();

        return ast.new_string_with_token(Token::IMPORT_STAR, name);
    }
    // port: IR#function(Node name, Node params, Node body)
    pub fn function(ast: &mut Ast, name: NodeId, params: NodeId, body: NodeId) -> NodeId {
        check_state!(name.is_name(ast));
        check_state!(params.is_param_list(ast));
        check_state!(body.is_block(ast) || body.is_empty(ast));
        return ast.new_node_with_children3(Token::FUNCTION, name, params, body);
    }
    // port: IR#arrowFunction(Node name, Node params, Node body)
    pub fn arrow_function(ast: &mut Ast, name: NodeId, params: NodeId, body: NodeId) -> NodeId {
        check_state!(name.is_name(ast));
        check_state!(params.is_param_list(ast));
        check_state!(body.is_block(ast) || Self::may_be_expression(ast, body));
        let func = ast.new_node_with_children3(Token::FUNCTION, name, params, body);
        func.set_is_arrow_function(ast, true);
        return func;
    }
    // port: IR#paramList(Node... params)
    pub fn param_list(ast: &mut Ast, params: &[NodeId]) -> NodeId {
        let param_list = ast.new_node(Token::PARAM_LIST);
        for param in params.iter().copied() {
            check_state!(param.is_name(ast) || param.is_rest(ast) || param.is_default_value(ast));
            param_list.add_child_to_back(ast, param);
        }
        return param_list;
    }
    // port: IR#root(Node... rootChildren)
    pub fn root(ast: &mut Ast, root_children: &[NodeId]) -> NodeId {
        let root = ast.new_node(Token::ROOT);
        for child in root_children.iter().copied() {
            check_state!(
                child.get_token(ast) == Token::ROOT || child.get_token(ast) == Token::SCRIPT
            );
            root.add_child_to_back(ast, child);
        }
        return root;
    }
    // port: IR#block()
    pub fn block(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::BLOCK);
    }
    // port: IR#block(Node stmt)
    pub fn block_with_child(ast: &mut Ast, stmt: NodeId) -> NodeId {
        check_state!(
            Self::may_be_statement(ast, stmt),
            "Block node cannot contain %s",
            stmt.get_token(ast)
        );
        return ast.new_node_with_child(Token::BLOCK, stmt);
    }
    // port: IR#block(Node... stmts)
    pub fn block_with_children(ast: &mut Ast, stmts: &[NodeId]) -> NodeId {
        let block = Self::block(ast);
        for stmt in stmts.iter().copied() {
            check_state!(Self::may_be_statement(ast, stmt));
            block.add_child_to_back(ast, stmt);
        }
        return block;
    }
    // port: IR#block(List<Node> stmts)
    pub fn block_with_list(ast: &mut Ast, stmts: &[NodeId]) -> NodeId {
        let param_list = Self::block(ast);
        for stmt in stmts.iter().copied() {
            check_state!(Self::may_be_statement(ast, stmt));
            param_list.add_child_to_back(ast, stmt);
        }
        return param_list;
    }
    // port: IR#blockUnchecked(Node stmt)
    pub fn block_unchecked(ast: &mut Ast, stmt: NodeId) -> NodeId {
        return ast.new_node_with_child(Token::BLOCK, stmt);
    }
    // port: IR#script()
    pub fn script(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::SCRIPT);
    }
    // port: IR#script(Node... stmts)
    pub fn script_with_children(ast: &mut Ast, stmts: &[NodeId]) -> NodeId {
        let block = Self::script(ast);
        for stmt in stmts.iter().copied() {
            check_state!(Self::may_be_statement_no_return(ast, stmt));
            block.add_child_to_back(ast, stmt);
        }
        return block;
    }
    // port: IR#script(List<Node> stmts)
    pub fn script_with_list(ast: &mut Ast, stmts: &[NodeId]) -> NodeId {
        let param_list = Self::script(ast);
        for stmt in stmts.iter().copied() {
            check_state!(Self::may_be_statement_no_return(ast, stmt));
            param_list.add_child_to_back(ast, stmt);
        }
        return param_list;
    }
    // port: IR#var(Node lhs, Node value)
    pub fn var_with_value(ast: &mut Ast, lhs: NodeId, value: NodeId) -> NodeId {
        return Self::declaration_with_value(ast, lhs, value, Token::VAR);
    }
    // port: IR#var(Node lhs)
    pub fn var(ast: &mut Ast, lhs: NodeId) -> NodeId {
        return Self::declaration(ast, lhs, Token::VAR);
    }
    // port: IR#let(Node lhs, Node value)
    pub fn let_with_value(ast: &mut Ast, lhs: NodeId, value: NodeId) -> NodeId {
        return Self::declaration_with_value(ast, lhs, value, Token::LET);
    }
    // port: IR#let(Node lhs)
    pub fn r#let(ast: &mut Ast, lhs: NodeId) -> NodeId {
        return Self::declaration(ast, lhs, Token::LET);
    }
    // port: IR#constNode(Node lhs, Node value)
    pub fn const_node(ast: &mut Ast, lhs: NodeId, value: NodeId) -> NodeId {
        return Self::declaration_with_value(ast, lhs, value, Token::CONST);
    }
    // port: IR#declaration(Node lhs, Token type)
    pub fn declaration(ast: &mut Ast, mut lhs: NodeId, r#type: Token) -> NodeId {
        check_state!(
            lhs.is_name(ast) || lhs.is_destructuring_pattern(ast) || lhs.is_destructuring_lhs(ast),
            "%s",
            lhs.to_string(ast)
        );
        if lhs.is_destructuring_pattern(ast) {
            lhs = ast.new_node_with_child(Token::DESTRUCTURING_LHS, lhs);
        }
        return ast.new_node_with_child(r#type, lhs);
    }
    // port: IR#declaration(Node lhs, Node value, Token type)
    pub fn declaration_with_value(
        ast: &mut Ast,
        mut lhs: NodeId,
        value: NodeId,
        r#type: Token,
    ) -> NodeId {
        if lhs.is_name(ast) {
            check_state!(!lhs.has_children(ast));
        } else {
            check_state!(lhs.is_array_pattern(ast) || lhs.is_object_pattern(ast));
            lhs = ast.new_node_with_child(Token::DESTRUCTURING_LHS, lhs);
        }
        check_state!(
            Self::may_be_expression(ast, value),
            "%s can't be an expression",
            value.to_string(ast)
        );

        lhs.add_child_to_back(ast, value);
        return ast.new_node_with_child(r#type, lhs);
    }
    // port: IR#returnNode()
    pub fn return_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::RETURN);
    }
    // port: IR#returnNode(Node expr)
    pub fn return_node_with_expression(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(Token::RETURN, expr);
    }
    // port: IR#yieldNode(Node expr)
    pub fn yield_node(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(Token::YIELD, expr);
    }
    // port: IR#await(Node expr)
    pub fn r#await(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(Token::AWAIT, expr);
    }
    // port: IR#throwNode(Node expr)
    pub fn throw_node(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(Token::THROW, expr);
    }
    // port: IR#exprResult(Node expr)
    pub fn expr_result(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(
            Self::may_be_expression(ast, expr),
            "%s",
            expr.to_string(ast)
        );
        return ast.new_node_with_child(Token::EXPR_RESULT, expr);
    }
    // port: IR#ifNode(Node cond, Node then)
    pub fn if_node(ast: &mut Ast, cond: NodeId, then: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, cond));
        check_state!(then.is_block(ast));
        return ast.new_node_with_children2(Token::IF, cond, then);
    }
    // port: IR#ifNode(Node cond, Node then, Node elseNode)
    pub fn if_node_with_else(
        ast: &mut Ast,
        cond: NodeId,
        then: NodeId,
        else_node: NodeId,
    ) -> NodeId {
        check_state!(Self::may_be_expression(ast, cond));
        check_state!(then.is_block(ast));
        check_state!(else_node.is_block(ast));
        return ast.new_node_with_children3(Token::IF, cond, then, else_node);
    }
    // port: IR#doNode(Node body, Node cond)
    pub fn do_node(ast: &mut Ast, body: NodeId, cond: NodeId) -> NodeId {
        check_state!(body.is_block(ast));
        check_state!(Self::may_be_expression(ast, cond));
        return ast.new_node_with_children2(Token::DO, body, cond);
    }
    // port: IR#whileNode(Node cond, Node body)
    pub fn while_node(ast: &mut Ast, cond: NodeId, body: NodeId) -> NodeId {
        check_state!(body.is_block(ast));
        check_state!(Self::may_be_expression(ast, cond));
        return ast.new_node_with_children2(Token::WHILE, cond, body);
    }
    // port: IR#forIn(Node target, Node cond, Node body)
    pub fn for_in(ast: &mut Ast, target: NodeId, cond: NodeId, body: NodeId) -> NodeId {
        check_state!(target.is_var(ast) || Self::may_be_expression(ast, target));
        check_state!(Self::may_be_expression(ast, cond));
        check_state!(body.is_block(ast));
        return ast.new_node_with_children3(Token::FOR_IN, target, cond, body);
    }
    // port: IR#forNode(Node init, Node cond, Node incr, Node body)
    pub fn for_node(
        ast: &mut Ast,
        init: NodeId,
        cond: NodeId,
        incr: NodeId,
        body: NodeId,
    ) -> NodeId {
        check_state!(
            init.is_var(ast)
                || init.is_let(ast)
                || init.is_const(ast)
                || Self::may_be_expression_or_empty(ast, init)
        );
        check_state!(Self::may_be_expression_or_empty(ast, cond));
        check_state!(Self::may_be_expression_or_empty(ast, incr));
        check_state!(body.is_block(ast));
        let r = ast.new_node_with_children3(Token::FOR, init, cond, incr);
        r.add_child_to_back(ast, body);
        return r;
    }
    // port: IR#switchNode(Node cond, Node... cases)
    pub fn switch_node(ast: &mut Ast, cond: NodeId, cases: &[NodeId]) -> NodeId {
        check_state!(Self::may_be_expression(ast, cond));
        let switch_node = ast.new_node_with_child(Token::SWITCH, cond);
        let switch_body = ast.new_node(Token::SWITCH_BODY);
        switch_node.add_child_to_back(ast, switch_body);
        for case_node in cases.iter().copied() {
            check_state!(case_node.is_case(ast) || case_node.is_default_case(ast));
            switch_body.add_child_to_back(ast, case_node);
        }
        return switch_node;
    }
    // port: IR#caseNode(Node expr, Node body)
    pub fn case_node(ast: &mut Ast, expr: NodeId, body: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        check_state!(body.is_block(ast));
        body.set_is_added_block(ast, true);
        return ast.new_node_with_children2(Token::CASE, expr, body);
    }
    // port: IR#defaultCase(Node body)
    pub fn default_case(ast: &mut Ast, body: NodeId) -> NodeId {
        check_state!(body.is_block(ast));
        body.set_is_added_block(ast, true);
        return ast.new_node_with_child(Token::DEFAULT_CASE, body);
    }
    // port: IR#label(Node name, Node stmt)
    pub fn label(ast: &mut Ast, name: NodeId, stmt: NodeId) -> NodeId {
        check_state!(name.is_label_name(ast));
        check_state!(Self::may_be_statement(ast, stmt));
        return ast.new_node_with_children2(Token::LABEL, name, stmt);
    }
    // port: IR#labelName(String name)
    pub fn label_name(ast: &mut Ast, name: impl Into<JsString>) -> NodeId {
        let name = name.into();

        check_state!(!name.is_empty());
        return ast.new_string_with_token(Token::LABEL_NAME, name);
    }
    // port: IR#tryFinally(Node tryBody, Node finallyBody)
    pub fn try_finally(ast: &mut Ast, try_body: NodeId, finally_body: NodeId) -> NodeId {
        check_state!(try_body.is_block(ast));
        check_state!(finally_body.is_block(ast));
        let catch_body = Self::block(ast).srcref_if_missing(ast, try_body);
        return ast.new_node_with_children3(Token::TRY, try_body, catch_body, finally_body);
    }
    // port: IR#tryCatch(Node tryBody, Node catchNode)
    pub fn try_catch(ast: &mut Ast, try_body: NodeId, catch_node: NodeId) -> NodeId {
        check_state!(try_body.is_block(ast));
        check_state!(catch_node.is_catch(ast));
        let catch_body = Self::block_unchecked(ast, catch_node).srcref_if_missing(ast, catch_node);
        return ast.new_node_with_children2(Token::TRY, try_body, catch_body);
    }
    // port: IR#tryCatchFinally(Node tryBody, Node catchNode, Node finallyBody)
    pub fn try_catch_finally(
        ast: &mut Ast,
        try_body: NodeId,
        catch_node: NodeId,
        finally_body: NodeId,
    ) -> NodeId {
        check_state!(finally_body.is_block(ast));
        let try_node = Self::try_catch(ast, try_body, catch_node);
        try_node.add_child_to_back(ast, finally_body);
        return try_node;
    }
    // port: IR#catchNode(Node expr, Node body)
    pub fn catch_node(ast: &mut Ast, expr: NodeId, body: NodeId) -> NodeId {
        check_state!(expr.is_name(ast));
        check_state!(body.is_block(ast));
        return ast.new_node_with_children2(Token::CATCH, expr, body);
    }
    // port: IR#breakNode()
    pub fn break_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::BREAK);
    }
    // port: IR#breakNode(Node name)
    pub fn break_node_with_label(ast: &mut Ast, name: NodeId) -> NodeId {
        check_state!(name.is_label_name(ast));
        return ast.new_node_with_child(Token::BREAK, name);
    }
    // port: IR#continueNode()
    pub fn continue_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::CONTINUE);
    }
    // port: IR#continueNode(Node name)
    pub fn continue_node_with_label(ast: &mut Ast, name: NodeId) -> NodeId {
        check_state!(name.is_label_name(ast));
        return ast.new_node_with_child(Token::CONTINUE, name);
    }
    // port: IR#call(Node target, Node... args)
    pub fn call(ast: &mut Ast, target: NodeId, args: &[NodeId]) -> NodeId {
        let call = ast.new_node_with_child(Token::CALL, target);
        for arg in args.iter().copied() {
            check_state!(
                Self::may_be_expression(ast, arg) || arg.is_spread(ast),
                "%s",
                arg.to_string(ast)
            );
            call.add_child_to_back(ast, arg);
        }
        return call;
    }
    // port: IR#startOptChainCall(Node target, Node... args)
    pub fn start_opt_chain_call(ast: &mut Ast, target: NodeId, args: &[NodeId]) -> NodeId {
        let call = ast.new_node_with_child(Token::OPTCHAIN_CALL, target);
        for arg in args.iter().copied() {
            check_state!(
                Self::may_be_expression(ast, arg) || arg.is_spread(ast),
                "%s",
                arg.to_string(ast)
            );
            call.add_child_to_back(ast, arg);
        }
        call.set_is_optional_chain_start(ast, true);
        return call;
    }
    // port: IR#continueOptChainCall(Node target, Node... args)
    pub fn continue_opt_chain_call(ast: &mut Ast, target: NodeId, args: &[NodeId]) -> NodeId {
        let call = ast.new_node_with_child(Token::OPTCHAIN_CALL, target);
        for arg in args.iter().copied() {
            check_state!(
                Self::may_be_expression(ast, arg) || arg.is_spread(ast),
                "%s",
                arg.to_string(ast)
            );
            call.add_child_to_back(ast, arg);
        }
        call.set_is_optional_chain_start(ast, false);
        return call;
    }
    // port: IR#newNode(Node target, Node... args)
    pub fn new_node(ast: &mut Ast, target: NodeId, args: &[NodeId]) -> NodeId {
        let newcall = ast.new_node_with_child(Token::NEW, target);
        for arg in args.iter().copied() {
            check_state!(
                Self::may_be_expression(ast, arg) || arg.is_spread(ast),
                "%s",
                arg.to_string(ast)
            );
            newcall.add_child_to_back(ast, arg);
        }
        return newcall;
    }
    // port: IR#name(String name)
    pub fn name(ast: &mut Ast, name: impl Into<JsString>) -> NodeId {
        let name = name.into();

        check_state!(
            name.index_of_char(b'.' as u16) == -1,
            "Invalid name '%s'. Did you mean to use NodeUtil.newQName?",
            name
        );
        return ast.new_string_with_token(Token::NAME, name);
    }
    // port: IR#startOptChainGetprop(Node target, String prop)
    pub fn start_opt_chain_getprop(
        ast: &mut Ast,
        target: NodeId,
        prop: impl Into<JsString>,
    ) -> NodeId {
        let prop = prop.into();

        check_state!(
            Self::may_be_expression(ast, target),
            "%s",
            target.to_string(ast)
        );
        let opt_chain_get_prop = ast.new_string_with_token(Token::OPTCHAIN_GETPROP, prop);
        opt_chain_get_prop.add_child_to_back(ast, target);
        opt_chain_get_prop.set_is_optional_chain_start(ast, true);
        return opt_chain_get_prop;
    }
    // port: IR#continueOptChainGetprop(Node target, String prop)
    pub fn continue_opt_chain_getprop(
        ast: &mut Ast,
        target: NodeId,
        prop: impl Into<JsString>,
    ) -> NodeId {
        let prop = prop.into();

        check_state!(
            Self::may_be_expression(ast, target),
            "%s",
            target.to_string(ast)
        );
        let opt_chain_get_prop = ast.new_string_with_token(Token::OPTCHAIN_GETPROP, prop);
        opt_chain_get_prop.add_child_to_back(ast, target);
        opt_chain_get_prop.set_is_optional_chain_start(ast, false);
        return opt_chain_get_prop;
    }
    // port: IR#getprop(Node target, String prop)
    pub fn getprop(ast: &mut Ast, target: NodeId, prop: impl Into<JsString>) -> NodeId {
        let prop = prop.into();

        check_state!(Self::may_be_expression(ast, target));
        let getprop = ast.new_string_with_token(Token::GETPROP, prop);
        getprop.add_child_to_back(ast, target);
        return getprop;
    }
    // port: IR#getprop(Node target, String prop, String... moreProps)
    pub fn getprop_with_more_props(
        ast: &mut Ast,
        target: NodeId,
        prop: impl Into<JsString>,
        more_props: &[JsString],
    ) -> NodeId {
        let prop = prop.into();

        check_state!(Self::may_be_expression(ast, target));
        let mut result = Self::getprop(ast, target, prop);
        for more_prop in more_props {
            result = Self::getprop(ast, result, more_prop);
        }
        return result;
    }
    // port: IR#startOptChainGetelem(Node target, Node elem)
    pub fn start_opt_chain_getelem(ast: &mut Ast, target: NodeId, elem: NodeId) -> NodeId {
        check_state!(
            Self::may_be_expression(ast, target),
            "%s",
            target.to_string(ast)
        );
        check_state!(
            Self::may_be_expression(ast, elem),
            "%s",
            elem.to_string(ast)
        );
        let opt_chain_get_elem = ast.new_node_with_children2(Token::OPTCHAIN_GETELEM, target, elem);
        opt_chain_get_elem.set_is_optional_chain_start(ast, true);
        return opt_chain_get_elem;
    }
    // port: IR#continueOptChainGetelem(Node target, Node elem)
    pub fn continue_opt_chain_getelem(ast: &mut Ast, target: NodeId, elem: NodeId) -> NodeId {
        check_state!(
            Self::may_be_expression(ast, target),
            "%s",
            target.to_string(ast)
        );
        check_state!(
            Self::may_be_expression(ast, elem),
            "%s",
            elem.to_string(ast)
        );
        let opt_chain_get_elem = ast.new_node_with_children2(Token::OPTCHAIN_GETELEM, target, elem);
        opt_chain_get_elem.set_is_optional_chain_start(ast, false);
        return opt_chain_get_elem;
    }
    // port: IR#getelem(Node target, Node elem)
    pub fn getelem(ast: &mut Ast, target: NodeId, elem: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, target));
        check_state!(Self::may_be_expression(ast, elem));
        return ast.new_node_with_children2(Token::GETELEM, target, elem);
    }
    // port: IR#delprop(Node target)
    pub fn delprop(ast: &mut Ast, target: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, target));
        return ast.new_node_with_child(Token::DELPROP, target);
    }
    // port: IR#assign(Node target, Node expr)
    pub fn assign(ast: &mut Ast, target: NodeId, expr: NodeId) -> NodeId {
        check_state!(
            target.is_valid_assignment_target(ast),
            "%s",
            target.to_string(ast)
        );
        check_state!(
            Self::may_be_expression(ast, expr),
            "%s",
            expr.to_string(ast)
        );
        return ast.new_node_with_children2(Token::ASSIGN, target, expr);
    }
    // port: IR#hook(Node cond, Node trueval, Node falseval)
    pub fn hook(ast: &mut Ast, cond: NodeId, trueval: NodeId, falseval: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, cond));
        check_state!(Self::may_be_expression(ast, trueval));
        check_state!(Self::may_be_expression(ast, falseval));
        return ast.new_node_with_children3(Token::HOOK, cond, trueval, falseval);
    }
    // port: IR#in(Node expr1, Node expr2)
    pub fn r#in(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::IN, expr1, expr2);
    }
    // port: IR#comma(Node expr1, Node expr2)
    pub fn comma(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::COMMA, expr1, expr2);
    }
    // port: IR#and(Node expr1, Node expr2)
    pub fn and(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::AND, expr1, expr2);
    }
    // port: IR#or(Node expr1, Node expr2)
    pub fn or(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::OR, expr1, expr2);
    }
    // port: IR#coalesce(Node expr1, Node expr2)
    pub fn coalesce(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::COALESCE, expr1, expr2);
    }
    // port: IR#not(Node expr1)
    pub fn not(ast: &mut Ast, expr1: NodeId) -> NodeId {
        return Self::unary_op(ast, Token::NOT, expr1);
    }
    // port: IR#lt(Node expr1, Node expr2)
    pub fn lt(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::LT, expr1, expr2);
    }
    // port: IR#ge(Node expr1, Node expr2)
    pub fn ge(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::GE, expr1, expr2);
    }
    // port: IR#eq(Node expr1, Node expr2)
    pub fn eq(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::EQ, expr1, expr2);
    }
    // port: IR#ne(Node expr1, Node expr2)
    pub fn ne(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::NE, expr1, expr2);
    }
    // port: IR#sheq(Node expr1, Node expr2)
    pub fn sheq(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::SHEQ, expr1, expr2);
    }
    // port: IR#shne(Node expr1, Node expr2)
    pub fn shne(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::SHNE, expr1, expr2);
    }
    // port: IR#voidNode(Node expr1)
    pub fn void_node(ast: &mut Ast, expr1: NodeId) -> NodeId {
        return Self::unary_op(ast, Token::VOID, expr1);
    }
    // port: IR#neg(Node expr1)
    pub fn neg(ast: &mut Ast, expr1: NodeId) -> NodeId {
        return Self::unary_op(ast, Token::NEG, expr1);
    }
    // port: IR#pos(Node expr1)
    pub fn pos(ast: &mut Ast, expr1: NodeId) -> NodeId {
        return Self::unary_op(ast, Token::POS, expr1);
    }
    // port: IR#cast(Node expr1, JSDocInfo jsdoc)
    pub fn cast(ast: &mut Ast, expr1: NodeId, jsdoc: Option<Arc<JSDocInfo>>) -> NodeId {
        let op = Self::unary_op(ast, Token::CAST, expr1);
        op.set_jsdoc_info(ast, jsdoc);
        return op;
    }
    // port: IR#inc(Node exp, boolean isPost)
    pub fn inc(ast: &mut Ast, exp: NodeId, is_post: bool) -> NodeId {
        let op = Self::unary_op(ast, Token::INC, exp);
        op.put_boolean_prop(ast, NodeId::INCRDECR_PROP, is_post);
        return op;
    }
    // port: IR#dec(Node exp, boolean isPost)
    pub fn dec(ast: &mut Ast, exp: NodeId, is_post: bool) -> NodeId {
        let op = Self::unary_op(ast, Token::DEC, exp);
        op.put_boolean_prop(ast, NodeId::INCRDECR_PROP, is_post);
        return op;
    }
    // port: IR#add(Node expr1, Node expr2)
    pub fn add(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::ADD, expr1, expr2);
    }
    // port: IR#sub(Node expr1, Node expr2)
    pub fn sub(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::SUB, expr1, expr2);
    }
    // port: IR#assignOr(Node expr1, Node expr2)
    pub fn assign_or(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::ASSIGN_OR, expr1, expr2);
    }
    // port: IR#assignAnd(Node expr1, Node expr2)
    pub fn assign_and(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::ASSIGN_AND, expr1, expr2);
    }
    // port: IR#assignCoalesce(Node expr1, Node expr2)
    pub fn assign_coalesce(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::ASSIGN_COALESCE, expr1, expr2);
    }
    // port: IR#bitwiseAnd(Node expr1, Node expr2)
    pub fn bitwise_and(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::BITAND, expr1, expr2);
    }
    // port: IR#rightShift(Node expr1, Node expr2)
    pub fn right_shift(ast: &mut Ast, expr1: NodeId, expr2: NodeId) -> NodeId {
        return Self::binary_op(ast, Token::RSH, expr1, expr2);
    }
    // port: IR#objectlit(Node... propdefs)
    pub fn objectlit(ast: &mut Ast, propdefs: &[NodeId]) -> NodeId {
        let objectlit = ast.new_node(Token::OBJECTLIT);
        for propdef in propdefs.iter().copied() {
            match propdef.get_token(ast) {
                Token::STRING_KEY
                | Token::MEMBER_FUNCTION_DEF
                | Token::GETTER_DEF
                | Token::SETTER_DEF
                | Token::OBJECT_SPREAD
                | Token::COMPUTED_PROP => {}
                _ => panic!("Unexpected OBJECTLIT child: {}", propdef.to_string(ast)),
            }

            objectlit.add_child_to_back(ast, propdef);
        }
        return objectlit;
    }
    // port: IR#objectPattern(Node... keys)
    pub fn object_pattern(ast: &mut Ast, keys: &[NodeId]) -> NodeId {
        let object_pattern = ast.new_node(Token::OBJECT_PATTERN);
        for key in keys.iter().copied() {
            check_state!(key.is_string_key(ast) || key.is_computed_prop(ast) || key.is_rest(ast));
            object_pattern.add_child_to_back(ast, key);
        }
        return object_pattern;
    }
    // port: IR#arrayPattern(Node... keys)
    pub fn array_pattern(ast: &mut Ast, keys: &[NodeId]) -> NodeId {
        let array_pattern = ast.new_node(Token::ARRAY_PATTERN);
        for key in keys.iter().copied() {
            check_state!(key.is_rest(ast) || key.is_valid_assignment_target(ast));
            array_pattern.add_child_to_back(ast, key);
        }
        return array_pattern;
    }
    // port: IR#computedProp(Node key, Node value)
    pub fn computed_prop(ast: &mut Ast, key: NodeId, value: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, key), "%s", key.to_string(ast));
        check_state!(
            Self::may_be_expression(ast, value),
            "%s",
            value.to_string(ast)
        );
        return ast.new_node_with_children2(Token::COMPUTED_PROP, key, value);
    }
    // port: IR#propdef(Node string, Node value)
    pub fn propdef(ast: &mut Ast, string: NodeId, value: NodeId) -> NodeId {
        check_state!(string.is_string_key(ast));
        check_state!(!string.has_children(ast));
        check_state!(Self::may_be_expression(ast, value));
        string.add_child_to_front(ast, value);
        return string;
    }
    // port: IR#arraylit(Node... exprs)
    pub fn arraylit(ast: &mut Ast, exprs: &[NodeId]) -> NodeId {
        return Self::arraylit_from_iterable(ast, exprs.iter().copied());
    }
    // port: IR#arraylit(Iterable<Node> exprs)
    pub fn arraylit_from_iterable(
        ast: &mut Ast,
        exprs: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        let arraylit = ast.new_node(Token::ARRAYLIT);
        for expr in exprs {
            check_state!(
                Self::may_be_expression_or_empty(ast, expr) || expr.is_spread(ast),
                "%s",
                expr.to_string(ast)
            );
            arraylit.add_child_to_back(ast, expr);
        }
        return arraylit;
    }
    // port: IR#regexp(Node expr)
    pub fn regexp(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(expr.is_string_lit(ast));
        return ast.new_node_with_child(Token::REGEXP, expr);
    }
    // port: IR#regexp(Node expr, Node flags)
    pub fn regexp_with_flags(ast: &mut Ast, expr: NodeId, flags: NodeId) -> NodeId {
        check_state!(expr.is_string_lit(ast));
        check_state!(flags.is_string_lit(ast));
        return ast.new_node_with_children2(Token::REGEXP, expr, flags);
    }
    // port: IR#string(String s)
    pub fn string(ast: &mut Ast, s: impl Into<JsString>) -> NodeId {
        let s = s.into();

        return ast.new_string(s);
    }
    // port: IR#stringKey(String s)
    pub fn string_key(ast: &mut Ast, s: impl Into<JsString>) -> NodeId {
        let s = s.into();

        return ast.new_string_with_token(Token::STRING_KEY, s);
    }
    // port: IR#stringKey(String s, Node value)
    pub fn string_key_with_value(ast: &mut Ast, s: impl Into<JsString>, value: NodeId) -> NodeId {
        let s = s.into();

        check_state!(
            Self::may_be_expression(ast, value)
                || value.is_default_value(ast)
                || value.is_object_pattern(ast)
        );
        let string_key = Self::string_key(ast, s);
        string_key.add_child_to_front(ast, value);
        return string_key;
    }
    // port: IR#quotedStringKey(String s, Node value)
    pub fn quoted_string_key(ast: &mut Ast, s: impl Into<JsString>, value: NodeId) -> NodeId {
        let s = s.into();

        let k = Self::string_key_with_value(ast, s, value);
        k.put_boolean_prop(ast, NodeId::QUOTED_PROP, true);
        return k;
    }
    // port: IR#templateLiteral()
    pub fn template_literal(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::TEMPLATELIT);
    }
    // port: IR#templateLiteralString(@Nullable String cooked, String raw)
    pub fn template_literal_string(
        ast: &mut Ast,
        cooked: Option<JsString>,
        raw: impl Into<JsString>,
    ) -> NodeId {
        let raw = raw.into();

        return ast.new_template_lit_string(cooked, raw);
    }
    // port: IR#templateLiteralSubstitution(Node child)
    pub fn template_literal_substitution(ast: &mut Ast, child: NodeId) -> NodeId {
        let sub = ast.new_node(Token::TEMPLATELIT_SUB);
        sub.add_child_to_back(ast, child);
        return sub;
    }
    // port: IR#iterRest(Node target)
    pub fn iter_rest(ast: &mut Ast, target: NodeId) -> NodeId {
        check_state!(
            target.is_valid_assignment_target(ast),
            "%s",
            target.to_string(ast)
        );
        return ast.new_node_with_child(Token::ITER_REST, target);
    }
    // port: IR#objectRest(Node target)
    pub fn object_rest(ast: &mut Ast, target: NodeId) -> NodeId {
        check_state!(
            target.is_valid_assignment_target(ast),
            "%s",
            target.to_string(ast)
        );
        return ast.new_node_with_child(Token::OBJECT_REST, target);
    }
    // port: IR#iterSpread(Node expr)
    pub fn iter_spread(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(Token::ITER_SPREAD, expr);
    }
    // port: IR#objectSpread(Node expr)
    pub fn object_spread(ast: &mut Ast, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(Token::OBJECT_SPREAD, expr);
    }
    // port: IR#superNode()
    pub fn super_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::SUPER);
    }
    // port: IR#getterDef(String name, Node value)
    pub fn getter_def(ast: &mut Ast, name: impl Into<JsString>, value: NodeId) -> NodeId {
        let name = name.into();

        check_state!(value.is_function(ast));
        let member = ast.new_string_with_token(Token::GETTER_DEF, name);
        member.add_child_to_front(ast, value);
        return member;
    }
    // port: IR#setterDef(String name, Node value)
    pub fn setter_def(ast: &mut Ast, name: impl Into<JsString>, value: NodeId) -> NodeId {
        let name = name.into();

        check_state!(value.is_function(ast));
        let member = ast.new_string_with_token(Token::SETTER_DEF, name);
        member.add_child_to_front(ast, value);
        return member;
    }
    // port: IR#memberFieldDef(String name, Node value)
    pub fn member_field_def(ast: &mut Ast, name: impl Into<JsString>, value: NodeId) -> NodeId {
        let name = name.into();

        check_state!(Self::may_be_expression(ast, value));
        let member = ast.new_string_with_token(Token::MEMBER_FIELD_DEF, name);
        member.add_child_to_front(ast, value);
        return member;
    }
    // port: IR#memberFunctionDef(String name, Node function)
    pub fn member_function_def(
        ast: &mut Ast,
        name: impl Into<JsString>,
        function: NodeId,
    ) -> NodeId {
        let name = name.into();

        check_state!(function.is_function(ast));
        let member = ast.new_string_with_token(Token::MEMBER_FUNCTION_DEF, name);
        member.add_child_to_back(ast, function);
        return member;
    }
    // port: IR#number(double d)
    pub fn number(ast: &mut Ast, d: f64) -> NodeId {
        check_state!(!d.is_nan(), "%s", double_to_string(d));
        check_state!(JSCompDoubles::is_positive(d), "%s", double_to_string(d));
        return ast.new_number(d);
    }
    // port: IR#bigint(BigInteger b)
    pub fn bigint(ast: &mut Ast, b: impl Into<Arc<BigInt>>) -> NodeId {
        let b = b.into();

        check_state!(b.sign() != Sign::Minus, "%s", b);
        return ast.new_big_int(b);
    }
    // port: IR#thisNode()
    pub fn this_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::THIS);
    }
    // port: IR#trueNode()
    pub fn true_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::TRUE);
    }
    // port: IR#falseNode()
    pub fn false_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::FALSE);
    }
    // port: IR#nullNode()
    pub fn null_node(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::NULL);
    }
    // port: IR#typeof(Node expr)
    pub fn r#typeof(ast: &mut Ast, expr: NodeId) -> NodeId {
        return Self::unary_op(ast, Token::TYPEOF, expr);
    }
    // port: IR#importMeta()
    pub fn import_meta(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::IMPORT_META);
    }
    // port: IR#newTarget()
    pub fn new_target(ast: &mut Ast) -> NodeId {
        return ast.new_node(Token::NEW_TARGET);
    }
    // port: IR#binaryOp(Token token, Node expr1, Node expr2)
    pub fn binary_op(ast: &mut Ast, token: Token, expr1: NodeId, expr2: NodeId) -> NodeId {
        check_state!(
            Self::may_be_expression(ast, expr1),
            "%s",
            expr1.to_string(ast)
        );
        check_state!(
            Self::may_be_expression(ast, expr2),
            "%s",
            expr2.to_string(ast)
        );
        return ast.new_node_with_children2(token, expr1, expr2);
    }
    // port: IR#unaryOp(Token token, Node expr)
    pub fn unary_op(ast: &mut Ast, token: Token, expr: NodeId) -> NodeId {
        check_state!(Self::may_be_expression(ast, expr));
        return ast.new_node_with_child(token, expr);
    }
    // port: IR#mayBeExpressionOrEmpty(Node n)
    pub fn may_be_expression_or_empty(ast: &Ast, n: NodeId) -> bool {
        return n.is_empty(ast) || Self::may_be_expression(ast, n);
    }
    // port: IR#mayBeStatementNoReturn(Node n)
    pub fn may_be_statement_no_return(ast: &Ast, n: NodeId) -> bool {
        return match n.get_token(ast) {
            Token::EMPTY | Token::FUNCTION => true,
            Token::BLOCK
            | Token::BREAK
            | Token::CLASS
            | Token::CONST
            | Token::CONTINUE
            | Token::DEBUGGER
            | Token::DO
            | Token::ENUM
            | Token::EXPR_RESULT
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::IF
            | Token::INTERFACE
            | Token::LABEL
            | Token::LET
            | Token::SWITCH
            | Token::THROW
            | Token::TRY
            | Token::VAR
            | Token::WHILE
            | Token::WITH => true,
            _ => false,
        };
    }
    // port: IR#mayBeStatement(Node n)
    pub fn may_be_statement(ast: &Ast, n: NodeId) -> bool {
        if !Self::may_be_statement_no_return(ast, n) {
            return n.is_return(ast);
        }
        return true;
    }
    // port: IR#mayBeExpression(Node n)
    pub fn may_be_expression(ast: &Ast, n: NodeId) -> bool {
        return match n.get_token(ast) {
            Token::FUNCTION | Token::CLASS => true,
            Token::ADD
            | Token::AND
            | Token::ARRAYLIT
            | Token::ASSIGN
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_EXPONENT
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_OR
            | Token::ASSIGN_AND
            | Token::ASSIGN_COALESCE
            | Token::AWAIT
            | Token::BIGINT
            | Token::BITAND
            | Token::BITOR
            | Token::BITNOT
            | Token::BITXOR
            | Token::CALL
            | Token::CAST
            | Token::COALESCE
            | Token::COMMA
            | Token::DEC
            | Token::DELPROP
            | Token::DIV
            | Token::DYNAMIC_IMPORT
            | Token::EQ
            | Token::EXPONENT
            | Token::FALSE
            | Token::GE
            | Token::GETPROP
            | Token::GETELEM
            | Token::GT
            | Token::HOOK
            | Token::IMPORT_META
            | Token::IN
            | Token::INC
            | Token::INSTANCEOF
            | Token::LE
            | Token::LSH
            | Token::LT
            | Token::MOD
            | Token::MUL
            | Token::NAME
            | Token::NE
            | Token::NEG
            | Token::NEW
            | Token::NEW_TARGET
            | Token::NOT
            | Token::NUMBER
            | Token::NULL
            | Token::OBJECTLIT
            | Token::OPTCHAIN_CALL
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_GETPROP
            | Token::OR
            | Token::POS
            | Token::REGEXP
            | Token::RSH
            | Token::SHEQ
            | Token::SHNE
            | Token::STRINGLIT
            | Token::SUB
            | Token::SUPER
            | Token::TEMPLATELIT
            | Token::TAGGED_TEMPLATELIT
            | Token::THIS
            | Token::TYPEOF
            | Token::TRUE
            | Token::URSH
            | Token::VOID
            | Token::YIELD => true,
            _ => false,
        };
    }
}
impl Default for IR {
    fn default() -> Self {
        Self::new()
    }
}
