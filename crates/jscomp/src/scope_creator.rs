/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ScopeCreator.java.

use crate::{
    abstract_compiler::AbstractCompiler, abstract_scope::AbstractScopeHandle, scope::ScopeId,
};
use closure_rhino::node::NodeId;

pub trait ScopeCreator {
    // port: ScopeCreator#createScope
    /// Syntactic creators implement this half of Java's method (their scopes are `ScopeId`s);
    /// a typed creator implements it as `create_abstract_scope(..).untyped(..)`, which throws like
    /// Java's `TypedScope#untyped`.
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId;

    // port: ScopeCreator#createScope
    /// Java's signature (`AbstractScope<?, ?>` in and out), which NodeTraversal calls; a typed
    /// creator (TypedScopeCreator) overrides it to return its `TypedScope`.
    fn create_abstract_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<AbstractScopeHandle>,
    ) -> AbstractScopeHandle {
        let parent = parent.map(|parent| parent.untyped(compiler));
        AbstractScopeHandle::Untyped(self.create_scope(compiler, n, parent))
    }
}

pub struct AssertNoScopesCreated;

pub const ASSERT_NO_SCOPES_CREATED: AssertNoScopesCreated = AssertNoScopesCreated;

impl ScopeCreator for AssertNoScopesCreated {
    // port: ScopeCreator.ASSERT_NO_SCOPES_CREATED#createScope
    fn create_scope(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _n: NodeId,
        _parent: Option<ScopeId>,
    ) -> ScopeId {
        panic!("ScopeCreator.ASSERT_NO_SCOPES_CREATED cannot create child scopes.")
    }
}

// Rust-only forwarding permits Java interface ownership and borrowed creator arguments.
impl<T: ScopeCreator + ?Sized> ScopeCreator for Box<T> {
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        (**self).create_scope(compiler, n, parent)
    }
    fn create_abstract_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<AbstractScopeHandle>,
    ) -> AbstractScopeHandle {
        (**self).create_abstract_scope(compiler, n, parent)
    }
}

impl<T: ScopeCreator + ?Sized> ScopeCreator for &mut T {
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        (**self).create_scope(compiler, n, parent)
    }
    fn create_abstract_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<AbstractScopeHandle>,
    ) -> AbstractScopeHandle {
        (**self).create_abstract_scope(compiler, n, parent)
    }
}
