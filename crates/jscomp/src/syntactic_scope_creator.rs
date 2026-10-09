/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SyntacticScopeCreator.java.

use crate::{
    abstract_compiler::AbstractCompiler, compiler_input::CompilerInput, node_util::NodeUtil,
    scope::ScopeId, scope_creator::ScopeCreator,
};
use closure_rhino::{
    check_not_null, check_state, input_id::InputId, js_string::JsString, node::NodeId, token::Token,
};
use indexmap::IndexSet;
use std::sync::Arc;

pub struct SyntacticScopeCreator<'a> {
    redeclaration_handler: Box<dyn RedeclarationHandler + 'a>,
    treat_provides_as_redeclarations: bool,
}

const ARGUMENTS: &str = "arguments";

pub const DEFAULT_REDECLARATION_HANDLER: DefaultRedeclarationHandler = DefaultRedeclarationHandler;

impl Default for SyntacticScopeCreator<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> SyntacticScopeCreator<'a> {
    // port: SyntacticScopeCreator#SyntacticScopeCreator(AbstractCompiler)
    pub fn new() -> Self {
        Self::new_with_redeclaration_handler(Box::new(DEFAULT_REDECLARATION_HANDLER))
    }

    // port: SyntacticScopeCreator#SyntacticScopeCreator(AbstractCompiler, RedeclarationHandler)
    pub fn new_with_redeclaration_handler(
        redeclaration_handler: Box<dyn RedeclarationHandler + 'a>,
    ) -> Self {
        Self::new_with_options(redeclaration_handler, false)
    }

    // port: SyntacticScopeCreator#SyntacticScopeCreator(AbstractCompiler, RedeclarationHandler, boolean)
    pub fn new_with_options(
        redeclaration_handler: Box<dyn RedeclarationHandler + 'a>,
        treat_provides_as_redeclarations: bool,
    ) -> Self {
        Self {
            redeclaration_handler,
            treat_provides_as_redeclarations,
        }
    }
}

impl ScopeCreator for SyntacticScopeCreator<'_> {
    // port: SyntacticScopeCreator#createScope(Node, AbstractScope<?, ?>)
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        SyntacticScopeCreator::create_scope(self, compiler, n, parent)
    }
}

impl SyntacticScopeCreator<'_> {
    // port: SyntacticScopeCreator#createScope(Node, Scope)
    pub fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        let scope = match parent {
            None => ScopeId::create_global_scope(compiler, n),
            Some(parent) => ScopeId::create_child_scope(compiler, parent, n),
        };
        ScopeScanner::new(
            compiler,
            &mut *self.redeclaration_handler,
            scope,
            None,
            self.treat_provides_as_redeclarations,
        )
        .populate(compiler);
        scope
    }
}

struct ScopeScanner<'a> {
    scope: ScopeId,
    redeclaration_handler: &'a mut dyn RedeclarationHandler,
    treat_provides_as_redeclarations: bool,
    input_id: Option<Arc<InputId>>,
    change_root_set: Option<IndexSet<NodeId>>,
}

impl<'a> ScopeScanner<'a> {
    // port: SyntacticScopeCreator.ScopeScanner#ScopeScanner
    fn new(
        compiler: &AbstractCompiler,
        redeclaration_handler: &'a mut dyn RedeclarationHandler,
        scope: ScopeId,
        change_root_set: Option<IndexSet<NodeId>>,
        treat_provides_as_redeclarations: bool,
    ) -> Self {
        check_state!(change_root_set.is_none() || scope.is_global(compiler));
        Self {
            scope,
            redeclaration_handler,
            treat_provides_as_redeclarations,
            input_id: None,
            change_root_set,
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#populate
    fn populate(&mut self, compiler: &mut AbstractCompiler) {
        let n = self.scope.get_root_node(compiler);
        self.input_id = NodeUtil::get_input_id(compiler, n);
        match n.get_token(compiler) {
            Token::FUNCTION => {
                let fn_name_node = check_not_null!(n.get_first_child(compiler));
                let args = check_not_null!(fn_name_node.get_next(compiler));
                check_state!(args.is_param_list(compiler));
                self.declare_lhs(compiler, self.scope, args);
                let fn_name = fn_name_node.get_string(compiler);
                if !fn_name.is_empty() && NodeUtil::is_function_expression(compiler, n) {
                    self.declare_var(compiler, self.scope, fn_name_node);
                }
            }
            Token::CLASS => {
                let class_name_node = check_not_null!(n.get_first_child(compiler));
                if !class_name_node.is_empty(compiler) && NodeUtil::is_class_expression(compiler, n)
                {
                    self.declare_var(compiler, self.scope, class_name_node);
                }
            }
            Token::ROOT | Token::SCRIPT => {
                check_state!(
                    self.scope.is_global(compiler),
                    &self.scope.to_string(compiler)
                );
                self.scan_vars(compiler, n, Some(self.scope), Some(self.scope));
            }
            Token::MODULE_BODY => {
                self.scan_vars(compiler, n, Some(self.scope), Some(self.scope));
            }
            Token::FOR
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::FOR_IN
            | Token::SWITCH_BODY => {
                self.scan_vars(compiler, n, None, Some(self.scope));
            }
            Token::BLOCK => {
                if NodeUtil::is_function_block(compiler, n)
                    || NodeUtil::is_class_static_block(compiler, n)
                {
                    self.scan_vars(compiler, n, Some(self.scope), Some(self.scope));
                } else {
                    self.scan_vars(compiler, n, None, Some(self.scope));
                }
            }
            Token::COMPUTED_FIELD_DEF | Token::MEMBER_FIELD_DEF => {}
            _ => panic!("Illegal scope root: {}", n.to_string(compiler)),
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#declareLHS
    fn declare_lhs(&mut self, compiler: &mut AbstractCompiler, s: ScopeId, n: NodeId) {
        if n.has_one_child(compiler)
            && check_not_null!(n.get_first_child(compiler)).is_name(compiler)
        {
            let name_node = check_not_null!(n.get_first_child(compiler));
            self.declare_var(compiler, s, name_node);
        } else {
            NodeUtil::visit_lhs_nodes_in_node(compiler, n, &mut |compiler, lhs| {
                self.declare_var(compiler, s, lhs);
            });
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#scanVars
    #[allow(clippy::collapsible_if, clippy::collapsible_match)] // Retain Java control flow.
    fn scan_vars(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        hoist_scope: Option<ScopeId>,
        block_scope: Option<ScopeId>,
    ) {
        match n.get_token(compiler) {
            Token::VAR => {
                if let Some(hoist_scope) = hoist_scope {
                    self.declare_lhs(compiler, hoist_scope, n);
                }
                return;
            }
            Token::LET | Token::CONST => {
                if let Some(block_scope) = block_scope {
                    self.declare_lhs(compiler, block_scope, n);
                }
                return;
            }
            Token::IMPORT => {
                self.declare_lhs(compiler, check_not_null!(hoist_scope), n);
                return;
            }
            Token::EXPORT => {
                let declaration = check_not_null!(n.get_first_child(compiler));
                self.scan_vars(compiler, declaration, hoist_scope, block_scope);
                return;
            }
            Token::FUNCTION => {
                if NodeUtil::is_function_expression(compiler, n) || block_scope.is_none() {
                    return;
                }
                let fn_name_node = check_not_null!(n.get_first_child(compiler));
                if fn_name_node.get_string_ref(compiler).is_empty() {
                    return;
                }
                self.declare_var(compiler, check_not_null!(block_scope), fn_name_node);
                return;
            }
            Token::CLASS => {
                if NodeUtil::is_class_expression(compiler, n) || block_scope.is_none() {
                    return;
                }
                let class_name_node = check_not_null!(n.get_first_child(compiler));
                if class_name_node.get_string_ref(compiler).is_empty() {
                    return;
                }
                self.declare_var(compiler, check_not_null!(block_scope), class_name_node);
                return;
            }
            Token::CATCH => {
                check_state!(n.has_two_children(compiler), &n.to_string(compiler));
                if let Some(block_scope) = block_scope {
                    self.declare_lhs(compiler, block_scope, n);
                }
                let block = check_not_null!(n.get_second_child(compiler));
                self.scan_vars(compiler, block, hoist_scope, block_scope);
                return;
            }
            Token::SCRIPT => {
                if self
                    .change_root_set
                    .as_ref()
                    .is_some_and(|set| !set.contains(&n))
                {
                    return;
                }
                self.input_id = n.get_input_id(compiler);
            }
            Token::MODULE_BODY => {
                let hoist_scope = check_not_null!(hoist_scope);
                if hoist_scope.is_global(compiler) {
                    if let Some(expr) = n.get_first_child(compiler) {
                        if Self::is_legacy_goog_module(compiler, expr) {
                            self.declare_implicit_goog_namespace_from_call(
                                compiler,
                                hoist_scope,
                                expr,
                            );
                        }
                    }
                    return;
                }
            }
            Token::EXPR_RESULT => {
                if check_not_null!(n.get_parent(compiler)).is_script(compiler) {
                    if NodeUtil::is_goog_provide_call(compiler, n) {
                        let global_scope = check_not_null!(hoist_scope).get_global_scope(compiler);
                        self.declare_implicit_goog_namespace_from_call(compiler, global_scope, n);
                    } else {
                        let call = check_not_null!(n.get_first_child(compiler));
                        if NodeUtil::is_bundled_goog_module_call(compiler, call)
                            && check_not_null!(call.get_second_child(compiler))
                                .is_function(compiler)
                        {
                            let function = check_not_null!(call.get_second_child(compiler));
                            let module_call = check_not_null!(
                                NodeUtil::get_function_body(compiler, function)
                                    .get_first_child(compiler)
                            );
                            if Self::is_legacy_goog_module(compiler, module_call) {
                                let global_scope =
                                    check_not_null!(hoist_scope).get_global_scope(compiler);
                                self.declare_implicit_goog_namespace_from_call(
                                    compiler,
                                    global_scope,
                                    module_call,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        let is_block_start = block_scope.is_some_and(|scope| n == scope.get_root_node(compiler));
        let entering_new_block = !is_block_start && NodeUtil::creates_block_scope(compiler, n);
        if entering_new_block && hoist_scope.is_none() {
            return;
        }
        if NodeUtil::is_shallow_statement_tree(compiler, Some(n)) {
            let mut child = n.get_first_child(compiler);
            while let Some(current) = child {
                let next = current.get_next(compiler);
                self.scan_vars(
                    compiler,
                    current,
                    hoist_scope,
                    if entering_new_block {
                        None
                    } else {
                        block_scope
                    },
                );
                child = next;
            }
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#declareVar
    fn declare_var(&mut self, compiler: &mut AbstractCompiler, s: ScopeId, n: NodeId) {
        check_state!(
            n.is_name(compiler) || n.is_import_star(compiler),
            "Invalid node for declareVar: %s",
            n.to_string(compiler)
        );
        let name = n.get_string(compiler);
        let mut var = s.get_own_slot(compiler, &name);
        if let Some(existing) = var {
            if existing.get_node(compiler) == Some(n) {
                return;
            }
            if existing.is_implicit_goog_namespace(compiler) {
                s.undeclare(compiler, existing);
                var = None;
            }
        }
        let input = self
            .input_id
            .as_deref()
            .and_then(|input_id| compiler.get_input(input_id))
            .cloned();
        if var.is_some()
            || !Self::is_shadowing_allowed(compiler, &name, s)
            || ((s.is_function_scope(compiler) || s.is_function_block_scope(compiler))
                && name == ARGUMENTS)
        {
            self.redeclaration_handler
                .on_redeclaration(compiler, s, &name, n, input);
        } else {
            s.declare(compiler, name, n, input);
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#declareImplicitGoogNamespaceFromCall
    #[allow(clippy::collapsible_if)] // Retain Java control flow.
    fn declare_implicit_goog_namespace_from_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        s: ScopeId,
        expr_call: NodeId,
    ) {
        let namespace_node =
            check_not_null!(expr_call.get_first_child(compiler)).get_second_child(compiler);
        let Some(namespace_node) = namespace_node else {
            return;
        };
        if !namespace_node.is_string_lit(compiler) {
            return;
        }
        let namespace = namespace_node.get_string(compiler);
        let root = NodeUtil::get_root_of_qualified_name_string(&namespace);
        if root.is_empty() {
            return;
        }
        if self.treat_provides_as_redeclarations {
            if let Some(existing) = s.get_own_slot(compiler, &root) {
                if !existing.is_implicit_goog_namespace(compiler) {
                    let input = self
                        .input_id
                        .as_deref()
                        .and_then(|input_id| compiler.get_input(input_id))
                        .cloned();
                    let declaration = check_not_null!(existing.get_node(compiler));
                    self.redeclaration_handler.on_redeclaration(
                        compiler,
                        s,
                        &root,
                        declaration,
                        input,
                    );
                }
            }
        }
        s.declare_implicit_goog_namespace_if_absent(compiler, root, namespace_node);
    }

    // port: SyntacticScopeCreator.ScopeScanner#isShadowingAllowed
    fn is_shadowing_allowed(compiler: &mut AbstractCompiler, name: &JsString, s: ScopeId) -> bool {
        if s.is_function_block_scope(compiler) {
            let maybe_param = check_not_null!(s.get_parent(compiler)).get_own_slot(compiler, name);
            return maybe_param.is_none_or(|param| !param.is_param(compiler));
        }
        true
    }

    // port: SyntacticScopeCreator.ScopeScanner#isLegacyGoogModule
    fn is_legacy_goog_module(compiler: &AbstractCompiler, n: NodeId) -> bool {
        if !NodeUtil::is_goog_module_call(compiler, n) {
            return false;
        }
        n.get_next(compiler).is_some_and(|next| {
            NodeUtil::is_goog_module_declare_legacy_namespace_call(compiler, next)
        })
    }
}

pub trait RedeclarationHandler {
    // port: SyntacticScopeCreator.RedeclarationHandler#onRedeclaration
    fn on_redeclaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        s: ScopeId,
        name: &JsString,
        n: NodeId,
        input: Option<CompilerInput>,
    );
}

pub struct DefaultRedeclarationHandler;

impl RedeclarationHandler for DefaultRedeclarationHandler {
    // port: SyntacticScopeCreator.DefaultRedeclarationHandler#onRedeclaration
    fn on_redeclaration(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _s: ScopeId,
        _name: &JsString,
        _n: NodeId,
        _input: Option<CompilerInput>,
    ) {
    }
}
