/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RemoveUnnecessarySyntheticExterns.java.

//! Port of `RemoveUnnecessarySyntheticExterns.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_input::CompilerInput;
use crate::compiler_pass::CompilerPass;
use crate::scope::ScopeId;
use crate::syntactic_scope_creator::{RedeclarationHandler, SyntacticScopeCreator};
use closure_rhino::check_argument;
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use std::cell::RefCell;
use std::rc::Rc;

/// Deletes name declarations from synthetic externs if they are in fact declared in source
///
/// The VarCheck pass will create synthetic extern declarations (`var someName;`) for every
/// variable name referenced without a corresponding externs or code declaration. Such names are
/// "unfulfilled". VarCheck only does this because later compiler passes expect the following
/// invariant to hold: all variable names referenced in externs or code are also declared in
/// externs or code.
///
/// When running optimizations based on precompiled TypedAST library shards, it's possible for a
/// name to be unfulfilled in shard A, but declared in another shard B. Shard B "fulfills" the
/// reference from shard A. This pass removes the corresponding synthetic extern that came from
/// shard A now that the name is fulfilled. The purpose is to improve optimizations, e.g. to allow
/// mangling the name if it is not defined in externs.
pub struct RemoveUnnecessarySyntheticExterns {
    // Shared with the RedeclarationCheckHandler the scope creator owns (Java's inner class reads
    // the outer field).
    nodes_to_detach: Rc<RefCell<IndexSet<NodeId>>>,
}

impl RemoveUnnecessarySyntheticExterns {
    // port: RemoveUnnecessarySyntheticExterns#RemoveUnnecessarySyntheticExterns
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            nodes_to_detach: Rc::new(RefCell::new(IndexSet::<_>::default())),
        }
    }
}

impl CompilerPass for RemoveUnnecessarySyntheticExterns {
    // port: RemoveUnnecessarySyntheticExterns#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, _root: NodeId) {
        let global_root = externs.get_parent(compiler).unwrap();
        let mut scope_creator = SyntacticScopeCreator::new_with_options(
            Box::new(RedeclarationCheckHandler {
                nodes_to_detach: self.nodes_to_detach.clone(),
            }),
            true,
        );

        // This call will invoke the RedeclarationCheckHandler on duplicate synthetic names
        scope_creator.create_scope(compiler, global_root, /* parent= */ None);

        if self.nodes_to_detach.borrow().is_empty() {
            return;
        }

        for n in self.nodes_to_detach.borrow().iter().copied() {
            n.detach(compiler);
        }

        let synthetic_externs_input = compiler.get_synthesized_externs_input().clone();
        let synthetic_externs_script = synthetic_externs_input.get_ast_root(compiler);
        compiler.report_change_to_enclosing_scope(synthetic_externs_script);
    }
}

/// The handler for duplicate declarations.
struct RedeclarationCheckHandler {
    nodes_to_detach: Rc<RefCell<IndexSet<NodeId>>>,
}

impl RedeclarationHandler for RedeclarationCheckHandler {
    // port: RemoveUnnecessarySyntheticExterns.RedeclarationCheckHandler#onRedeclaration
    fn on_redeclaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        s: ScopeId,
        name: &JsString,
        n: NodeId,
        input: Option<CompilerInput>,
    ) {
        check_argument!(
            s.is_global(compiler),
            "Unexpected non-global redeclaration %s in scope %s",
            n.to_string(compiler),
            s.to_string(compiler)
        );

        // Java's input.equals(...) dereferences input.
        let is_synthetic = input.unwrap() == *compiler.get_synthesized_externs_input();
        let orig_var = s.get_var(compiler, name.clone()).unwrap();
        // origNode will be null for `arguments`, since there's no node that declares it.
        let orig_node = orig_var.get_node(compiler);

        if is_synthetic {
            // Delete duplicate unfulfilled synthetic declarations.
            // The original declaration will be deleted later iff it's also synthetic and we visit
            // a non-synthetic duplicate declaration.
            let parent = n.get_parent(compiler).unwrap();
            if parent.is_synthesized_unfulfilled_name_declaration(compiler) {
                self.nodes_to_detach.borrow_mut().insert(parent);
            }
        } else if let Some(orig_node) = orig_node
            && orig_node
                .get_parent(compiler)
                .unwrap()
                .is_synthesized_unfulfilled_name_declaration(compiler)
        {
            // Found a synthetic extern declaration that's fulfilled by some non-synthetic code.
            self.nodes_to_detach
                .borrow_mut()
                .insert(orig_node.get_parent(compiler).unwrap());
        }
    }
}
