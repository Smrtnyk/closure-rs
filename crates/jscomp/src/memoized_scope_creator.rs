/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/MemoizedScopeCreator.java.

use crate::{abstract_compiler::AbstractCompiler, scope::ScopeId, scope_creator::ScopeCreator};
use closure_rhino::{check_state, node::NodeId};
use indexmap::IndexMap;

pub struct MemoizedScopeCreator<'a> {
    scopes_by_scope_root: IndexMap<NodeId, ScopeId>,
    delegate: Box<dyn ScopeCreator + 'a>,
}

impl<'a> MemoizedScopeCreator<'a> {
    // port: MemoizedScopeCreator#MemoizedScopeCreator
    pub fn new(delegate: Box<dyn ScopeCreator + 'a>) -> Self {
        Self {
            scopes_by_scope_root: IndexMap::new(),
            delegate,
        }
    }

    // port: MemoizedScopeCreator#createScope
    pub fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        match self.scopes_by_scope_root.get(&n).copied() {
            None => {
                let scope = self.delegate.create_scope(compiler, n, parent);
                self.scopes_by_scope_root.insert(n, scope);
                scope
            }
            Some(scope) => {
                check_state!(parent == scope.get_parent(compiler));
                scope
            }
        }
    }
}

impl ScopeCreator for MemoizedScopeCreator<'_> {
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        MemoizedScopeCreator::create_scope(self, compiler, n, parent)
    }
}
