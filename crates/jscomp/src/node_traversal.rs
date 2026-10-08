/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/NodeTraversalTest.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::AbstractScopeHandle,
    change_tracker::ChangeTracker,
    compiler_input::CompilerInput,
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    diagnostic_type::DiagnosticType,
    js_chunk::JSChunk,
    js_error::JSError,
    memoized_scope_creator::MemoizedScopeCreator,
    modules::module_metadata_map::{ModuleMetadata, ModuleMetadataMap},
    node_util::NodeUtil,
    platform::Platform,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::{
    check_not_null, check_state,
    input_id::InputId,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::{
    any::Any,
    collections::VecDeque,
    ops::{Deref, DerefMut},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, LazyLock},
};

// Rust-only representations of Java's Object-valued lazy scope stack and of
// a borrowed or default-created ScopeCreator.
#[derive(Clone, Copy)]
enum ScopeObject {
    Node(NodeId),
    Scope(AbstractScopeHandle),
}
enum ScopeCreatorHolder<'a> {
    Borrowed(&'a mut dyn ScopeCreator),
    Owned(Box<dyn ScopeCreator + 'a>),
}
impl ScopeCreatorHolder<'_> {
    fn get_mut(&mut self) -> &mut dyn ScopeCreator {
        match self {
            Self::Borrowed(value) => &mut **value,
            Self::Owned(value) => &mut **value,
        }
    }
}
enum CallbackHolder<'a> {
    Borrowed(&'a mut dyn Callback),
    Owned(Box<dyn Callback + 'a>),
}
impl CallbackHolder<'_> {
    fn get_mut(&mut self) -> &mut dyn Callback {
        match self {
            Self::Borrowed(value) => &mut **value,
            Self::Owned(value) => &mut **value,
        }
    }
}

/// Java's callback is taken out during traversal and threaded through the
/// recursive calls, so callbacks can receive the entire mutable traversal.
pub struct NodeTraversal<'a> {
    compiler: &'a mut AbstractCompiler,
    callback: Option<&'a mut dyn Callback>,
    scope_callback: bool,
    scope_creator: ScopeCreatorHolder<'a>,
    obey_destructuring_and_default_value_execution_order: bool,
    may_contain_synthetic_blocks: bool,
    current_node: Option<NodeId>,
    current_hoist_scope_root: Option<NodeId>,
    current_function: Option<NodeId>,
    current_script: Option<NodeId>,
    current_change_scope: Option<NodeId>,
    scopes: Vec<ScopeObject>,
    source_name: Option<String>,
    input_id: Option<Arc<InputId>>,
    // The CompilerInput is retained in the compiler; this ID keeps cached
    // lookups referring to that same object rather than copying its fields.
    compiler_input: Option<Arc<InputId>>,
}

pub trait Callback {
    // port: NodeTraversal.Callback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool;

    // port: NodeTraversal.Callback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>);

    // Rust-only counterpart of Java's instanceof ScopedCallback. Implementors
    // of ScopedCallback return Some(self); ordinary callbacks retain None.
    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        None
    }
}

pub trait ScopedCallback: Callback {
    // port: NodeTraversal.ScopedCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>);

    // port: NodeTraversal.ScopedCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>);
}

pub struct AbstractPostOrderCallback<C> {
    callback: C,
}
impl<C> AbstractPostOrderCallback<C> {
    // Rust-only adapter for Java's abstract subclass / lambda construction.
    pub fn new(callback: C) -> Self {
        Self { callback }
    }

    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    pub fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
}

pub struct ExternsSkippingCallback<C> {
    callback: C,
}
impl<C> ExternsSkippingCallback<C> {
    pub fn new(callback: C) -> Self {
        Self { callback }
    }
}
impl<C: AbstractPostOrderCallbackInterface> Callback for ExternsSkippingCallback<C> {
    // port: NodeTraversal.ExternsSkippingCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !n.is_script(t) || !n.is_from_externs(t) || NodeUtil::is_from_type_summary(t, n)
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.callback.visit(t, n, parent);
    }
}

pub trait AbstractPostOrderCallbackInterface {
    // port: NodeTraversal.AbstractPostOrderCallbackInterface#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>);
}
impl<F> AbstractPostOrderCallbackInterface for F
where
    F: FnMut(&mut NodeTraversal<'_>, NodeId, Option<NodeId>),
{
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self(t, n, parent);
    }
}

pub struct AbstractPreOrderCallback<F> {
    should_traverse: F,
}
impl<F> AbstractPreOrderCallback<F> {
    pub fn new(should_traverse: F) -> Self {
        Self { should_traverse }
    }
}
impl<F> Callback for AbstractPreOrderCallback<F>
where
    F: FnMut(&mut NodeTraversal<'_>, NodeId, Option<NodeId>) -> bool,
{
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        (self.should_traverse)(t, n, parent)
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

type ScopeHook<'a> = Box<dyn FnMut(&mut NodeTraversal<'_>) + 'a>;
pub struct AbstractScopedCallback<'a, C> {
    callback: C,
    enter_scope: Option<ScopeHook<'a>>,
    exit_scope: Option<ScopeHook<'a>>,
}
impl<C> AbstractScopedCallback<'_, C> {
    pub fn new(callback: C) -> Self {
        Self {
            callback,
            enter_scope: None,
            exit_scope: None,
        }
    }
}
impl<'a, C> AbstractScopedCallback<'a, C> {
    pub fn with_scope_callbacks(
        callback: C,
        enter_scope: impl FnMut(&mut NodeTraversal<'_>) + 'a,
        exit_scope: impl FnMut(&mut NodeTraversal<'_>) + 'a,
    ) -> Self {
        Self {
            callback,
            enter_scope: Some(Box::new(enter_scope)),
            exit_scope: Some(Box::new(exit_scope)),
        }
    }
}
impl<C: AbstractPostOrderCallbackInterface> Callback for AbstractScopedCallback<'_, C> {
    // port: NodeTraversal.AbstractScopedCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.callback.visit(t, n, parent);
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}
impl<C: AbstractPostOrderCallbackInterface> ScopedCallback for AbstractScopedCallback<'_, C> {
    // port: NodeTraversal.AbstractScopedCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(enter_scope) = &mut self.enter_scope {
            enter_scope(t);
        }
    }

    // port: NodeTraversal.AbstractScopedCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(exit_scope) = &mut self.exit_scope {
            exit_scope(t);
        }
    }
}

enum CfgObject {
    Root(NodeId),
    Graph(Box<ControlFlowGraph<NodeId>>),
}
type CfgScopeHook<'a> = Box<dyn FnMut(&mut AbstractCfgCallback<'a>, &mut NodeTraversal<'_>) + 'a>;
type CfgShouldTraverseHook<'a> = Box<
    dyn FnMut(&mut AbstractCfgCallback<'a>, &mut NodeTraversal<'_>, NodeId, Option<NodeId>) -> bool
        + 'a,
>;
type CfgVisitHook<'a> = Box<
    dyn FnMut(&mut AbstractCfgCallback<'a>, &mut NodeTraversal<'_>, NodeId, Option<NodeId>) + 'a,
>;
pub struct AbstractCfgCallback<'a> {
    cfgs: VecDeque<CfgObject>,
    enter_scope_with_cfg: Option<CfgScopeHook<'a>>,
    exit_scope_with_cfg: Option<CfgScopeHook<'a>>,
    should_traverse: Option<CfgShouldTraverseHook<'a>>,
    visit: Option<CfgVisitHook<'a>>,
}
impl Default for AbstractCfgCallback<'_> {
    fn default() -> Self {
        Self::new()
    }
}
impl<'a> AbstractCfgCallback<'a> {
    pub fn new() -> Self {
        Self {
            cfgs: VecDeque::new(),
            enter_scope_with_cfg: None,
            exit_scope_with_cfg: None,
            should_traverse: None,
            visit: None,
        }
    }

    // Rust-only adapters for overriding Java's non-final methods. The extra
    // receiver gives a subclass hook access to getControlFlowGraph.
    pub fn set_enter_scope_with_cfg(
        &mut self,
        hook: impl FnMut(&mut Self, &mut NodeTraversal<'_>) + 'a,
    ) -> &mut Self {
        self.enter_scope_with_cfg = Some(Box::new(hook));
        self
    }
    pub fn set_exit_scope_with_cfg(
        &mut self,
        hook: impl FnMut(&mut Self, &mut NodeTraversal<'_>) + 'a,
    ) -> &mut Self {
        self.exit_scope_with_cfg = Some(Box::new(hook));
        self
    }
    pub fn set_should_traverse(
        &mut self,
        hook: impl FnMut(&mut Self, &mut NodeTraversal<'_>, NodeId, Option<NodeId>) -> bool + 'a,
    ) -> &mut Self {
        self.should_traverse = Some(Box::new(hook));
        self
    }
    pub fn set_visit(
        &mut self,
        hook: impl FnMut(&mut Self, &mut NodeTraversal<'_>, NodeId, Option<NodeId>) + 'a,
    ) -> &mut Self {
        self.visit = Some(Box::new(hook));
        self
    }

    // port: NodeTraversal.AbstractCfgCallback#getControlFlowGraph
    pub fn get_control_flow_graph(
        &mut self,
        compiler: &mut AbstractCompiler,
    ) -> &ControlFlowGraph<NodeId> {
        check_state!(!self.cfgs.is_empty());
        if let Some(CfgObject::Root(cfg_root)) = self.cfgs.front() {
            let result = ControlFlowAnalysis::builder()
                .set_compiler(compiler)
                .set_cfg_root(*cfg_root)
                .set_include_edge_annotations(true)
                .compute_cfg(compiler);
            self.cfgs.pop_front();
            self.cfgs.push_front(CfgObject::Graph(Box::new(result)));
        }
        match self.cfgs.front().unwrap() {
            CfgObject::Graph(result) => result,
            CfgObject::Root(_) => unreachable!(),
        }
    }

    // Rust-only: Java's getControlFlowGraph result is mutable (GraphReachability annotates its
    // nodes); this is the same lookup with a mutable borrow.
    pub fn get_control_flow_graph_mut(
        &mut self,
        compiler: &mut AbstractCompiler,
    ) -> &mut ControlFlowGraph<NodeId> {
        self.get_control_flow_graph(compiler);
        match self.cfgs.front_mut().unwrap() {
            CfgObject::Graph(result) => result,
            CfgObject::Root(_) => unreachable!(),
        }
    }
}
impl ScopedCallback for AbstractCfgCallback<'_> {
    // port: NodeTraversal.AbstractCfgCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            self.cfgs.push_front(CfgObject::Root(current_scope_root));
        }
        self.enter_scope_with_cfg(t);
    }

    // port: NodeTraversal.AbstractCfgCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.exit_scope_with_cfg(t);
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            check_not_null!(self.cfgs.pop_front());
        }
    }
}
impl AbstractCfgCallback<'_> {
    // port: NodeTraversal.AbstractCfgCallback#enterScopeWithCfg
    pub fn enter_scope_with_cfg(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(mut enter_scope_with_cfg) = self.enter_scope_with_cfg.take() {
            enter_scope_with_cfg(self, t);
            self.enter_scope_with_cfg = Some(enter_scope_with_cfg);
        }
    }

    // port: NodeTraversal.AbstractCfgCallback#exitScopeWithCfg
    pub fn exit_scope_with_cfg(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(mut exit_scope_with_cfg) = self.exit_scope_with_cfg.take() {
            exit_scope_with_cfg(self, t);
            self.exit_scope_with_cfg = Some(exit_scope_with_cfg);
        }
    }
}
impl Callback for AbstractCfgCallback<'_> {
    // port: NodeTraversal.AbstractCfgCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if let Some(mut should_traverse) = self.should_traverse.take() {
            let result = should_traverse(self, t, n, parent);
            self.should_traverse = Some(should_traverse);
            return result;
        }
        true
    }

    // port: NodeTraversal.AbstractCfgCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if let Some(mut visit) = self.visit.take() {
            visit(self, t, n, parent);
            self.visit = Some(visit);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

pub struct AbstractShallowCallback<C> {
    callback: C,
}
impl<C> AbstractShallowCallback<C> {
    pub fn new(callback: C) -> Self {
        Self { callback }
    }
}
impl<C: AbstractPostOrderCallbackInterface> Callback for AbstractShallowCallback<C> {
    // port: NodeTraversal.AbstractShallowCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        parent.is_none_or(|parent| {
            !parent.is_function(node_traversal) || Some(n) == parent.get_first_child(node_traversal)
        })
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.callback.visit(t, n, parent);
    }
}

pub struct AbstractShallowStatementCallback<C> {
    callback: C,
}
impl<C> AbstractShallowStatementCallback<C> {
    pub fn new(callback: C) -> Self {
        Self { callback }
    }
}
impl<C: AbstractPostOrderCallbackInterface> Callback for AbstractShallowStatementCallback<C> {
    // port: NodeTraversal.AbstractShallowStatementCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        NodeUtil::is_shallow_statement_tree(node_traversal, parent)
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.callback.visit(t, n, parent);
    }
}

pub trait ModuleCallback {
    fn enter_module(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _current_module: &Arc<ModuleMetadata>,
        _module_scope_root: NodeId,
    ) {
    }
    fn exit_module(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _old_module: &Arc<ModuleMetadata>,
        _module_scope_root: NodeId,
    ) {
    }
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _current_module: Option<&Arc<ModuleMetadata>>,
        _module_scope_root: Option<NodeId>,
    ) -> bool {
        true
    }
    fn visit(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _current_module: Option<&Arc<ModuleMetadata>>,
        _module_scope_root: Option<NodeId>,
    ) {
    }
}

pub struct AbstractModuleCallback<'a, C> {
    module_metadata_map: &'a ModuleMetadataMap,
    current_module: Option<Arc<ModuleMetadata>>,
    scope_root: Option<NodeId>,
    in_load_module: bool,
    callback: C,
}
impl<'a, C: ModuleCallback> AbstractModuleCallback<'a, C> {
    // port: NodeTraversal.AbstractModuleCallback#AbstractModuleCallback
    // The compiler field is supplied by the traversal at each call (DESIGN §6).
    pub fn new(module_metadata_map: &'a ModuleMetadataMap, callback: C) -> Self {
        Self {
            module_metadata_map,
            current_module: None,
            scope_root: None,
            in_load_module: false,
            callback,
        }
    }

    // port: NodeTraversal.AbstractModuleCallback#enterModule
    pub fn enter_module(
        &mut self,
        compiler: &mut AbstractCompiler,
        current_module: &Arc<ModuleMetadata>,
        module_scope_root: NodeId,
    ) {
        self.callback
            .enter_module(compiler, current_module, module_scope_root);
    }

    // port: NodeTraversal.AbstractModuleCallback#exitModule
    pub fn exit_module(
        &mut self,
        compiler: &mut AbstractCompiler,
        old_module: &Arc<ModuleMetadata>,
        module_scope_root: NodeId,
    ) {
        self.callback
            .exit_module(compiler, old_module, module_scope_root);
    }
}

static GOOG_MODULE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.module"));

impl<C: ModuleCallback> Callback for AbstractModuleCallback<'_, C> {
    // port: NodeTraversal.AbstractModuleCallback#shouldTraverse(NodeTraversal, Node, Node)
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::SCRIPT => {
                let input = check_not_null!(t.get_input()).clone();
                let path = input.get_path(t.compiler).to_string();
                self.current_module = self
                    .module_metadata_map
                    .get_modules_by_path()
                    .get(&path)
                    .cloned();
                let current_module = check_not_null!(self.current_module.clone());
                self.scope_root = Some(
                    if n.has_children(t) && n.get_first_child(t).unwrap().is_module_body(t) {
                        n.get_first_child(t).unwrap()
                    } else {
                        n
                    },
                );
                self.enter_module(t.get_compiler(), &current_module, self.scope_root.unwrap());
            }
            Token::BLOCK => {
                if NodeUtil::is_bundled_goog_module_scope_root(t, n) {
                    self.scope_root = Some(n);
                    self.in_load_module = true;
                }
            }
            Token::CALL => {
                if self.in_load_module && GOOG_MODULE.matches(t, n.get_first_child(t).unwrap()) {
                    let namespace = n.get_last_child(t).unwrap().get_string(t);
                    let new_module = check_not_null!(
                        self.module_metadata_map
                            .get_modules_by_goog_namespace()
                            .get(&namespace)
                            .cloned()
                    );
                    if !self
                        .current_module
                        .as_ref()
                        .is_some_and(|current_module| Arc::ptr_eq(&new_module, current_module))
                    {
                        self.current_module = Some(new_module.clone());
                        self.enter_module(t.get_compiler(), &new_module, self.scope_root.unwrap());
                    }
                }
            }
            _ => {}
        }
        self.should_traverse_module(t, n, self.current_module.clone(), self.scope_root)
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_node(t, n, parent);
    }
}
impl<C: ModuleCallback> AbstractModuleCallback<'_, C> {
    // port: NodeTraversal.AbstractModuleCallback#shouldTraverse(NodeTraversal, Node, ModuleMetadata, Node)
    pub fn should_traverse_module(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_module: Option<Arc<ModuleMetadata>>,
        module_scope_root: Option<NodeId>,
    ) -> bool {
        self.callback
            .should_traverse(t, n, current_module.as_ref(), module_scope_root)
    }

    // port: NodeTraversal.AbstractModuleCallback#visit(NodeTraversal, Node, Node)
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit_node(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::SCRIPT => {
                let current_module = check_not_null!(self.current_module.clone());
                self.exit_module(t.get_compiler(), &current_module, self.scope_root.unwrap());
                self.current_module = None;
                self.scope_root = None;
            }
            Token::BLOCK => {
                if NodeUtil::is_bundled_goog_module_scope_root(t, n) {
                    let current_module = check_not_null!(self.current_module.clone());
                    self.exit_module(t.get_compiler(), &current_module, self.scope_root.unwrap());
                    let grandparent = n.get_grandparent(t).unwrap();
                    self.scope_root = grandparent.get_grandparent(t);
                    self.in_load_module = false;
                    let input = check_not_null!(t.get_input()).clone();
                    let path = input.get_path(t.compiler).to_string();
                    self.current_module = self
                        .module_metadata_map
                        .get_modules_by_path()
                        .get(&path)
                        .cloned();
                    check_not_null!(self.current_module.as_ref());
                }
            }
            _ => {}
        }
        self.visit_module(t, n, self.current_module.clone(), self.scope_root);
    }

    // port: NodeTraversal.AbstractModuleCallback#visit(NodeTraversal, Node, ModuleMetadata, Node)
    pub fn visit_module(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_module: Option<Arc<ModuleMetadata>>,
        module_scope_root: Option<NodeId>,
    ) {
        self.callback
            .visit(t, n, current_module.as_ref(), module_scope_root);
    }
}

pub struct AbstractChangedScopeCallback<F> {
    enter_changed_scope_root: F,
}
impl<F> AbstractChangedScopeCallback<F>
where
    F: FnMut(&mut AbstractCompiler, NodeId),
{
    pub fn new(enter_changed_scope_root: F) -> Self {
        Self {
            enter_changed_scope_root,
        }
    }

    // port: NodeTraversal.AbstractChangedScopeCallback#enterChangedScopeRoot
    pub fn enter_changed_scope_root(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        (self.enter_changed_scope_root)(compiler, root);
    }
}
impl<F> Callback for AbstractChangedScopeCallback<F>
where
    F: FnMut(&mut AbstractCompiler, NodeId),
{
    // port: NodeTraversal.AbstractChangedScopeCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if ChangeTracker::is_change_scope_root(t, n) && t.get_compiler().has_scope_changed(n) {
            self.enter_changed_scope_root(t.get_compiler(), n);
        }
        true
    }

    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

impl NodeTraversal<'_> {
    // port: NodeTraversal#builder
    pub fn builder<'a>() -> Builder<'a> {
        Builder::new()
    }
}

pub struct Builder<'a> {
    callback: Option<CallbackHolder<'a>>,
    compiler: Option<&'a mut AbstractCompiler>,
    scope_creator: Option<&'a mut dyn ScopeCreator>,
    obey_destructuring_and_default_value_execution_order: bool,
}
impl<'a> Builder<'a> {
    // port: NodeTraversal.Builder#Builder
    fn new() -> Self {
        Self {
            callback: None,
            compiler: None,
            scope_creator: None,
            obey_destructuring_and_default_value_execution_order: false,
        }
    }

    // port: NodeTraversal.Builder#setCallback(Callback)
    pub fn set_callback(&mut self, x: &'a mut dyn Callback) -> &mut Self {
        self.callback = Some(CallbackHolder::Borrowed(x));
        self
    }

    // port: NodeTraversal.Builder#setCallback(AbstractPostOrderCallbackInterface)
    pub fn set_callback_post_order(
        &mut self,
        x: impl AbstractPostOrderCallbackInterface + 'a,
    ) -> &mut Self {
        self.callback = Some(CallbackHolder::Owned(Box::new(
            AbstractPostOrderCallback::new(x),
        )));
        self
    }
}

impl<C: AbstractPostOrderCallbackInterface> Callback for AbstractPostOrderCallback<C> {
    // Rust-only forwarding of the final AbstractPostOrderCallback method.
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        AbstractPostOrderCallback::should_traverse(self, t, n, parent)
    }

    // port: NodeTraversal.Builder.setCallback#visit
    // The anonymous AbstractPostOrderCallback in the lambda overload delegates
    // to its AbstractPostOrderCallbackInterface exactly as this adapter does.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.callback.visit(t, n, parent);
    }
}

impl<'a> Builder<'a> {
    // port: NodeTraversal.Builder#setCompiler
    pub fn set_compiler(&mut self, x: &'a mut AbstractCompiler) -> &mut Self {
        self.compiler = Some(x);
        self
    }

    // port: NodeTraversal.Builder#setScopeCreator
    pub fn set_scope_creator(&mut self, x: &'a mut dyn ScopeCreator) -> &mut Self {
        self.scope_creator = Some(x);
        self
    }

    // port: NodeTraversal.Builder#setObeyDestructuringAndDefaultValueExecutionOrder
    pub fn set_obey_destructuring_and_default_value_execution_order(
        &mut self,
        x: bool,
    ) -> &mut Self {
        self.obey_destructuring_and_default_value_execution_order = x;
        self
    }

    // port: NodeTraversal.Builder#build
    pub fn build(&mut self) -> NodeTraversal<'_> {
        let compiler = check_not_null!(self.compiler.as_deref_mut());
        let callback = check_not_null!(self.callback.as_mut()).get_mut();
        let scope_creator = self
            .scope_creator
            .as_mut()
            .map(|scope_creator| &mut **scope_creator as &mut dyn ScopeCreator);
        NodeTraversal::new(
            compiler,
            callback,
            scope_creator,
            self.obey_destructuring_and_default_value_execution_order,
        )
    }

    // port: NodeTraversal.Builder#traverse
    pub fn traverse(&mut self, root: NodeId) {
        self.build().traverse_tree(root);
    }

    // port: NodeTraversal.Builder#traverseAtScope
    pub fn traverse_at_scope(&mut self, scope: impl Into<AbstractScopeHandle>) {
        self.build().traverse_at_scope(scope);
    }

    // port: NodeTraversal.Builder#traverseRoots
    pub fn traverse_roots(&mut self, externs: NodeId, root: NodeId) {
        self.build().traverse_roots_tree(externs, root);
    }

    // port: NodeTraversal.Builder#traverseWithScope
    pub fn traverse_with_scope(&mut self, root: NodeId, s: impl Into<AbstractScopeHandle>) {
        self.build().traverse_with_scope(root, s);
    }
}

impl<'a> NodeTraversal<'a> {
    // port: NodeTraversal#NodeTraversal
    fn new(
        compiler: &'a mut AbstractCompiler,
        callback: &'a mut dyn Callback,
        scope_creator: Option<&'a mut dyn ScopeCreator>,
        obey_destructuring_and_default_value_execution_order: bool,
    ) -> Self {
        let scope_callback = callback.as_scoped_callback().is_some();
        let scope_creator = scope_creator.map_or_else(
            || ScopeCreatorHolder::Owned(Box::new(SyntacticScopeCreator::new())),
            ScopeCreatorHolder::Borrowed,
        );
        let may_contain_synthetic_blocks = compiler
            .get_options_opt()
            .is_none_or(|options| options.get_synthetic_block_start_marker().is_some());
        Self {
            compiler,
            callback: Some(callback),
            scope_callback,
            scope_creator,
            obey_destructuring_and_default_value_execution_order,
            may_contain_synthetic_blocks,
            current_node: None,
            current_hoist_scope_root: None,
            current_function: None,
            current_script: None,
            current_change_scope: None,
            scopes: Vec::new(),
            source_name: None,
            input_id: None,
            compiler_input: None,
        }
    }

    // port: NodeTraversal#throwUnexpectedException
    fn throw_unexpected_exception(&mut self, unexpected_exception: Box<dyn Any + Send>) -> ! {
        let cause = unexpected_exception
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                unexpected_exception
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
            })
            .unwrap_or_else(|| "null".to_string());
        let mut message = cause.clone();
        if self.current_script.is_some() {
            message = format!(
                "{}\n{}{}",
                cause,
                self.format_node_context("Node", self.current_node),
                match self.current_node {
                    Some(current_node) => {
                        self.format_node_context("Parent", current_node.get_parent(self))
                    }
                    None => String::new(),
                }
            );
        }
        self.compiler.throw_internal_error(&message, &cause);
    }

    // port: NodeTraversal#formatNodeContext
    fn format_node_context(&mut self, label: &str, n: Option<NodeId>) -> String {
        match n {
            None => format!("  {label}: NULL"),
            Some(n) => format!(
                "  {}({}): {}",
                label,
                n.to_string_with_options(self, false, false, false),
                self.format_node_position(n)
            ),
        }
    }

    // port: NodeTraversal#traverse(Node)
    pub fn traverse_tree(&mut self, root: NodeId) {
        let callback = check_not_null!(self.callback.take());
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.init_traversal(root);
            self.current_node = Some(root);
            self.push_scope(root, callback);
            self.traverse_branch(root, None, callback);
            self.pop_scope(callback);
        }));
        self.callback = Some(callback);
        if let Err(unexpected_exception) = result {
            self.throw_unexpected_exception(unexpected_exception);
        }
    }

    // port: NodeTraversal#traverse(AbstractCompiler, Node, Callback)
    pub fn traverse(compiler: &mut AbstractCompiler, root: NodeId, cb: &mut dyn Callback) {
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(cb)
            .traverse(root);
    }

    // port: NodeTraversal#traverseRoots(Node, Node)
    pub fn traverse_roots_tree(&mut self, externs: NodeId, root: NodeId) {
        let callback = check_not_null!(self.callback.take());
        let result = catch_unwind(AssertUnwindSafe(|| {
            let scope_root = check_not_null!(externs.get_parent(self));
            self.init_traversal(scope_root);
            self.current_node = Some(scope_root);
            self.push_scope(scope_root, callback);
            self.traverse_branch(externs, Some(scope_root), callback);
            check_state!(root.get_parent(self) == Some(scope_root));
            self.traverse_branch(root, Some(scope_root), callback);
            self.pop_scope(callback);
        }));
        self.callback = Some(callback);
        if let Err(unexpected_exception) = result {
            self.throw_unexpected_exception(unexpected_exception);
        }
    }

    // port: NodeTraversal#traverseRoots(AbstractCompiler, Callback, Node, Node)
    pub fn traverse_roots(
        compiler: &mut AbstractCompiler,
        cb: &mut dyn Callback,
        externs: NodeId,
        root: NodeId,
    ) {
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(cb)
            .traverse_roots(externs, root);
    }

    const MISSING_SOURCE: &'static str = "[source unknown]";

    // port: NodeTraversal#formatNodePosition
    fn format_node_position(&mut self, n: NodeId) -> String {
        let Some(source_file_name) = self.get_best_source_file_name(Some(n)) else {
            return format!("{}\n", Self::MISSING_SOURCE);
        };
        let line_number = n.get_lineno(self);
        let column_number = n.get_charno(self);
        let src = self
            .compiler
            .get_source_line(&source_file_name, line_number)
            .unwrap_or_else(|| Self::MISSING_SOURCE.to_string());
        format!("{source_file_name}:{line_number}:{column_number}\n{src}\n")
    }

    // port: NodeTraversal#traverseWithScope
    pub fn traverse_with_scope(&mut self, root: NodeId, s: impl Into<AbstractScopeHandle>) {
        let s = s.into();
        let callback = check_not_null!(self.callback.take());
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.traverse_with_scope_callback(root, s, callback);
        }));
        self.callback = Some(callback);
        // Rust-only restoration of the borrowed callback. Java's error
        // handling remains in traverse_with_scope_callback, and this wrapper
        // forwards its panic unchanged.
        if let Err(unexpected_exception) = result {
            resume_unwind(unexpected_exception);
        }
    }

    // Rust-only split that permits traverseAtScope to reuse Java's method while
    // the callback is already taken out of the traversal.
    fn traverse_with_scope_callback(
        &mut self,
        root: NodeId,
        s: AbstractScopeHandle,
        callback: &mut dyn Callback,
    ) {
        check_state!(
            s.is_global(self.compiler) || s.is_module_scope(self.compiler),
            "%s",
            s.to_string(self.compiler)
        );
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.init_traversal(root);
            self.current_node = Some(root);
            self.push_scope_existing(s, callback);
            self.traverse_branch(root, None, callback);
            self.pop_scope(callback);
        }));
        if let Err(unexpected_exception) = result {
            self.throw_unexpected_exception(unexpected_exception);
        }
    }

    // port: NodeTraversal#traverseAtScope
    pub fn traverse_at_scope(&mut self, s: impl Into<AbstractScopeHandle>) {
        let s = s.into();
        let callback = check_not_null!(self.callback.take());
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.traverse_at_scope_callback(s, callback);
        }));
        self.callback = Some(callback);
        // Java does not catch here. Preserve that boundary while restoring the
        // callback borrowed out of the traversal before the call.
        if let Err(unexpected_exception) = result {
            resume_unwind(unexpected_exception);
        }
    }

    // Rust-only split for borrowed callback dispatch.
    fn traverse_at_scope_callback(&mut self, s: AbstractScopeHandle, callback: &mut dyn Callback) {
        let n = s.get_root_node(self.compiler);
        self.init_traversal(n);
        self.current_node = Some(n);
        let mut parent_scopes = VecDeque::new();
        let mut temp = s.get_parent(self.compiler);
        while let Some(scope) = temp {
            parent_scopes.push_front(scope);
            temp = scope.get_parent(self.compiler);
        }
        while let Some(scope) = parent_scopes.pop_front() {
            self.push_scope_existing_quietly(scope, true, callback);
        }
        match n.get_token(self) {
            Token::FUNCTION => {
                if callback.should_traverse(self, n, None) {
                    self.push_scope_existing(s, callback);
                    let fn_name = n.get_first_child(self).unwrap();
                    let args = fn_name.get_next(self).unwrap();
                    let body = args.get_next(self).unwrap();
                    if !NodeUtil::is_function_declaration(self, n) {
                        self.traverse_branch(fn_name, Some(n), callback);
                    }
                    self.traverse_branch(args, Some(n), callback);
                    self.traverse_branch(body, Some(n), callback);
                    self.pop_scope(callback);
                    callback.visit(self, n, None);
                }
            }
            Token::CLASS => {
                if callback.should_traverse(self, n, None) {
                    self.push_scope_existing(s, callback);
                    let class_name = n.get_first_child(self).unwrap();
                    let body = n.get_last_child(self).unwrap();
                    if NodeUtil::is_class_expression(self, n) {
                        self.traverse_branch(class_name, Some(n), callback);
                    }
                    self.traverse_branch(body, Some(n), callback);
                    self.pop_scope(callback);
                    callback.visit(self, n, None);
                }
            }
            Token::BLOCK | Token::SWITCH_BODY => {
                if callback.should_traverse(self, n, None) {
                    self.push_scope_existing(s, callback);
                    self.traverse_children(n, callback);
                    self.pop_scope(callback);
                    callback.visit(self, n, None);
                }
            }
            Token::MEMBER_FIELD_DEF => {
                self.push_scope_existing(s, callback);
                if callback.should_traverse(self, n, None) {
                    self.traverse_children(n, callback);
                    callback.visit(self, n, None);
                }
                self.pop_scope(callback);
            }
            Token::COMPUTED_FIELD_DEF => {
                self.push_scope_existing(s, callback);
                if callback.should_traverse(self, n, None) {
                    self.traverse_branch(n.get_last_child(self).unwrap(), Some(n), callback);
                    callback.visit(self, n, None);
                }
                self.pop_scope(callback);
            }
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF | Token::FOR => {
                if callback.should_traverse(self, n, None) {
                    self.push_scope_existing(s, callback);
                    let for_assignment_param = n.get_first_child(self).unwrap();
                    let for_iterable_param = for_assignment_param.get_next(self).unwrap();
                    let for_body_scope = for_iterable_param.get_next(self).unwrap();
                    self.traverse_branch(for_assignment_param, Some(n), callback);
                    self.traverse_branch(for_iterable_param, Some(n), callback);
                    self.traverse_branch(for_body_scope, Some(n), callback);
                    self.pop_scope(callback);
                    callback.visit(self, n, None);
                }
            }
            _ => {
                check_state!(
                    s.is_global(self.compiler) || s.is_module_scope(self.compiler),
                    "Expected global or module scope. Got: (%s)",
                    s.to_string(self.compiler)
                );
                self.traverse_with_scope_callback(n, s, callback);
            }
        }
    }

    // port: NodeTraversal#traverseScopeRoots
    pub fn traverse_scope_roots(
        compiler: &mut AbstractCompiler,
        scope_nodes: &[NodeId],
        cb: &mut dyn Callback,
        traverse_nested: bool,
    ) {
        struct TraverseScopeRootsCallback<'a> {
            inside_scope_node: bool,
            scope_node: Option<NodeId>,
            cb: &'a mut dyn Callback,
            traverse_nested: bool,
        }
        impl Callback for TraverseScopeRootsCallback<'_> {
            // port: NodeTraversal.TraverseScopeRootsCallback#shouldTraverse
            fn should_traverse(
                &mut self,
                t: &mut NodeTraversal<'_>,
                n: NodeId,
                parent: Option<NodeId>,
            ) -> bool {
                if self.scope_node == Some(n) {
                    self.inside_scope_node = true;
                }
                (self.traverse_nested
                    || self.scope_node == Some(n)
                    || !ChangeTracker::is_change_scope_root(t, n))
                    && self.cb.should_traverse(t, n, parent)
            }

            // port: NodeTraversal.TraverseScopeRootsCallback#visit
            fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
                if self.scope_node == Some(n) {
                    self.inside_scope_node = false;
                }
                self.cb.visit(t, n, parent);
            }

            fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
                Some(self)
            }
        }
        impl ScopedCallback for TraverseScopeRootsCallback<'_> {
            // port: NodeTraversal.TraverseScopeRootsCallback#enterScope
            #[allow(clippy::collapsible_if)] // Retain Java control flow.
            fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
                if self.inside_scope_node {
                    if let Some(scoped_callback) = self.cb.as_scoped_callback() {
                        scoped_callback.enter_scope(t);
                    }
                }
            }

            // port: NodeTraversal.TraverseScopeRootsCallback#exitScope
            #[allow(clippy::collapsible_if)] // Retain Java control flow.
            fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
                if self.inside_scope_node {
                    if let Some(scoped_callback) = self.cb.as_scoped_callback() {
                        scoped_callback.exit_scope(t);
                    }
                }
            }
        }
        let mut scb = TraverseScopeRootsCallback {
            inside_scope_node: false,
            scope_node: None,
            cb,
            traverse_nested,
        };
        let mut scope_creator = MemoizedScopeCreator::new(Box::new(SyntacticScopeCreator::new()));
        for &scope_node in scope_nodes {
            scb.scope_node = Some(scope_node);
            NodeTraversal::builder()
                .set_compiler(compiler)
                .set_callback(&mut scb)
                .set_scope_creator(&mut scope_creator)
                .build()
                .traverse_scope_root(scope_node);
        }
    }

    // port: NodeTraversal#traverseScopeRoot
    fn traverse_scope_root(&mut self, scope_root: NodeId) {
        let callback = check_not_null!(self.callback.take());
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.init_traversal(scope_root);
            self.current_node = Some(scope_root);
            self.init_scope_roots(scope_root.get_parent(self), callback);
            self.traverse_branch(scope_root, scope_root.get_parent(self), callback);
        }));
        self.callback = Some(callback);
        if let Err(unexpected_exception) = result {
            self.throw_unexpected_exception(unexpected_exception);
        }
    }

    // port: NodeTraversal#getCompiler
    pub fn get_compiler(&mut self) -> &mut AbstractCompiler {
        self.compiler
    }

    // port: NodeTraversal#getSourceName
    pub fn get_source_name(&mut self) -> Option<String> {
        if self.source_name.is_none() {
            self.source_name = self
                .current_script
                .map_or_else(|| Some(String::new()), |n| n.get_source_file_name(self));
        }
        self.source_name.clone()
    }

    // port: NodeTraversal#getInput
    #[allow(clippy::collapsible_if)] // Retain Java control flow.
    pub fn get_input(&mut self) -> Option<&CompilerInput> {
        let input_id = self.get_input_id();
        if self.compiler_input.is_none() {
            if let Some(input_id) = input_id {
                if self.compiler.get_input(&input_id).is_some() {
                    self.compiler_input = Some(input_id);
                }
            }
        }
        self.compiler_input
            .as_ref()
            .and_then(|input_id| self.compiler.get_input(input_id))
    }

    // port: NodeTraversal#getChunk
    pub fn get_chunk(&mut self) -> Option<JSChunk> {
        self.get_input().and_then(|input| input.get_chunk())
    }

    // port: NodeTraversal#getCurrentNode
    pub fn get_current_node(&self) -> Option<NodeId> {
        self.current_node
    }

    // port: NodeTraversal#handleScript
    fn handle_script(&mut self, n: NodeId, parent: Option<NodeId>, callback: &mut dyn Callback) {
        if Platform::is_thread_interrupted() {
            panic!("java.lang.InterruptedException");
        }
        self.set_change_scope(Some(n));
        self.current_node = Some(n);
        self.current_script = Some(n);
        self.clear_script_state();
        if callback.should_traverse(self, n, parent) {
            self.traverse_children(n, callback);
            self.current_node = Some(n);
            callback.visit(self, n, parent);
        }
        self.set_change_scope(None);
    }

    // port: NodeTraversal#handleFunction
    fn handle_function(&mut self, n: NodeId, parent: Option<NodeId>, callback: &mut dyn Callback) {
        let change_scope = self.current_change_scope;
        self.set_change_scope(Some(n));
        self.current_node = Some(n);
        if callback.should_traverse(self, n, parent) {
            self.traverse_function(n, parent, callback);
            self.current_node = Some(n);
            callback.visit(self, n, parent);
        }
        self.set_change_scope(change_scope);
    }

    // port: NodeTraversal#handleModule
    fn handle_module(&mut self, n: NodeId, parent: Option<NodeId>, callback: &mut dyn Callback) {
        self.current_hoist_scope_root = Some(n);
        self.push_scope(n, callback);
        self.current_node = Some(n);
        if callback.should_traverse(self, n, parent) {
            self.traverse_children(n, callback);
            self.current_node = Some(n);
            callback.visit(self, n, parent);
        }
        self.pop_scope(callback);
        self.current_hoist_scope_root = None;
    }

    // port: NodeTraversal#handleDestructuringOrDefaultValue
    fn handle_destructuring_or_default_value(
        &mut self,
        n: NodeId,
        parent: Option<NodeId>,
        callback: &mut dyn Callback,
    ) {
        self.current_node = Some(n);
        if callback.should_traverse(self, n, parent) {
            let first = n.get_first_child(self).unwrap();
            let second = first.get_next(self);
            if let Some(second) = second {
                check_state!(
                    second.get_next(self).is_none(),
                    "%s",
                    second.to_string(self)
                );
                self.traverse_branch(second, Some(n), callback);
            }
            self.traverse_branch(first, Some(n), callback);
            self.current_node = Some(n);
            callback.visit(self, n, parent);
        }
    }

    // port: NodeTraversal#createsBlockScope
    fn creates_block_scope(&self, n: NodeId) -> bool {
        match n.get_token(self) {
            Token::BLOCK => {
                if self.may_contain_synthetic_blocks && n.is_synthetic_block(self) {
                    return false;
                }
                n.get_parent(self).is_some_and(|parent| {
                    !NodeUtil::is_switch_case(self, parent) && !parent.is_catch(self)
                })
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY
            | Token::CLASS => true,
            _ => false,
        }
    }

    // port: NodeTraversal#traverseBranch
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn traverse_branch(&mut self, n: NodeId, parent: Option<NodeId>, callback: &mut dyn Callback) {
        match n.get_token(self) {
            Token::SCRIPT => {
                self.handle_script(n, parent, callback);
                return;
            }
            Token::FUNCTION => {
                self.handle_function(n, parent, callback);
                return;
            }
            Token::MODULE_BODY => {
                self.handle_module(n, parent, callback);
                return;
            }
            Token::CLASS => {
                self.handle_class(n, parent, callback);
                return;
            }
            Token::CLASS_MEMBERS => {
                self.handle_class_members(n, parent, callback);
                return;
            }
            Token::DEFAULT_VALUE | Token::DESTRUCTURING_LHS => {
                if self.obey_destructuring_and_default_value_execution_order {
                    self.handle_destructuring_or_default_value(n, parent, callback);
                    return;
                }
            }
            _ => {}
        }
        self.current_node = Some(n);
        if !callback.should_traverse(self, n, parent) {
            return;
        }
        let creates_block_scope = self.creates_block_scope(n);
        let previous_hoist_scope_root = self.current_hoist_scope_root;
        if creates_block_scope {
            self.push_scope(n, callback);
            if NodeUtil::is_class_static_block(self, n) {
                self.current_hoist_scope_root = Some(n);
            }
        }
        // Intentionally inlined, as in Java, to avoid doubling recursion depth.
        let mut child = n.get_first_child(self);
        while let Some(c) = child {
            let next = c.get_next(self);
            self.traverse_branch(c, Some(n), callback);
            child = next;
        }
        if creates_block_scope {
            self.pop_scope(callback);
            self.current_hoist_scope_root = previous_hoist_scope_root;
        }
        self.current_node = Some(n);
        callback.visit(self, n, parent);
    }

    // port: NodeTraversal#traverseFunction
    fn traverse_function(
        &mut self,
        n: NodeId,
        parent: Option<NodeId>,
        callback: &mut dyn Callback,
    ) {
        let fn_name = n.get_first_child(self).unwrap();
        let is_function_declaration =
            parent.is_some() && NodeUtil::is_function_declaration(self, n);
        if is_function_declaration {
            self.traverse_branch(fn_name, Some(n), callback);
        }
        self.current_node = Some(n);
        let previous_hoist_scope_root = self.current_hoist_scope_root;
        self.current_hoist_scope_root = Some(n);
        let previous_function = self.current_function;
        self.current_function = Some(n);
        self.push_scope(n, callback);
        if !is_function_declaration {
            self.traverse_branch(fn_name, Some(n), callback);
        }
        let args = fn_name.get_next(self).unwrap();
        let body = args.get_next(self).unwrap();
        self.traverse_branch(args, Some(n), callback);
        self.traverse_branch(body, Some(n), callback);
        self.pop_scope(callback);
        self.current_function = previous_function;
        self.current_hoist_scope_root = previous_hoist_scope_root;
    }

    // port: NodeTraversal#handleClass
    fn handle_class(&mut self, n: NodeId, parent: Option<NodeId>, callback: &mut dyn Callback) {
        self.current_node = Some(n);
        if !callback.should_traverse(self, n, parent) {
            return;
        }
        let class_name = n.get_first_child(self).unwrap();
        let extends_clause = class_name.get_next(self).unwrap();
        let body = extends_clause.get_next(self).unwrap();
        let is_class_expression = NodeUtil::is_class_expression(self, n);
        self.traverse_branch(extends_clause, Some(n), callback);
        if !is_class_expression {
            self.traverse_branch(class_name, Some(n), callback);
        }
        self.current_node = Some(n);
        self.push_scope(n, callback);
        if is_class_expression {
            self.traverse_branch(class_name, Some(n), callback);
        }
        self.traverse_branch(body, Some(n), callback);
        self.pop_scope(callback);
        self.current_node = Some(n);
        callback.visit(self, n, parent);
    }

    // port: NodeTraversal#handleClassMembers
    fn handle_class_members(
        &mut self,
        n: NodeId,
        parent: Option<NodeId>,
        callback: &mut dyn Callback,
    ) {
        self.current_node = Some(n);
        if !callback.should_traverse(self, n, parent) {
            return;
        }
        let mut child = n.get_first_child(self);
        while let Some(c) = child {
            let next = c.get_next(self);
            if c.is_computed_prop(self) || c.is_computed_field_def(self) {
                self.traverse_branch(c.get_first_child(self).unwrap(), Some(c), callback);
            }
            child = next;
        }
        let mut child = n.get_first_child(self);
        while let Some(c) = child {
            let next = c.get_next(self);
            match c.get_token(self) {
                Token::COMPUTED_PROP => {
                    self.current_node = Some(n);
                    if callback.should_traverse(self, c, Some(n)) {
                        self.traverse_branch(c.get_last_child(self).unwrap(), Some(c), callback);
                        self.current_node = Some(n);
                        callback.visit(self, c, Some(n));
                    }
                }
                Token::COMPUTED_FIELD_DEF => {
                    self.current_node = Some(n);
                    let previous_hoist_scope_root = self.current_hoist_scope_root;
                    self.current_hoist_scope_root = Some(n);
                    self.push_scope(c, callback);
                    if callback.should_traverse(self, c, Some(n)) {
                        if c.has_two_children(self) {
                            self.traverse_branch(
                                c.get_last_child(self).unwrap(),
                                Some(c),
                                callback,
                            );
                        }
                        self.current_node = Some(n);
                        callback.visit(self, c, Some(n));
                    }
                    self.pop_scope(callback);
                    self.current_hoist_scope_root = previous_hoist_scope_root;
                }
                Token::MEMBER_FIELD_DEF => self.handle_member_field_def(n, c, callback),
                Token::BLOCK
                | Token::MEMBER_FUNCTION_DEF
                | Token::MEMBER_VARIABLE_DEF
                | Token::GETTER_DEF
                | Token::SETTER_DEF => self.traverse_branch(c, Some(n), callback),
                _ => panic!("Invalid class member: {}", c.get_token(self)),
            }
            child = next;
        }
        self.current_node = Some(n);
        callback.visit(self, n, parent);
    }

    // port: NodeTraversal#handleMemberFieldDef
    fn handle_member_field_def(&mut self, n: NodeId, child: NodeId, callback: &mut dyn Callback) {
        let previous_hoist_scope_root = self.current_hoist_scope_root;
        self.current_hoist_scope_root = Some(n);
        self.push_scope(child, callback);
        self.traverse_branch(child, Some(n), callback);
        self.pop_scope(callback);
        self.current_hoist_scope_root = previous_hoist_scope_root;
    }

    // port: NodeTraversal#traverseChildren
    fn traverse_children(&mut self, n: NodeId, callback: &mut dyn Callback) {
        let mut child = n.get_first_child(self);
        while let Some(c) = child {
            let next = c.get_next(self);
            self.traverse_branch(c, Some(n), callback);
            child = next;
        }
    }

    // port: NodeTraversal#getEnclosingFunction
    pub fn get_enclosing_function(&self) -> Option<NodeId> {
        self.current_function
    }

    // port: NodeTraversal#pushScope(Node)
    fn push_scope(&mut self, node: NodeId, callback: &mut dyn Callback) {
        check_not_null!(self.current_node);
        self.scopes.push(ScopeObject::Node(node));
        if self.scope_callback {
            check_not_null!(callback.as_scoped_callback()).enter_scope(self);
        }
    }

    // port: NodeTraversal#pushScope(AbstractScope)
    fn push_scope_existing(&mut self, s: AbstractScopeHandle, callback: &mut dyn Callback) {
        self.push_scope_existing_quietly(s, false, callback);
    }

    // port: NodeTraversal#pushScope(AbstractScope, boolean)
    fn push_scope_existing_quietly(
        &mut self,
        s: AbstractScopeHandle,
        quietly: bool,
        callback: &mut dyn Callback,
    ) {
        check_not_null!(self.current_node);
        self.scopes.push(ScopeObject::Scope(s));
        if !quietly && self.scope_callback {
            check_not_null!(callback.as_scoped_callback()).enter_scope(self);
        }
    }

    // port: NodeTraversal#popScope()
    fn pop_scope(&mut self, callback: &mut dyn Callback) {
        self.pop_scope_quietly(false, callback);
    }

    // port: NodeTraversal#popScope(boolean)
    fn pop_scope_quietly(&mut self, quietly: bool, callback: &mut dyn Callback) {
        if !quietly && self.scope_callback {
            check_not_null!(callback.as_scoped_callback()).exit_scope(self);
        }
        self.scopes.remove(self.scopes.len() - 1);
    }

    // port: NodeTraversal#getNodeRootFromScopeObj
    fn get_node_root_from_scope_obj(&self, root: ScopeObject) -> NodeId {
        match root {
            ScopeObject::Node(node) => node,
            ScopeObject::Scope(scope) => scope.get_root_node(self.compiler),
        }
    }

    // port: NodeTraversal#getScopeRoot
    pub fn get_scope_root(&self) -> Option<NodeId> {
        self.scopes
            .last()
            .map(|root| self.get_node_root_from_scope_obj(*root))
    }

    // port: NodeTraversal#getScopeDepth
    pub fn get_scope_depth(&self) -> i32 {
        let depth = self.scopes.len();
        check_state!(depth > 0);
        depth as i32 - 1
    }

    // port: NodeTraversal#getScope
    pub fn get_scope(&mut self) -> ScopeId {
        let scope = self.get_abstract_scope();
        scope.untyped(self.compiler)
    }

    // port: NodeTraversal#getTypedScope
    pub fn get_typed_scope(&mut self) -> crate::typed_scope::TypedScope {
        let scope = self.get_abstract_scope();
        scope.typed(self.compiler)
    }

    // port: NodeTraversal#getAbstractScope()
    pub fn get_abstract_scope(&mut self) -> AbstractScopeHandle {
        self.get_abstract_scope_at_depth(self.scopes.len() - 1)
    }

    // port: NodeTraversal#getAbstractScope(int)
    fn get_abstract_scope_at_depth(&mut self, root_depth: usize) -> AbstractScopeHandle {
        match self.scopes[root_depth] {
            ScopeObject::Node(node) => {
                let parent_scope = if root_depth > 0 {
                    Some(self.get_abstract_scope_at_depth(root_depth - 1))
                } else {
                    None
                };
                let scope = self.scope_creator.get_mut().create_abstract_scope(
                    self.compiler,
                    node,
                    parent_scope,
                );
                self.scopes[root_depth] = ScopeObject::Scope(scope);
                scope
            }
            ScopeObject::Scope(scope) => scope,
        }
    }

    // port: NodeTraversal#isHoistScope
    pub fn is_hoist_scope(&self) -> bool {
        Self::is_hoist_scope_root_node(self, check_not_null!(self.get_scope_root()))
    }

    // port: NodeTraversal#getClosestHoistScopeRoot
    pub fn get_closest_hoist_scope_root(&self) -> Option<NodeId> {
        for root in self.scopes.iter().rev() {
            let root_node = self.get_node_root_from_scope_obj(*root);
            if Self::is_hoist_scope_root_node(self, root_node) {
                return Some(root_node);
            }
        }
        None
    }

    // port: NodeTraversal#getClosestContainerScope
    pub fn get_closest_container_scope(&mut self) -> Option<AbstractScopeHandle> {
        for i in (0..self.scopes.len()).rev() {
            let root_node = self.get_node_root_from_scope_obj(self.scopes[i]);
            if !NodeUtil::creates_block_scope(self, root_node) {
                return Some(self.get_abstract_scope_at_depth(i));
            }
        }
        None
    }

    // port: NodeTraversal#getClosestHoistScope
    pub fn get_closest_hoist_scope(&mut self) -> Option<AbstractScopeHandle> {
        for i in (0..self.scopes.len()).rev() {
            let root_node = self.get_node_root_from_scope_obj(self.scopes[i]);
            if Self::is_hoist_scope_root_node(self, root_node) {
                return Some(self.get_abstract_scope_at_depth(i));
            }
        }
        None
    }

    // port: NodeTraversal#isHoistScopeRootNode
    fn is_hoist_scope_root_node(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::FUNCTION | Token::MODULE_BODY | Token::ROOT | Token::SCRIPT => true,
            _ => NodeUtil::is_function_block(ast, n),
        }
    }

    // port: NodeTraversal#getClosestScopeRootNodeBindingThisOrSuper
    pub fn get_closest_scope_root_node_binding_this_or_super(&self) -> Option<NodeId> {
        for root in self.scopes.iter().rev() {
            let root_node = self.get_node_root_from_scope_obj(*root);
            match root_node.get_token(self) {
                Token::FUNCTION => {
                    if root_node.is_arrow_function(self) {
                        continue;
                    }
                    return Some(root_node);
                }
                Token::MEMBER_FIELD_DEF
                | Token::COMPUTED_FIELD_DEF
                | Token::CLASS
                | Token::MODULE_BODY
                | Token::ROOT => return Some(root_node),
                Token::BLOCK => {
                    if NodeUtil::is_class_static_block(self, root_node) {
                        return Some(root_node);
                    }
                    continue;
                }
                _ => continue,
            }
        }
        None
    }

    // port: NodeTraversal#getScopeCreator
    pub fn get_scope_creator(&mut self) -> &mut dyn ScopeCreator {
        self.scope_creator.get_mut()
    }

    // Rust-only split: a callback that starts a nested traversal from inside this one (Java's
    // ReferenceCollector#maybeJumpToHoistedFunction) uses the compiler and the scope creator
    // together, as Java shares both objects.
    pub fn get_compiler_and_scope_creator(
        &mut self,
    ) -> (&mut AbstractCompiler, &mut dyn ScopeCreator) {
        (&mut *self.compiler, self.scope_creator.get_mut())
    }

    // port: NodeTraversal#inGlobalScope
    pub fn in_global_scope(&self) -> bool {
        self.get_scope_depth() == 0
    }

    // port: NodeTraversal#inModuleScope
    pub fn in_module_scope(&self) -> bool {
        NodeUtil::is_module_scope_root(self, check_not_null!(self.get_scope_root()))
    }

    // port: NodeTraversal#inGlobalOrModuleScope
    pub fn in_global_or_module_scope(&self) -> bool {
        self.in_global_scope() || self.in_module_scope()
    }

    // port: NodeTraversal#inFunctionBlockScope
    pub fn in_function_block_scope(&self) -> bool {
        NodeUtil::is_function_block(self, check_not_null!(self.get_scope_root()))
    }

    // port: NodeTraversal#inGlobalHoistScope
    pub fn in_global_hoist_scope(&self) -> bool {
        self.current_hoist_scope_root.is_none()
    }

    // port: NodeTraversal#inModuleHoistScope
    pub fn in_module_hoist_scope(&self) -> bool {
        let Some(mut module_root) = self.current_hoist_scope_root else {
            return false;
        };
        if module_root.is_function(self) {
            module_root = module_root.get_last_child(self).unwrap();
        }
        NodeUtil::is_module_scope_root(self, module_root)
    }

    // port: NodeTraversal#report(Node, DiagnosticType, String...)
    pub fn report(
        &mut self,
        n: NodeId,
        diagnostic_type: &'static DiagnosticType,
        arguments: &[&str],
    ) {
        let error = JSError::make(self, n, diagnostic_type, arguments);
        self.compiler.report(error);
    }

    // port: NodeTraversal#report(Node, Node, DiagnosticType, String...)
    pub fn report_with_range(
        &mut self,
        start: NodeId,
        end: NodeId,
        diagnostic_type: &'static DiagnosticType,
        arguments: &[&str],
    ) {
        let error = JSError::make_with_node_range(self, start, end, diagnostic_type, arguments);
        self.compiler.report(error);
    }

    // port: NodeTraversal#reportCodeChange()
    pub fn report_code_change(&mut self) {
        let change_scope = check_not_null!(self.current_change_scope);
        check_state!(
            ChangeTracker::is_change_scope_root(self, change_scope),
            "%s",
            change_scope.to_string(self)
        );
        self.compiler.report_change_to_change_scope(change_scope);
    }

    // port: NodeTraversal#reportCodeChange(Node)
    pub fn report_code_change_at_node(&mut self, n: NodeId) {
        self.compiler.report_change_to_enclosing_scope(n);
    }

    // port: NodeTraversal#getCurrentScript
    pub fn get_current_script(&self) -> Option<NodeId> {
        if self.current_script.is_none() {
            panic!("getCurrentScript not supported");
        }
        self.current_script
    }

    // port: NodeTraversal#setChangeScope
    fn set_change_scope(&mut self, n: Option<NodeId>) {
        self.current_change_scope = n;
    }

    // port: NodeTraversal#getEnclosingScript
    fn get_enclosing_script(&self, mut n: Option<NodeId>) -> Option<NodeId> {
        while let Some(node) = n {
            if node.is_script(self) {
                break;
            }
            n = node.get_parent(self);
        }
        n
    }

    // port: NodeTraversal#initTraversal
    fn init_traversal(&mut self, traversal_root: NodeId) {
        if Platform::is_thread_interrupted() {
            panic!("java.lang.InterruptedException");
        }
        let hoist_scope_root = NodeUtil::get_enclosing_hoist_scope_root(self, traversal_root);
        self.current_hoist_scope_root = hoist_scope_root;
        let change_scope = ChangeTracker::get_enclosing_change_scope_root(
            self,
            Some(hoist_scope_root.unwrap_or(traversal_root)),
        );
        self.set_change_scope(change_scope);
        let enclosing_function =
            hoist_scope_root.and_then(|root| NodeUtil::get_enclosing_function(self, root));
        self.current_function = enclosing_function;
        self.current_script = self.get_enclosing_script(change_scope);
        self.clear_script_state();
    }

    // port: NodeTraversal#initScopeRoots
    fn init_scope_roots(&mut self, mut n: Option<NodeId>, callback: &mut dyn Callback) {
        let mut queued_scope_roots = VecDeque::new();
        while let Some(node) = n {
            if self.is_scope_root(node) {
                queued_scope_roots.push_front(node);
            }
            n = node.get_parent(self);
        }
        for queued_scope_root in queued_scope_roots {
            self.push_scope(queued_scope_root, callback);
        }
    }

    // port: NodeTraversal#isScopeRoot
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn is_scope_root(&self, n: NodeId) -> bool {
        if n.is_root(self) && n.get_parent(self).is_none() {
            return true;
        } else if n.is_function(self) {
            return true;
        } else if NodeUtil::creates_block_scope(self, n) {
            return true;
        }
        false
    }

    // port: NodeTraversal#clearScriptState
    fn clear_script_state(&mut self) {
        self.input_id = None;
        self.source_name = None;
        self.compiler_input = None;
    }

    // port: NodeTraversal#getInputId
    #[allow(clippy::unnecessary_unwrap)] // Retain Java control flow.
    pub fn get_input_id(&mut self) -> Option<Arc<InputId>> {
        if self.current_script.is_some() && self.input_id.is_none() {
            self.input_id = self.current_script.unwrap().get_input_id(self);
        }
        self.input_id.clone()
    }

    // port: NodeTraversal#getBestSourceFileName
    fn get_best_source_file_name(&mut self, n: Option<NodeId>) -> Option<String> {
        match n {
            None => self.get_source_name(),
            Some(n) => n.get_source_file_name(self),
        }
    }
}

impl Deref for NodeTraversal<'_> {
    type Target = Ast;
    fn deref(&self) -> &Ast {
        self.compiler
    }
}
impl DerefMut for NodeTraversal<'_> {
    fn deref_mut(&mut self) -> &mut Ast {
        self.compiler
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        check_level::CheckLevel, compiler::Compiler, error_handler::ErrorHandler,
        error_manager::ErrorManager, sorting_error_manager::SortingErrorManager,
    };
    use std::sync::Mutex;

    struct RecordingErrorManager {
        errors: Arc<Mutex<Vec<JSError>>>,
        sorting_manager: SortingErrorManager,
    }
    impl ErrorHandler for RecordingErrorManager {
        fn report(&mut self, _level: CheckLevel, error: JSError) {
            self.errors.lock().unwrap().push(error);
        }
    }
    impl ErrorManager for RecordingErrorManager {
        fn generate_report(&mut self, _ast: &Ast) {}
        fn get_error_count(&self) -> i32 {
            self.sorting_manager.get_error_count()
        }
        fn get_warning_count(&self) -> i32 {
            self.sorting_manager.get_warning_count()
        }
        fn get_errors(&self) -> Vec<JSError> {
            self.sorting_manager.get_errors()
        }
        fn get_warnings(&self) -> Vec<JSError> {
            self.sorting_manager.get_warnings()
        }
        fn set_typed_percent(&mut self, value: f64) {
            self.sorting_manager.set_typed_percent(value);
        }
        fn get_typed_percent(&self) -> f64 {
            self.sorting_manager.get_typed_percent()
        }
    }

    // port: NodeTraversalTest#testReport
    #[test]
    fn test_report() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        static DT: DiagnosticType = DiagnosticType::warning("FOO", "{0}, {1} - {2}");

        struct TestCallback;
        impl Callback for TestCallback {
            fn should_traverse(
                &mut self,
                t: &mut NodeTraversal<'_>,
                n: NodeId,
                _parent: Option<NodeId>,
            ) -> bool {
                t.report(n, &DT, &["Foo", "Bar", "Hello"]);
                false
            }

            fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {
                panic!("AssertionError");
            }
        }
        let mut compiler = Compiler::new_with_error_manager(Box::new(RecordingErrorManager {
            errors: errors.clone(),
            sorting_manager: SortingErrorManager::new(Vec::new()),
        }));
        compiler.init_compiler_options_if_testing();
        let empty = compiler.new_node(Token::EMPTY);
        let mut callback = TestCallback;
        NodeTraversal::builder()
            .set_compiler(&mut compiler)
            .set_callback(&mut callback)
            .traverse(empty);

        let errors = errors.lock().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].description(), "Foo, Bar - Hello");
    }
}
