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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ScopedName.java.

use crate::abstract_compiler::AbstractCompiler;
use closure_rhino::{check_not_null, js_string::JsString, node::NodeId};
use std::hash::{Hash, Hasher};

pub trait ScopedName {
    // port: ScopedName#getName
    fn get_name(&self, compiler: &AbstractCompiler) -> JsString;

    // port: ScopedName#getScopeRoot
    fn get_scope_root(&self, compiler: &AbstractCompiler) -> Option<NodeId>;
}

impl dyn ScopedName + '_ {
    // port: ScopedName#of
    pub fn of(name: impl Into<JsString>, scope_root: Option<NodeId>) -> Simple {
        Simple::new(name.into(), scope_root)
    }

    /// `ScopedName.of(name, scopeRoot)` with a possibly-null Java `name` (ModuleImportResolver
    /// passes `binding.boundName()`, which is null for a name imported from a SCRIPT).
    // port: ScopedName#of
    pub fn of_nullable(name: Option<JsString>, scope_root: Option<NodeId>) -> Simple {
        Simple { name, scope_root }
    }
}

// Rust-only: Eq/Hash over the same fields as Java's ScopedName#equals/#hashCode (name, root
// identity), so Simple keys hash containers (TypedScopeCreator's escaped/assigned var names).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Simple {
    // Java's `final String name` may be null (see `ScopedName::of_nullable`).
    name: Option<JsString>,
    scope_root: Option<NodeId>,
}

impl Simple {
    // port: ScopedName.Simple#Simple
    pub fn new(name: JsString, scope_root: Option<NodeId>) -> Self {
        Self {
            name: Some(name),
            scope_root,
        }
    }
}

impl ScopedName for Simple {
    // port: ScopedName.Simple#getName
    // Java returns the null name; every Java caller dereferences it (none is reached for the
    // null-name SCRIPT import, whose scope root is null), so a null name panics here as the NPE.
    fn get_name(&self, _compiler: &AbstractCompiler) -> JsString {
        self.name
            .clone()
            .expect("NullPointerException: ScopedName name is null")
    }

    // port: ScopedName.Simple#getScopeRoot
    fn get_scope_root(&self, _compiler: &AbstractCompiler) -> Option<NodeId> {
        self.scope_root
    }
}

impl dyn ScopedName + '_ {
    // port: ScopedName#equals
    pub fn equals(&self, compiler: &AbstractCompiler, other: Option<&dyn ScopedName>) -> bool {
        let Some(scoped_name) = other else {
            return false;
        };
        self.get_name(compiler) == scoped_name.get_name(compiler)
            && Some(check_not_null!(self.get_scope_root(compiler)))
                == scoped_name.get_scope_root(compiler)
    }

    // port: ScopedName#hashCode
    pub fn hash_code(&self, compiler: &AbstractCompiler) -> i32 {
        let name = self.get_name(compiler);
        let root = check_not_null!(self.get_scope_root(compiler));
        let mut result = 1_i32;
        result = result.wrapping_mul(31).wrapping_add(name.hash_code());
        result = result
            .wrapping_mul(31)
            .wrapping_add(node_identity_hash(root));
        result
    }
}

impl Simple {
    pub fn get_name(&self, compiler: &AbstractCompiler) -> JsString {
        <Self as ScopedName>::get_name(self, compiler)
    }

    pub fn get_scope_root(&self, compiler: &AbstractCompiler) -> Option<NodeId> {
        <Self as ScopedName>::get_scope_root(self, compiler)
    }

    pub fn equals(&self, compiler: &AbstractCompiler, other: Option<&dyn ScopedName>) -> bool {
        (self as &dyn ScopedName).equals(compiler, other)
    }

    pub fn hash_code(&self, compiler: &AbstractCompiler) -> i32 {
        (self as &dyn ScopedName).hash_code(compiler)
    }
}

// Rust-only bridge for Object.hashCode: NodeId's arena identity replaces the opaque Java
// identity hash, retaining equality and the wrapping ScopedName hash formula.
fn node_identity_hash(root: NodeId) -> i32 {
    #[derive(Default)]
    struct IdentityHasher(u32);

    impl Hasher for IdentityHasher {
        fn finish(&self) -> u64 {
            u64::from(self.0)
        }

        fn write(&mut self, bytes: &[u8]) {
            self.0 = u32::from_ne_bytes(bytes.try_into().unwrap());
        }

        fn write_u32(&mut self, value: u32) {
            self.0 = value;
        }
    }

    let mut hasher = IdentityHasher::default();
    root.hash(&mut hasher);
    hasher.finish() as i32
}
