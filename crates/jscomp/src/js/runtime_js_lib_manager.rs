/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/js/RuntimeJsLibManager.java.

//! Port of `js/RuntimeJsLibManager.java`: injects runtime libraries from the jscomp/js directory
//! into an AST.
//!
//! Supports injecting libraries either based on the library path name, or by the specific
//! `$jscomp.*` field/class name.
//!
//! `RUNTIME_LIB_DIR` and the nested `FieldsTable` need no AST and live in closure-resources
//! (`closure_resources::js::runtime_js_lib_manager`); they are re-exported here.
//!
//! Rust note: Java's manager reaches the compiler through the objects it was created with (the
//! `ResourceProvider`, the compiler's `ChangeTracker` and the `nodeForCodeInsertion` supplier).
//! In Rust those are callbacks on the compiler that owns the arena, so the methods that touch the
//! AST take `compiler: &mut Compiler`. The compiler shares its manager as `Arc<Mutex<..>>`
//! (`Compiler::get_runtime_js_lib_manager`).

use crate::{change_tracker::ChangeTracker, compiler::Compiler};
pub use closure_resources::js::runtime_js_lib_manager::{FieldsTable, RUNTIME_LIB_DIR};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    node::{Ast, NodeId},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// What to do with the runtime libraries under the js/ directory: the compiler can ignore them
/// completely; can validate any attempted library usage is correct but not modify the AST; or both
/// validate & add to the AST.
// port: RuntimeJsLibManager.RuntimeLibraryMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuntimeLibraryMode {
    INJECT,
    // mode where the compiler is building a TypedAST for the runtime libraries & can't use them,
    // or where the compiler is in transpile-only mode and running per-file.
    NO_OP,
    // for testing - compiler records that it was asked to inject a runtime library, but does not
    // modify the AST
    RECORD_ONLY,
    // for testing - a stricter version of RECORD_ONLY. The compiler records that it was asked to
    // inject a runtime library, and will also validate injection in JsLibField::assertInjected,
    // but does not modify the AST.
    RECORD_AND_VALIDATE_FIELDS,
    // for @closureUnaware nested compilation. When asked to inject a field, the compiler adds an
    // @extern definition of the field name to prevent misoptimization, and uses a different
    // identifier from the original uncompiled field name.
    // The RuntimeJsLibManager is not responsible for actually defining the external field name
    // definitions; the code using EXTERN_FIELD_NAMES mode must enure that everything field from
    // getExternedFields() is actually loaded at runtime.
    EXTERN_FIELD_NAMES,
}
impl RuntimeLibraryMode {
    pub const VALUES: &'static [Self] = &[
        Self::INJECT,
        Self::NO_OP,
        Self::RECORD_ONLY,
        Self::RECORD_AND_VALIDATE_FIELDS,
        Self::EXTERN_FIELD_NAMES,
    ];
    // port: RuntimeJsLibManager.RuntimeLibraryMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl std::fmt::Display for RuntimeLibraryMode {
    // port: RuntimeJsLibManager.RuntimeLibraryMode#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Loads /js resources into AST format.
///
/// Returns an AST for the given resource name, with a synthetic source file with the given path
/// (`resourceName` for example es6/set; `path` for example /my/filesystem/jscomp/js/es6/set).
/// `None` is Java's `null`.
// port: RuntimeJsLibManager.ResourceProvider
pub type ResourceProvider = Box<dyn FnMut(&mut Compiler, &str, &str) -> Option<NodeId> + Send>;

/// Java's `changeTracker` field: the compiler's `ChangeTracker`, split-borrowed with the arena
/// its methods take.
pub type ChangeTrackerAccess = fn(&mut Compiler) -> (&mut ChangeTracker, &mut Ast);

/// Java's `Supplier<Node> nodeForCodeInsertion`.
pub type NodeSupplier = Box<dyn FnMut(&mut Compiler) -> NodeId + Send>;

/// Injects runtime libraries from the jscomp/js directory into an AST.
pub struct RuntimeJsLibManager {
    change_tracker: ChangeTrackerAccess,
    mode: RuntimeLibraryMode,
    resource_provider: ResourceProvider,
    node_for_code_insertion: NodeSupplier,
    injected_libs: IndexSet<String>,
    interned_fields: IndexMap<String, Arc<InternalField>>,

    last_injected_library: Option<NodeId>,
}

impl RuntimeJsLibManager {
    // port: RuntimeJsLibManager#RuntimeJsLibManager
    fn new(
        change_tracker: ChangeTrackerAccess,
        mode: RuntimeLibraryMode,
        resource_provider: ResourceProvider,
        node_for_code_insertion: NodeSupplier,
    ) -> Self {
        Self {
            mode,
            change_tracker,
            resource_provider,
            node_for_code_insertion,
            injected_libs: IndexSet::<_>::default(),
            interned_fields: IndexMap::<_, _>::default(),
            last_injected_library: None,
        }
    }

    // port: RuntimeJsLibManager#create
    pub fn create(
        mode: RuntimeLibraryMode,
        resource_provider: ResourceProvider,
        change_tracker: ChangeTrackerAccess,
        node_for_code_insertion: NodeSupplier,
    ) -> Self {
        Self::new(
            change_tracker,
            mode,
            resource_provider,
            node_for_code_insertion,
        )
    }

    /// Records that the given library has already been injected into the compilation.
    ///
    /// This doesn't actually modify the AST - it can be used if e.g. restoring compilation state
    /// after serialization/deserialization.
    ///
    /// `resource_name`: The name of the library. For example, if "base" is is specified, then we
    /// record js/base.js
    // port: RuntimeJsLibManager#recordLibraryInjected
    pub fn record_library_injected(&mut self, resource_name: &str) {
        self.injected_libs.insert(resource_name.to_string());
    }

    /// Returns true if the given library has already been injected into the compilation.
    // port: RuntimeJsLibManager#hasInjectedLibrary
    pub fn has_injected_library(&self, resource_name: &str) -> bool {
        self.injected_libs.contains(resource_name)
    }

    /// Returns a list of all library paths previously injected, via one of the other methods on
    /// this class.
    // port: RuntimeJsLibManager#getInjectedLibraries
    pub fn get_injected_libraries(&self) -> Vec<String> {
        self.injected_libs.iter().cloned().collect()
    }

    /// Returns a list of all fields previously injected, via one of the other methods on this
    /// class.
    // port: RuntimeJsLibManager#getExternedFields
    pub fn get_externed_fields(&self) -> Vec<Arc<dyn ExternedField>> {
        if self.mode != RuntimeLibraryMode::EXTERN_FIELD_NAMES {
            return Vec::new();
        }
        self.interned_fields
            .values()
            .filter(|field| field.is_injected())
            .map(|field| Arc::clone(field) as Arc<dyn ExternedField>)
            .collect()
    }

    // port: RuntimeJsLibManager#createField
    fn create_field(&self, field_name: &str) -> InternalField {
        check_argument!(!field_name.is_empty(), "%s", field_name);

        let parts: Vec<&str> = field_name.split('.').collect();
        check_argument!(
            parts.len() >= 2,
            "Field name must start with $jscomp., found %s",
            field_name
        );
        check_argument!(
            parts[0] == "$jscomp",
            "Field name must start with $jscomp, found %s",
            field_name
        );
        let prop_name = parts[1];

        let resource_name = check_not_null!(
            FieldsTable::instance().get(prop_name),
            "Cannot find definition of %s",
            field_name
        )
        .clone();

        let compiled_name = match self.mode {
            RuntimeLibraryMode::EXTERN_FIELD_NAMES => field_name.replace('.', "_"),
            RuntimeLibraryMode::NO_OP
            | RuntimeLibraryMode::RECORD_ONLY
            | RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS
            | RuntimeLibraryMode::INJECT => field_name.to_string(),
        };

        InternalField::new(
            self.mode,
            resource_name,
            compiled_name,
            field_name.to_string(),
        )
    }

    /// Returns a field definition for the given name, without actually asserting or requiring its
    /// definition to be injected.
    ///
    /// Panics (Java `IllegalArgumentException`) if passed a nonexistent field name.
    // port: RuntimeJsLibManager#getJsLibField
    pub fn get_js_lib_field(&mut self, field_name: &str) -> Arc<dyn JsLibField> {
        // wrapper around getJsLibFieldInternal that returns the broader JsLibField interface
        self.get_js_lib_field_internal(field_name)
    }

    // port: RuntimeJsLibManager#getJsLibFieldInternal
    fn get_js_lib_field_internal(&mut self, field_name: &str) -> Arc<InternalField> {
        if let Some(field) = self.interned_fields.get(field_name) {
            return Arc::clone(field);
        }
        let field = Arc::new(self.create_field(field_name));
        self.interned_fields
            .insert(field_name.to_string(), Arc::clone(&field));
        field
    }

    /// Injects the runtime library that defines the given $jscomp.* field.
    // port: RuntimeJsLibManager#injectLibForField
    pub fn inject_lib_for_field(&mut self, compiler: &mut Compiler, field_name: &str) {
        let field = self.get_js_lib_field_internal(field_name);
        if field.is_injected() {
            return; // already done.
        }
        field.mark_injected();
        if self.mode == RuntimeLibraryMode::EXTERN_FIELD_NAMES {
            let extern_root = (self.node_for_code_insertion)(compiler);
            check_state!(
                extern_root.is_from_externs(compiler),
                "%s",
                extern_root.to_string(compiler)
            );
            let name = IR::name(compiler, field.qualified_name.as_str());
            let declaration = IR::var(compiler, name);
            declaration
                .get_first_child(compiler)
                .unwrap()
                .put_boolean_prop(compiler, NodeId::IS_CONSTANT_NAME, true);
            extern_root.add_child_to_back(compiler, declaration);
        }
        self.ensure_library_injected(compiler, &field.resource_name, /* force= */ false);
    }

    /// The subdir js/ contains libraries of code that we inject at compile-time only if requested
    /// by this function.
    ///
    /// Notice that these libraries will almost always create global symbols.
    ///
    /// `resource_name`: The name of the library. For example, if "base" is is specified, then we
    /// load js/base.js. `force`: Inject the library even if compiler options say not to.
    ///
    /// Returns the last node of the most-recently-injected runtime library. If new code was
    /// injected, this will be the last expression node of the library. If the caller needs to add
    /// additional code, they should add it as the next sibling of this node. If no runtime
    /// libraries have been injected, then null is returned.
    // port: RuntimeJsLibManager#ensureLibraryInjected
    pub fn ensure_library_injected(
        &mut self,
        compiler: &mut Compiler,
        resource_name: &str,
        force: bool,
    ) -> Option<NodeId> {
        if !force {
            match self.mode {
                RuntimeLibraryMode::NO_OP => {
                    return self.last_injected_library;
                }
                RuntimeLibraryMode::RECORD_ONLY
                | RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS
                | RuntimeLibraryMode::EXTERN_FIELD_NAMES => {
                    self.record_library_injected(resource_name);
                    return self.last_injected_library;
                }
                RuntimeLibraryMode::INJECT => {} // Keep going.
            }
        }

        if self.injected_libs.contains(resource_name) {
            return self.last_injected_library;
        }
        self.record_library_injected(resource_name);

        let path = [RUNTIME_LIB_DIR, resource_name, ".js"].join("");
        let ast = (self.resource_provider)(compiler, resource_name, &path);

        self.inject_library_dependencies(compiler, ast, force); // may also modify 'ast'
        self.inject_library(compiler, ast.unwrap())
    }

    // port: RuntimeJsLibManager#injectLibrary
    fn inject_library(&mut self, compiler: &mut Compiler, ast: NodeId) -> Option<NodeId> {
        if !ast.has_children(compiler) {
            // Require-only libraries may be empty at this point. Nothing to do here.
            return self.last_injected_library;
        }

        let mut child = ast.get_first_child(compiler);
        while let Some(c) = child {
            let (change_tracker, arena) = (self.change_tracker)(compiler);
            change_tracker.mark_new_scopes_changed(arena, c);
            child = c.get_next(compiler);
        }
        let end_of_lib = ast.get_last_child(compiler).unwrap();
        let first_child = ast.remove_children(compiler);

        // Insert the code immediately after the last-inserted runtime library, if any.
        let parent = (self.node_for_code_insertion)(compiler);
        if self.last_injected_library.is_none() {
            parent.add_children_to_front(compiler, first_child);
        } else {
            parent.add_children_after(compiler, first_child, self.last_injected_library);
        }
        self.last_injected_library = Some(end_of_lib);

        let (change_tracker, arena) = (self.change_tracker)(compiler);
        change_tracker.report_change_to_enclosing_scope(arena, parent);
        Some(end_of_lib)
    }

    // port: RuntimeJsLibManager#injectLibraryDependencies
    fn inject_library_dependencies(
        &mut self,
        compiler: &mut Compiler,
        ast: Option<NodeId>,
        force: bool,
    ) {
        // Java dereferences the (nullable) parsed library here.
        let ast = ast.expect("NullPointerException: resourceProvider returned null");
        // Look for string literals of the form 'require foo bar'
        // As we process each one, remove it from its parent.
        let mut node = ast.get_first_child(compiler);
        while let Some(n) = node {
            if !(n.is_expr_result(compiler)
                && n.get_first_child(compiler).unwrap().is_string_lit(compiler))
            {
                break;
            }
            let directive = n
                .get_first_child(compiler)
                .unwrap()
                .get_string(compiler)
                .to_string_lossy();
            let words: Vec<&str> = directive.splitn(2, ' ').collect();
            match words[0] {
                // 'use strict' is ignored (and deleted).
                "use" => {}
                // 'require lib'; pulls in the named library before this one.
                "require" => {
                    self.ensure_library_injected(compiler, words[1], force);
                }
                _ => panic!("Bad directive: {directive}"),
            }
            n.detach(compiler);
            node = ast.get_first_child(compiler);
        }
    }

    // port: RuntimeJsLibManager#setLastInjectedLibrary
    pub fn set_last_injected_library(&mut self, last_injected_library: Option<NodeId>) {
        self.last_injected_library = last_injected_library;
    }

    // port: RuntimeJsLibManager#getLastInjectedLibrary
    pub fn get_last_injected_library(&self) -> Option<NodeId> {
        self.last_injected_library
    }
}

/// A $jscomp.* field (could be a method, class, or any arbitrary value) that exists in some
/// runtime library under the js/ directory.
// port: RuntimeJsLibManager.InternalField
pub struct InternalField {
    /// Java reads the enclosing manager's (final) `mode`.
    mode: RuntimeLibraryMode,
    /// The file containing this field, e.g. "es6/generator".
    resource_name: String,

    /// The name by which compiler passes should refer to this field, e.g. `$jscomp.inherits` or
    /// `$jscomp_inherits`.
    qualified_name: String,

    /// The original fully qualified name of this field, e.g. `$jscomp.inherits`.
    uncompiled_name: String,

    injected: AtomicBool,
}

impl InternalField {
    // port: RuntimeJsLibManager.InternalField#InternalField
    fn new(
        mode: RuntimeLibraryMode,
        resource_name: String,
        qualified_name: String,
        uncompiled_name: String,
    ) -> Self {
        Self {
            mode,
            resource_name,
            qualified_name,
            uncompiled_name,
            injected: AtomicBool::new(false),
        }
    }

    // port: RuntimeJsLibManager.InternalField#isInjected
    fn is_injected(&self) -> bool {
        self.injected.load(Ordering::Relaxed)
    }

    // port: RuntimeJsLibManager.InternalField#markInjected
    fn mark_injected(&self) {
        self.injected.store(true, Ordering::Relaxed);
    }
}

// Don't override .equals/.hashCode and just use reference equality: we already intern Fields
// by qualifiedName.

impl std::fmt::Display for InternalField {
    // port: RuntimeJsLibManager.InternalField#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Field<{}, {}>", self.uncompiled_name, self.resource_name)
    }
}

/// A $jscomp.* field (could be a method, class, or any arbitrary value) that exists in some
/// runtime library under the js/ directory.
// port: RuntimeJsLibManager.JsLibField
pub trait JsLibField: Send + Sync + std::fmt::Display {
    // port: RuntimeJsLibManager.JsLibField#matches
    fn matches(&self, ast: &Ast, node: NodeId) -> bool;

    // port: RuntimeJsLibManager.JsLibField#resourceName
    fn resource_name(&self) -> &str;

    // port: RuntimeJsLibManager.JsLibField#assertInjected
    fn assert_injected(&self) -> &dyn InjectedJsLibField;
}

/// A [`JsLibField`] that is statically guaranteed to be available at runtime, i.e. that has been
/// injected into the AST.
///
/// Only `InjectedJsLibField` instances provide direct access to the field name - this is to
/// prevent code from creating new AST references to a field that's not actually injected (yet).
// port: RuntimeJsLibManager.InjectedJsLibField
pub trait InjectedJsLibField: JsLibField {
    /// Returns the name of the field as it is available to the compiler.
    // port: RuntimeJsLibManager.InjectedJsLibField#qualifiedName
    fn qualified_name(&self) -> &str;
}

/// A [`InjectedJsLibField`] that was injected in [`RuntimeLibraryMode::EXTERN_FIELD_NAMES`] mode,
/// and so has both a regular "qualified name" by which transpilation passes should refer to it -
/// this is the externed name - but also provides the uncompiled, raw name of the field.
///
/// Only `ExternedField` instances provide access to the uncompiled name. Most use sites should use
/// the compiled/qualified name, which may or may not equal the uncompiled name. So to try and
/// minimize confusion, we only expose the uncompiled name when this is actually an external field
/// injection.
// port: RuntimeJsLibManager.ExternedField
pub trait ExternedField: InjectedJsLibField {
    /// Returns the uncompiled name of the field, as it is seen in the raw runtime lib.
    // port: RuntimeJsLibManager.ExternedField#uncompiledName
    fn uncompiled_name(&self) -> &str;
}

impl JsLibField for InternalField {
    // port: RuntimeJsLibManager.InternalField#matches
    fn matches(&self, ast: &Ast, node: NodeId) -> bool {
        node.matches_qualified_name(ast, self.qualified_name.as_str())
    }

    // port: RuntimeJsLibManager.InternalField#resourceName
    fn resource_name(&self) -> &str {
        &self.resource_name
    }

    // port: RuntimeJsLibManager.InternalField#assertInjected
    fn assert_injected(&self) -> &dyn InjectedJsLibField {
        match self.mode {
            RuntimeLibraryMode::NO_OP | RuntimeLibraryMode::RECORD_ONLY => {}
            RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS
            | RuntimeLibraryMode::INJECT
            | RuntimeLibraryMode::EXTERN_FIELD_NAMES => {
                check_state!(
                    self.is_injected(),
                    "Field %s is not injected",
                    self.uncompiled_name
                );
            }
        }
        self
    }
}

impl InjectedJsLibField for InternalField {
    // port: RuntimeJsLibManager.InternalField#qualifiedName
    fn qualified_name(&self) -> &str {
        &self.qualified_name
    }
}

impl ExternedField for InternalField {
    // port: RuntimeJsLibManager.InternalField#uncompiledName
    fn uncompiled_name(&self) -> &str {
        &self.uncompiled_name
    }
}
