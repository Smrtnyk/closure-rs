/*
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/AbstractCompiler.java,
//   src/com/google/javascript/jscomp/SyntacticScopeCreator.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_input::CompilerInput,
    node_util::NodeUtil,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    syntactic_scope_cache::{ScopeContext, ScriptDeclaration, SyntacticScopeCache},
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_not_null, check_state, input_id::InputId, js_string::JsString, node::NodeId, token::Token,
};
use std::sync::Arc;

pub struct SyntacticScopeCreator<'a> {
    redeclaration_handler: Box<dyn RedeclarationHandler + 'a>,
    treat_provides_as_redeclarations: bool,
    /// Rust-only (performance divergence): whether this creator's scopes are reused across passes,
    /// true unless provides count as redeclarations; see `SyntacticScopeCache`.
    reusable: bool,
    /// Rust-only (performance divergence): whether the redeclaration handler is the default one,
    /// which has no effects. A creator with another handler reuses, and keeps, only scopes whose
    /// scan called no handler, so that its handler would not have been called either.
    default_handler: bool,
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
        let mut creator =
            Self::new_with_redeclaration_handler(Box::new(DEFAULT_REDECLARATION_HANDLER));
        creator.default_handler = true;
        creator
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
            reusable: !treat_provides_as_redeclarations
                && *crate::syntactic_scope_cache::REDECLARATION_FREE_REUSE,
            default_handler: false,
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
        // Rust-only (performance divergence): a scope that an earlier pass scanned from code nobody
        // has changed since is handed out again instead of being scanned anew (see
        // SyntacticScopeCache).
        let context = if self.reusable || self.default_handler {
            ScopeContext::of(compiler, n, parent)
        } else {
            None
        };
        if let Some(context) = &context
            && let Some(scope) =
                SyntacticScopeCache::reuse(compiler, n, parent, context, !self.default_handler)
                    .or_else(|| {
                        SyntacticScopeCache::reuse_global_after_removals(
                            compiler,
                            n,
                            context,
                            !self.default_handler,
                            &mut ScopeScanner::script_declarations_now,
                        )
                    })
        {
            if SyntacticScopeCache::verifying() {
                let (fresh, redeclared) = self.scan(compiler, n, parent, false);
                SyntacticScopeCache::verify(compiler, scope, fresh, redeclared);
            }
            return scope;
        }
        let scanned_at = if context.is_some() {
            SyntacticScopeCache::scan_round(compiler)
        } else {
            0
        };
        // A global scope takes the declarations of unchanged scripts from the cache too.
        let replay_scripts =
            self.default_handler && context.as_ref().is_some_and(ScopeContext::is_global_root);
        let (scope, redeclared, script_lists) =
            self.scan_with_script_lists(compiler, n, parent, replay_scripts);
        if replay_scripts && SyntacticScopeCache::verifying() {
            let (fresh, fresh_redeclared) = self.scan(compiler, n, parent, false);
            assert_eq!(
                redeclared, fresh_redeclared,
                "syntactic scope cache: replayed redeclarations"
            );
            SyntacticScopeCache::verify(compiler, scope, fresh, redeclared);
        }
        if let Some(context) = context
            && (self.default_handler || !redeclared)
        {
            SyntacticScopeCache::keep(
                compiler,
                n,
                scope,
                scanned_at,
                context,
                redeclared,
                script_lists,
            );
        }
        scope
    }

    // port: SyntacticScopeCreator#createScope(Node, Scope)
    /// The Java method's body: a new scope scanned from `n` (Rust-only: `replay_scripts`, see
    /// `ScopeScanner::scan_script`).
    /// Also returns whether the scan called the redeclaration handler.
    fn scan(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
        replay_scripts: bool,
    ) -> (ScopeId, bool) {
        let (scope, redeclared, _) =
            self.scan_with_script_lists(compiler, n, parent, replay_scripts);
        (scope, redeclared)
    }

    /// `scan`, with the declarations made for each script of a global scope (`replay_scripts`).
    fn scan_with_script_lists(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
        replay_scripts: bool,
    ) -> (ScopeId, bool, Vec<Option<Arc<[ScriptDeclaration]>>>) {
        let scope = match parent {
            None => ScopeId::create_global_scope(compiler, n),
            Some(parent) => ScopeId::create_child_scope(compiler, parent, n),
        };
        let mut scanner = ScopeScanner::new(
            compiler,
            &mut *self.redeclaration_handler,
            scope,
            None,
            self.treat_provides_as_redeclarations,
        );
        scanner.replay_scripts = replay_scripts;
        scanner.populate(compiler);
        (scope, scanner.redeclared, scanner.script_lists)
    }
}

struct ScopeScanner<'a> {
    scope: ScopeId,
    redeclaration_handler: &'a mut dyn RedeclarationHandler,
    treat_provides_as_redeclarations: bool,
    input_id: Option<Arc<InputId>>,
    change_root_set: Option<IndexSet<NodeId>>,
    /// Rust-only: `compiler.getInput(inputId)` for the `input_id` it was looked up with, so that
    /// each declaration needs no lookup by id (performance divergence).
    input_cache: Option<(Arc<InputId>, Option<CompilerInput>)>,
    /// Rust-only (performance divergence): whether the top-level declarations of each script of
    /// this global scope are replayed from, or recorded for, the `SyntacticScopeCache` (see
    /// `scan_script`).
    replay_scripts: bool,
    /// Rust-only (performance divergence): the declarations of the script being scanned, while
    /// recorded; None once a declaration went to another scope (the record is then not kept).
    recording: Option<Option<Vec<ScriptDeclaration>>>,
    /// Rust-only (performance divergence): whether the redeclaration handler was called.
    redeclared: bool,
    /// Rust-only (performance divergence): the declarations made for each script `scan_script`
    /// scanned or replayed, in order (`None` for a script whose record was not kept).
    script_lists: Vec<Option<Arc<[ScriptDeclaration]>>>,
    /// Rust-only (performance divergence): declarations are only recorded, not made
    /// (`script_declarations_now`).
    record_only: bool,
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
            input_cache: None,
            replay_scripts: false,
            recording: None,
            redeclared: false,
            script_lists: Vec::new(),
            record_only: false,
        }
    }

    /// Rust-only (performance divergence): the declarations a scan of the global scope `global`
    /// makes for the top level of `script` (`scan_script`), without making them; `None` when one
    /// would go to another scope. The scan reads the AST alone.
    fn script_declarations_now(
        compiler: &mut AbstractCompiler,
        global: ScopeId,
        script: NodeId,
    ) -> Option<Vec<ScriptDeclaration>> {
        let mut handler = DefaultRedeclarationHandler;
        let mut scanner = ScopeScanner::new(compiler, &mut handler, global, None, false);
        scanner.record_only = true;
        scanner.input_id = script.get_input_id(compiler);
        scanner.recording = Some(Some(Vec::new()));
        scanner.scan_vars_of_children(compiler, script, Some(global), Some(global));
        scanner.recording.take().flatten()
    }

    // port: AbstractCompiler#getInput (for the scanner's current input id)
    fn current_input(&mut self, compiler: &AbstractCompiler) -> Option<CompilerInput> {
        let input_id = self.input_id.as_ref()?;
        if let Some((cached_id, input)) = &self.input_cache
            && Arc::ptr_eq(cached_id, input_id)
        {
            return input.clone();
        }
        let input = compiler.get_input(input_id).cloned();
        self.input_cache = Some((Arc::clone(input_id), input.clone()));
        input
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
                if !fn_name_node.get_string_ref(compiler).is_empty()
                    && NodeUtil::is_function_expression(compiler, n)
                {
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
                if self.replay_scripts
                    && self.recording.is_none()
                    && hoist_scope == Some(self.scope)
                    && block_scope == Some(self.scope)
                {
                    self.scan_script(compiler, n);
                    return;
                }
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

        self.scan_vars_of_children(compiler, n, hoist_scope, block_scope);
    }

    // port: SyntacticScopeCreator.ScopeScanner#scanVars
    /// The end of the Java method (Rust-only split, so that `scan_script` can run it): the
    /// children of `n`.
    fn scan_vars_of_children(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        hoist_scope: Option<ScopeId>,
        block_scope: Option<ScopeId>,
    ) {
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

    /// Rust-only (performance divergence): the top level of `script` in a global scope. The
    /// declarations an earlier scan found in it are made again, in their order, while the script is
    /// unchanged (the externs and the unchanged scripts of a program need no walk of their
    /// statements); else the script is scanned and its declarations are recorded. The declarations
    /// go through `declare_var` as in a scan, so redeclarations across scripts are found the same
    /// way.
    fn scan_script(&mut self, compiler: &mut AbstractCompiler, script: NodeId) {
        if let Some(declarations) = SyntacticScopeCache::script_declarations(compiler, script) {
            self.script_lists.push(Some(Arc::clone(&declarations)));
            for declaration in declarations.iter() {
                match *declaration {
                    ScriptDeclaration::Name { name, .. } => {
                        self.declare_var(compiler, self.scope, name);
                    }
                    ScriptDeclaration::GoogNamespace(expr_call) => {
                        self.declare_implicit_goog_namespace_from_call(
                            compiler, self.scope, expr_call,
                        );
                    }
                }
            }
            return;
        }
        let scanned_at = SyntacticScopeCache::scan_round(compiler);
        self.recording = Some(Some(Vec::new()));
        self.scan_vars_of_children(compiler, script, Some(self.scope), Some(self.scope));
        if let Some(Some(declarations)) = self.recording.take() {
            let declarations: Arc<[ScriptDeclaration]> = declarations.into();
            self.script_lists.push(Some(Arc::clone(&declarations)));
            SyntacticScopeCache::keep_script_declarations(
                compiler,
                script,
                scanned_at,
                declarations,
            );
        } else {
            self.script_lists.push(None);
        }
    }

    /// Rust-only (performance divergence): records a declaration for `scan_script`.
    fn record(&mut self, s: ScopeId, declaration: ScriptDeclaration) {
        if let Some(recording) = &mut self.recording {
            if s == self.scope {
                if let Some(declarations) = recording {
                    declarations.push(declaration);
                }
            } else {
                *recording = None;
            }
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#declareVar
    fn declare_var(&mut self, compiler: &mut AbstractCompiler, s: ScopeId, n: NodeId) {
        if self.recording.is_some() {
            let parent = n.get_parent(compiler);
            self.record(s, ScriptDeclaration::Name { name: n, parent });
            if self.record_only {
                return;
            }
        }
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
        let input = self.current_input(compiler);
        if var.is_some()
            || !Self::is_shadowing_allowed(compiler, &name, s)
            || ((s.is_function_scope(compiler) || s.is_function_block_scope(compiler))
                && name == ARGUMENTS)
        {
            self.redeclared = true;
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
        self.record(s, ScriptDeclaration::GoogNamespace(expr_call));
        if self.record_only {
            return;
        }
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
                    self.redeclared = true;
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

/// Rust-only (performance divergence): lazily materializes only the scope chain from `root` down
/// to `target_scope_root` (matching `NodeTraversal`'s exact scope push rules) and caches each
/// created scope in `memo`. Lets per-function passes skip creating scopes for unchanged or
/// ineligible functions while preserving exact scope depths for `SyntacticScopeCache`.
pub(crate) fn ensure_ancestor_scope(
    compiler: &mut AbstractCompiler,
    scope_creator: &mut dyn ScopeCreator,
    memo: &mut closure_rhino::fast_hash::IndexMap<NodeId, ScopeId>,
    root: NodeId,
    target_scope_root: NodeId,
) -> ScopeId {
    if let Some(&scope) = memo.get(&target_scope_root) {
        return scope;
    }
    let mut path: Vec<(NodeId, NodeId)> = Vec::new();
    let mut cur = target_scope_root;
    while cur != root {
        let parent = cur
            .get_parent(compiler)
            .expect("target_scope_root must be under root");
        path.push((parent, cur));
        cur = parent;
    }
    let mut current_scope = if let Some(&scope) = memo.get(&root) {
        scope
    } else {
        let scope = scope_creator.create_scope(compiler, root, None);
        memo.insert(root, scope);
        scope
    };
    // Walk from just below `root` down to `target_scope_root`'s parent.
    // `path.last()` has `anc == root` (already created); inspect every intermediate ancestor.
    for &(anc, child) in path.iter().rev() {
        if anc == root {
            continue;
        }
        let pushes_scope = match anc.get_token(compiler) {
            Token::FUNCTION | Token::MODULE_BODY | Token::MEMBER_FIELD_DEF => true,
            Token::CLASS => Some(child) != anc.get_second_child(compiler),
            Token::COMPUTED_FIELD_DEF => {
                anc.has_two_children(compiler) && Some(child) == anc.get_last_child(compiler)
            }
            Token::BLOCK => {
                !anc.is_synthetic_block(compiler)
                    && anc.get_parent(compiler).is_some_and(|p| {
                        !NodeUtil::is_switch_case(compiler, p) && !p.is_catch(compiler)
                    })
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY => true,
            _ => false,
        };
        if pushes_scope {
            current_scope = if let Some(&scope) = memo.get(&anc) {
                scope
            } else {
                let scope = scope_creator.create_scope(compiler, anc, Some(current_scope));
                memo.insert(anc, scope);
                scope
            };
        }
    }
    if target_scope_root == root {
        return current_scope;
    }
    let scope = scope_creator.create_scope(compiler, target_scope_root, Some(current_scope));
    memo.insert(target_scope_root, scope);
    scope
}
