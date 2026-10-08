/*
 * Copyright 2026 The closure-rs Authors.
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

//! Rust-only handle for the `StaticScope` that `Compiler#getTranspilationNamespace()` returns
//! (the compiler-owned `GlobalNamespace`), so transpilation passes can pass it to
//! `AstFactory#createQName(StaticScope, ...)` while the factory borrows the compiler.

use crate::{AbstractCompiler, ast_factory::AstFactoryStaticScope};
use closure_rhino::{js_string::JsString, node::NodeId};

#[derive(Clone, Copy, Debug)]
pub struct TranspilationNamespace;

impl TranspilationNamespace {
    /// `compiler.getTranspilationNamespace()`: builds and caches the namespace on first use.
    pub fn get(compiler: &mut AbstractCompiler) -> Self {
        compiler.get_transpilation_namespace();
        Self
    }
}

impl AstFactoryStaticScope<AbstractCompiler> for TranspilationNamespace {
    // `scope.getSlot(name)` then `getDeclaration().getNode()` on the compiler's GlobalNamespace.
    fn get_slot_declaration_node(
        &self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
    ) -> Option<Option<NodeId>> {
        let mut namespace = compiler.take_transpilation_namespace();
        let slot = namespace.get_slot(compiler, name);
        let result = slot.map(|slot| {
            slot.get_declaration(&namespace)
                .expect("NullPointerException: getDeclaration()")
                .get_node(&namespace)
        });
        compiler.restore_transpilation_namespace(namespace);
        result
    }
}
