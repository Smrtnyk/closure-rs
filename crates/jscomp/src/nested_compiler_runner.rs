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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/NestedCompilerRunner.java.

//! Port of `NestedCompilerRunner.java`.
//!
//! Runs a nested compilation over @closureUnaware shadow ASTs on a new (shadow) Compiler.
//!
//! Rust-only (DESIGN §6): Java moves the shadow SCRIPT nodes from the main AST into the shadow
//! Compiler; Node objects belong to no compiler there. Here every node lives in its compiler's
//! arena, so the shadow Compiler borrows the main compiler's arena (`Compiler::ast`) for the
//! nested compilation and gives it back afterwards: node identities stay exactly Java's, and the
//! main AST nodes are unreachable from the shadow roots, as in Java. The original compiler is not
//! stored (passes never store the compiler); the methods that use it receive it.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler::Compiler,
    compiler_input::CompilerInput,
    compiler_options::{CompilerOptions, SegmentOfCompilationToRun},
    compiler_pass::CompilerPass,
    js_chunk::JSChunk,
    node_util::NodeUtil,
    pass_config::PassConfig,
    source_file::SourceFile,
    var_check::VarCheck,
};
use closure_rhino::{
    check_argument, check_state,
    input_id::InputId,
    node::{Ast, NodeId},
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::sync::Arc;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    TRANSPILE_AND_OPTIMIZE,
    TRANSPILE_ONLY,
}

pub struct NestedCompilerRunner {
    shadow_compiler: Compiler,
    inputs: Vec<CompilerInput>,
    base_chunk: JSChunk,

    shadow_options: CompilerOptions,
    mode: Mode,
    shadow_pass_config: Option<Box<dyn PassConfig>>,
}

impl NestedCompilerRunner {
    // port: NestedCompilerRunner#NestedCompilerRunner
    fn new(
        shadow_options: CompilerOptions,
        mode: Mode,
        shadow_pass_config: Box<dyn PassConfig>,
    ) -> Self {
        Self {
            shadow_compiler: Compiler::new(),
            inputs: Vec::new(),
            base_chunk: JSChunk::new("shadow_base"),
            shadow_options,
            mode,
            shadow_pass_config: Some(shadow_pass_config),
        }
    }

    // port: NestedCompilerRunner#create
    pub fn create(
        shadow_options: CompilerOptions,
        mode: Mode,
        shadow_pass_config: Box<dyn PassConfig>,
    ) -> Self {
        Self::new(shadow_options, mode, shadow_pass_config)
    }

    // port: NestedCompilerRunner#addScript
    pub fn add_script(
        &mut self,
        original: &mut AbstractCompiler,
        shadow_script: NodeId,
        unique_shadow_id: &str,
    ) -> &mut Self {
        check_argument!(
            shadow_script.get_is_in_closure_unaware_subtree(original),
            "Expected closureUnaware script, found {}",
            shadow_script.to_string(original)
        );

        let shadow_file = Arc::new(SourceFile::from_code_with_kind(
            unique_shadow_id,
            "",
            SourceKind::STRONG,
        ));

        let shadow_input =
            CompilerInput::new_with_input_id(shadow_file.clone(), InputId::new(unique_shadow_id));
        shadow_input.init_shadow_ast(original, shadow_script);

        shadow_script.set_input_id(
            original,
            Some(Arc::new(shadow_input.get_input_id().clone())),
        );
        shadow_script
            .set_static_source_file(original, Some(shadow_file as Arc<dyn StaticSourceFile>));
        // TODO: b/421971366 - should we also update the shadow script children with this source
        // file?

        self.inputs.push(shadow_input);
        self
    }

    /// Initializes a new Compiler with all shadow ASTs and runs transpilation/optimizations
    // port: NestedCompilerRunner#compile
    pub fn compile(&mut self, original: &mut AbstractCompiler) {
        let externs_root = original
            .get_root()
            .unwrap()
            .get_first_child(original)
            .unwrap();
        let extern_names = NodeUtil::collect_extern_variable_names(original, externs_root);
        // Rust-only: the shadow compiler works on the main compiler's arena (module docs).
        self.shadow_compiler.ast = std::mem::replace(&mut original.ast, Ast::new());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.initialize_compiler(original, extern_names);
            self.run_compilation(original);
            self.check_invariants();
        }));
        original.ast = std::mem::replace(&mut self.shadow_compiler.ast, Ast::new());
        // Rust-only: the unported factories the shadow compile omitted count for the original.
        original.add_omitted_unported_passes(self.shadow_compiler.get_omitted_unported_passes());
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }

    /// Creates a new Compiler and initializes it with a chunk graph holding all shadow ASTs
    // port: NestedCompilerRunner#initializeCompiler
    fn initialize_compiler(
        &mut self,
        original: &mut AbstractCompiler,
        extern_names: closure_rhino::fast_hash::IndexSet<closure_rhino::js_string::JsString>,
    ) {
        // `externNames` is collected by `compile` while the main compiler still holds the arena.
        self.shadow_compiler
            .add_exported_names(extern_names.into_iter().map(|name| name.to_string()));
        // Java passes null through; a new Compiler already holds null there.
        if let Some(extern_properties) = original.get_extern_properties() {
            self.shadow_compiler
                .set_extern_properties(extern_properties.clone());
        }
        self.shadow_compiler
            .set_debug_message("<shadow AST compilation>".to_string());

        let chunks = self.create_chunks();
        self.shadow_compiler
            .init_chunks(&[], chunks, self.shadow_options.clone());
        self.shadow_compiler
            .set_pass_config(self.shadow_pass_config.take().unwrap());
        self.shadow_compiler.parse_for_compilation();
    }

    // port: NestedCompilerRunner#createChunks
    fn create_chunks(&mut self) -> Vec<JSChunk> {
        self.base_chunk
            .add(CompilerInput::new(SourceFile::from_code_with_kind(
                "synthetic_base",
                "",
                SourceKind::STRONG,
            )));
        let mut chunks = vec![self.base_chunk.clone()];
        for input in &self.inputs {
            let chunk = JSChunk::new(format!(
                "shadow_chunk_{}",
                input.get_source_file().get_name()
            ));
            chunk.add(input.clone());
            chunk.add_dependency(&self.base_chunk);
            chunks.push(chunk);
        }
        chunks
    }

    /// Runs compiler passes over the shadow ASTs
    // port: NestedCompilerRunner#runCompilation
    fn run_compilation(&mut self, original: &mut AbstractCompiler) {
        match self.mode {
            Mode::TRANSPILE_AND_OPTIMIZE => {
                // Run VarCheck to comply with a compiler invariant: when running optimizations,
                // all NAME nodes in the AST must have a corresponding declaration in source or
                // externs. The optimizer will either crash or silently misoptimize code otherwise.
                // (VarCheck not only reports errors for referencing undefined variables, but also
                // adds "synthetic extern declarations" of undefined variables, in case the errors
                // are suppressed and we want to continue compilation)
                let externs_root = self.shadow_compiler.get_externs_root().unwrap();
                let js_root = self.shadow_compiler.get_js_root().unwrap();
                VarCheck::new(&self.shadow_compiler).process(
                    &mut self.shadow_compiler,
                    externs_root,
                    js_root,
                );
                self.delegate_diagnostics_to_original(original);
                if self.shadow_compiler.has_halting_errors() {
                    return;
                }
                self.shadow_compiler
                    .stage2_passes(SegmentOfCompilationToRun::OPTIMIZATIONS);
                self.delegate_diagnostics_to_original(original);
                if self.shadow_compiler.has_halting_errors() {
                    return;
                }
                self.shadow_compiler.stage3_passes();
                self.delegate_diagnostics_to_original(original);
            }
            Mode::TRANSPILE_ONLY => {
                self.shadow_compiler.transpile_and_dont_check();
                self.delegate_diagnostics_to_original(original);
            }
        }
    }

    // port: NestedCompilerRunner#delegateDiagnosticsToOriginal
    fn delegate_diagnostics_to_original(&mut self, original: &mut AbstractCompiler) {
        // Rust-only: the original compiler gets the shared arena back while it reports (its
        // warnings guards may read the error nodes).
        std::mem::swap(&mut original.ast, &mut self.shadow_compiler.ast);
        for error in self.shadow_compiler.get_errors() {
            original.report(error);
        }
        for error in self.shadow_compiler.get_warnings() {
            original.report(error);
        }
        std::mem::swap(&mut original.ast, &mut self.shadow_compiler.ast);
    }

    // port: NestedCompilerRunner#checkInvariants
    fn check_invariants(&mut self) {
        let base_inputs = self.base_chunk.get_inputs();
        check_state!(
            base_inputs.len() == 1,
            "Expected exactly 1 base chunk input, got {}",
            format!(
                "[{}]",
                base_inputs
                    .iter()
                    .map(|input| input.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        );

        // Check that compilation did not introduce any new code into the base chunk: we don't
        // have a place to put that code in the main AST, once we move out of this nested
        // compilation.
        // Note: if it turns out to be useful to have some shared common base code for shadow
        // ASTs, we could consider designing a "common" closureUnaware shadow AST that goes in the
        // actual main AST root chunk. But we don't have a use case for that yet.
        let base_input_node = base_inputs[0].get_ast_root(&mut self.shadow_compiler);
        check_state!(
            !base_input_node.has_children(&self.shadow_compiler),
            "Expected synthetic base input chunk to have zero children, but found {}",
            base_input_node.to_string_tree(&self.shadow_compiler)
        );
    }
}
