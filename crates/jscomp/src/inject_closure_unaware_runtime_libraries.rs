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
//   src/com/google/javascript/jscomp/InjectClosureUnawareRuntimeLibraries.java.

//! Port of `InjectClosureUnawareRuntimeLibraries.java`.
//!
//! Takes the set of runtime libraries and fields used in a @closureUnaware nested compilation, and
//! then 1) ensures the library definitions are injected in the main AST and 2) passes all specific
//! $jscomp.* fields used into the @closureUnaware shadow AST as an argument.
//!
//! Before:
//!
//! ```text
//!   /** @closureUnaware * /
//!   (function() {
//!     use($jscomp_someField$$);
//!   }).call(undefined);
//! ```
//!
//! After:
//!
//! ```text
//!   /** @closureUnaware * /
//!   (function($jscomp_someField$$) {
//!     use($jscomp_someField$$);
//!   }).call(undefined, $jscomp.someField);
//! ```
//!
//! Rust-only split (borrow checker, DESIGN §6): Java holds both compilers at once. Here the pass
//! runs inside the shadow compilation, which owns the shared node arena while the main compiler
//! waits in `TranspileAndOptimizeClosureUnaware#process`. The pass applies its shadow-side changes
//! at once and queues its main-compiler steps (`MainCompilerQueue`) in Java order;
//! `TranspileAndOptimizeClosureUnaware` applies them to the main compiler as soon as the nested
//! compilation returns, before anything else touches the main compiler.
use crate::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    js::runtime_js_lib_manager::ExternedField, node_util::NodeUtil,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::sync::{Arc, Mutex};

/// Rust-only: one step on the main compiler that Java's pass performs through `mainCompiler`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MainCompilerStep {
    /// `mainManager.ensureLibraryInjected(runtimeLib, /* force= */ false)`
    EnsureLibraryInjected(String),
    /// `argumentParent.addChildToBack(NodeUtil.newQName(mainCompiler, uncompiledName)
    /// .srcrefTree(argumentParent)); mainCompiler.reportChangeToEnclosingScope(argumentParent)`
    AddArgument {
        argument_parent: NodeId,
        uncompiled_name: String,
    },
}

/// Rust-only: the main-compiler steps queued by the pass, in Java order.
pub type MainCompilerQueue = Arc<Mutex<Vec<MainCompilerStep>>>;

/// Rust-only: performs the queued main-compiler steps, in order, on the main compiler.
pub fn apply_main_compiler_steps(main_compiler: &mut AbstractCompiler, queue: &MainCompilerQueue) {
    let steps = std::mem::take(&mut *queue.lock().unwrap());
    for step in steps {
        match step {
            MainCompilerStep::EnsureLibraryInjected(runtime_lib) => {
                let main_manager = main_compiler.get_runtime_js_lib_manager();
                main_manager.lock().unwrap().ensure_library_injected(
                    main_compiler,
                    &runtime_lib,
                    /* force= */ false,
                );
            }
            MainCompilerStep::AddArgument {
                argument_parent,
                uncompiled_name,
            } => {
                let argument = NodeUtil::new_qname(main_compiler, uncompiled_name.as_str())
                    .srcref_tree(main_compiler, argument_parent);
                argument_parent.add_child_to_back(main_compiler, argument);
                main_compiler.report_change_to_enclosing_scope(argument_parent);
            }
        }
    }
}

// port: InjectClosureUnawareRuntimeLibraries.ClosureUnawareCallSite
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosureUnawareCallSite {
    call_node: NodeId,
    closure_unaware_function: NodeId,
}

impl ClosureUnawareCallSite {
    // port: InjectClosureUnawareRuntimeLibraries.ClosureUnawareCallSite#ClosureUnawareCallSite
    pub fn new(ast: &Ast, call_node: NodeId, closure_unaware_function: NodeId) -> Self {
        check_argument!(call_node.is_call(ast), "{}", call_node.to_string(ast));
        check_argument!(
            closure_unaware_function.is_function(ast),
            "{}",
            closure_unaware_function.to_string(ast)
        );
        Self {
            call_node,
            closure_unaware_function,
        }
    }

    // port: InjectClosureUnawareRuntimeLibraries.ClosureUnawareCallSite#callNode
    pub fn call_node(&self) -> NodeId {
        self.call_node
    }

    // port: InjectClosureUnawareRuntimeLibraries.ClosureUnawareCallSite#closureUnawareFunction
    pub fn closure_unaware_function(&self) -> NodeId {
        self.closure_unaware_function
    }
}

pub struct InjectClosureUnawareRuntimeLibraries {
    /// Java's `mainCompiler`: the queue of steps on the main compiler (see the module docs).
    main_compiler: MainCompilerQueue,
    closure_unaware_calls: Vec<ClosureUnawareCallSite>,
}

impl InjectClosureUnawareRuntimeLibraries {
    // port: InjectClosureUnawareRuntimeLibraries#InjectClosureUnawareRuntimeLibraries
    pub fn new(
        main_compiler: MainCompilerQueue,
        closure_unaware_calls: Vec<ClosureUnawareCallSite>,
    ) -> Self {
        Self {
            main_compiler,
            closure_unaware_calls,
        }
    }

    // port: InjectClosureUnawareRuntimeLibraries#injectLibrariesIntoMainAst
    fn inject_libraries_into_main_ast(&mut self, shadow_compiler: &mut AbstractCompiler) {
        let shadow_manager = shadow_compiler.get_runtime_js_lib_manager();
        let injected_libraries = shadow_manager.lock().unwrap().get_injected_libraries();
        for runtime_lib in injected_libraries {
            self.main_compiler
                .lock()
                .unwrap()
                .push(MainCompilerStep::EnsureLibraryInjected(runtime_lib));
        }
    }

    // port: InjectClosureUnawareRuntimeLibraries#injectFieldsIntoCallSites
    fn inject_fields_into_call_sites(&mut self, shadow_compiler: &mut AbstractCompiler) {
        let mut fields_by_name: IndexMap<JsString, Arc<dyn ExternedField>> =
            IndexMap::<_, _>::default();
        let externed_fields = shadow_compiler
            .get_runtime_js_lib_manager()
            .lock()
            .unwrap()
            .get_externed_fields();
        for field in externed_fields {
            let name = JsString::from(field.qualified_name());
            // toImmutableMap rejects duplicate keys.
            assert!(
                !fields_by_name.contains_key(&name),
                "Multiple entries with same key: {name}"
            );
            fields_by_name.insert(name, field);
        }

        for call_site in self.closure_unaware_calls.clone() {
            let to_inject = Self::gather_runtime_fields(
                shadow_compiler,
                call_site.closure_unaware_function(),
                &fields_by_name,
            );
            self.inject_all(shadow_compiler, call_site, to_inject);
        }
    }

    // port: InjectClosureUnawareRuntimeLibraries#injectAll
    fn inject_all(
        &mut self,
        shadow_compiler: &mut AbstractCompiler,
        call_site: ClosureUnawareCallSite,
        to_inject: Vec<Arc<dyn ExternedField>>,
    ) {
        if to_inject.is_empty() {
            return;
        }
        let parameter_parent = NodeUtil::get_function_parameters(
            shadow_compiler,
            call_site.closure_unaware_function(),
        );
        let argument_parent = call_site.call_node();

        if parameter_parent.has_children(shadow_compiler)
            && parameter_parent
                .get_last_child(shadow_compiler)
                .unwrap()
                .is_rest(shadow_compiler)
        {
            panic!(
                "rest parameters not allowed in closureUnaware function parameters, found {}",
                parameter_parent
                    .get_last_child(shadow_compiler)
                    .unwrap()
                    .to_string(shadow_compiler)
            );
        }

        for field in to_inject {
            let parameter = IR::name(shadow_compiler, field.qualified_name())
                .srcref(shadow_compiler, parameter_parent);
            // The argument is created on the main compiler (queued, see the module docs).
            let argument = MainCompilerStep::AddArgument {
                argument_parent,
                uncompiled_name: field.uncompiled_name().to_string(),
            };

            parameter_parent.add_child_to_back(shadow_compiler, parameter);
            shadow_compiler.report_change_to_enclosing_scope(parameter_parent);

            self.main_compiler.lock().unwrap().push(argument);
        }
    }

    /// Returns the subset of `all_injected_fields` that are actually referenced in the given
    /// `shadow_ast`.
    // port: InjectClosureUnawareRuntimeLibraries#gatherRuntimeFields
    #[allow(clippy::collapsible_if)] // Retain Java control flow.
    fn gather_runtime_fields(
        ast: &mut Ast,
        root: NodeId,
        all_known_fields: &IndexMap<JsString, Arc<dyn ExternedField>>,
    ) -> Vec<Arc<dyn ExternedField>> {
        // LinkedHashSet of fields (identity of the field objects, insertion order).
        let mut seen: IndexSet<usize> = IndexSet::<_>::default();
        let mut seen_fields: Vec<Arc<dyn ExternedField>> = Vec::new();
        let mut visitor = |ast: &mut Ast, n: NodeId| {
            if !n.is_name(ast) {
                return;
            }
            let field = all_known_fields.get(&n.get_string(ast));
            if let Some(field) = field {
                if seen.insert(Arc::as_ptr(field) as *const () as usize) {
                    seen_fields.push(field.clone());
                }
            }
        };
        NodeUtil::visit_pre_order(ast, root, &mut visitor);
        seen_fields
    }
}

impl CompilerPass for InjectClosureUnawareRuntimeLibraries {
    // port: InjectClosureUnawareRuntimeLibraries#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        self.inject_libraries_into_main_ast(compiler);
        self.inject_fields_into_call_sites(compiler);
    }
}
