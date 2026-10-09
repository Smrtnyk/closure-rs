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
//   src/com/google/javascript/jscomp/TranspileAndOptimizeClosureUnaware.java.

//! Port of `TranspileAndOptimizeClosureUnaware.java`.
//!
//! Runs a new Compiler instance over all @closureUnaware code (i.e. "shadow roots") in the AST.
//!
//! This is a separate Compiler instance to avoid leaking compiler state between the shadow AST
//! and main AST. Otherwise, we risk doing unsafe optimizations on @closureUnaware code.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_unaware_options::ClosureUnawareOptions,
    compiler_options::CompilerOptions,
    compiler_pass::CompilerPass,
    default_pass_config::DefaultPassConfig,
    inject_closure_unaware_runtime_libraries::{
        ClosureUnawareCallSite, InjectClosureUnawareRuntimeLibraries, MainCompilerQueue,
        apply_main_compiler_steps,
    },
    nested_compiler_runner::{Mode, NestedCompilerRunner},
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    pass_config::{PassConfig, PassConfigDelegate},
    pass_factory::PassFactory,
    pass_list_builder::PassListBuilder,
    pass_names,
    source_file::SourceFile,
};
use closure_parsing::feature_collector::FeatureCollector;
use closure_rhino::{
    check_state,
    jsdoc_info::PerFileClosureUnawareMode,
    node::{NodeId, ObjectProp, Prop},
    static_source_file::StaticSourceFile,
};
use std::sync::{Arc, Mutex};

pub struct TranspileAndOptimizeClosureUnaware {
    mode: Mode,
}

impl TranspileAndOptimizeClosureUnaware {
    // port: TranspileAndOptimizeClosureUnaware#TranspileAndOptimizeClosureUnaware
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }

    // port: TranspileAndOptimizeClosureUnaware#initShadowInputs
    #[allow(clippy::explicit_counter_loop)] // Retain Java's indexInFile++.
    fn init_shadow_inputs(
        &self,
        original: &mut AbstractCompiler,
        shadow_compiler: &mut NestedCompilerRunner,
        per_file_inputs: &ShadowAstsByFile,
    ) {
        for (source_file, shadow_asts) in per_file_inputs {
            let mut index_in_file = 0;
            for shadow_ast in shadow_asts {
                let unique_name = format!("{}.shadow{}", source_file.get_name(), index_in_file);
                index_in_file += 1;
                let script = shadow_ast.script().detach(original);
                shadow_compiler.add_script(original, script, &unique_name);
            }
        }
    }

    /// Re-attaches the shadow AST scripts, that were previously detached for compilation, to
    /// their associated ROOT nodes from the main AST.
    // port: TranspileAndOptimizeClosureUnaware#reattachShadowNodes
    fn reattach_shadow_nodes(&self, original: &mut AbstractCompiler, inputs: &ShadowAstsByFile) {
        for shadow_ast in inputs.iter().flat_map(|(_, asts)| asts) {
            let script = shadow_ast.script();
            check_state!(
                script.has_one_child(original),
                "{}",
                script.to_string_tree(original)
            );
            let detached = script.detach(original);
            shadow_ast
                .original_root()
                .add_child_to_front(original, detached);
            // Remove traces of the shadow compilation.
            script.set_input_id(original, None);
            script.set_static_source_file(original, None);
        }
    }

    // port: TranspileAndOptimizeClosureUnaware#setScriptFeaturesets
    fn set_script_featuresets(&self, original: &mut AbstractCompiler, inputs: &ShadowAstsByFile) {
        for shadow_ast in inputs.iter().flat_map(|(_, asts)| asts) {
            let shadow_script = shadow_ast.script();
            let mut collector = FeatureCollector::default();
            collector.visit_script(original, shadow_script);
            shadow_script.put_prop(
                original,
                Prop::FEATURE_SET,
                Some(ObjectProp::Opaque(Arc::new(collector.all_features()))),
            );
        }
    }

    // port: TranspileAndOptimizeClosureUnaware#createInjectRuntimeLibrariesPass
    fn create_inject_runtime_libraries_pass(
        &self,
        original: &AbstractCompiler,
        shadow_asts: &ShadowAstsByFile,
        main_compiler_queue: MainCompilerQueue,
    ) -> PassFactory {
        let mut call_sites: Vec<ClosureUnawareCallSite> = Vec::new();
        for shadow_ast in shadow_asts.iter().flat_map(|(_, asts)| asts) {
            let closure_unaware_function = NodeUtil::find_preorder(
                original,
                shadow_ast.script(),
                &|ast, n| n.is_function(ast),
                &|_, _child_to_visit| true,
            )
            .unwrap();
            call_sites.push(ClosureUnawareCallSite::new(
                original,
                shadow_ast.call_node(),
                closure_unaware_function,
            ));
        }
        PassFactory::builder()
            .set_name("injectClosureUnawareRuntimeLibs")
            .set_internal_factory(Arc::new(move |_compiler| {
                Box::new(InjectClosureUnawareRuntimeLibraries::new(
                    main_compiler_queue.clone(),
                    call_sites.clone(),
                ))
            }))
            .build()
    }
}

impl CompilerPass for TranspileAndOptimizeClosureUnaware {
    // port: TranspileAndOptimizeClosureUnaware#process
    fn process(&mut self, original: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut collector = CollectShadowAsts::default();
        NodeTraversal::traverse(original, root, &mut collector);
        if collector.shadow_asts.is_empty() {
            return;
        }

        self.set_script_featuresets(original, &collector.shadow_asts);
        let shadow_options = ClosureUnawareOptions::convert(original.get_options());
        let main_compiler_queue: MainCompilerQueue = Arc::new(Mutex::new(Vec::new()));
        let shadow_pass_config = RuntimeLibInjectionPassConfig::new(
            shadow_options.clone(),
            self.create_inject_runtime_libraries_pass(
                original,
                &collector.shadow_asts,
                main_compiler_queue.clone(),
            ),
        );
        let mut shadow_compiler =
            NestedCompilerRunner::create(shadow_options, self.mode, Box::new(shadow_pass_config));

        self.init_shadow_inputs(original, &mut shadow_compiler, &collector.shadow_asts);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            shadow_compiler.compile(original)
        }));
        // Rust-only: the main-compiler steps of InjectClosureUnawareRuntimeLibraries (see there).
        // Java performs them on the main compiler as the pass runs, so the steps queued before an
        // exception thrown in the shadow compilation (e.g. injectAll's rest-parameter check after
        // injectLibrariesIntoMainAst) are applied before it propagates.
        apply_main_compiler_steps(original, &main_compiler_queue);
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
        self.reattach_shadow_nodes(original, &collector.shadow_asts);
    }
}

// port: TranspileAndOptimizeClosureUnaware.ShadowAst
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShadowAst {
    original_root: NodeId,
    call_node: NodeId,
    script: NodeId,
}

impl ShadowAst {
    fn original_root(&self) -> NodeId {
        self.original_root
    }
    fn call_node(&self) -> NodeId {
        self.call_node
    }
    fn script(&self) -> NodeId {
        self.script
    }
}

/// Java's `LinkedHashMultimap<SourceFile, ShadowAst>`: keys by SourceFile identity (SourceFile
/// does not override equals), in insertion order; each key's values are a linked set.
type ShadowAstsByFile = Vec<(Arc<SourceFile>, Vec<ShadowAst>)>;

/// Traverses all closure unaware SCRIPTs to collect shadow ASTs.
#[derive(Default)]
struct CollectShadowAsts {
    shadow_asts: ShadowAstsByFile,
}

impl CollectShadowAsts {
    // port: LinkedHashMultimap#put
    fn put(&mut self, key: Arc<SourceFile>, value: ShadowAst) {
        if let Some((_, values)) = self
            .shadow_asts
            .iter_mut()
            .find(|(file, _)| Arc::ptr_eq(file, &key))
        {
            if !values.contains(&value) {
                values.push(value);
            }
        } else {
            self.shadow_asts.push((key, vec![value]));
        }
    }
}

impl Callback for CollectShadowAsts {
    // port: TranspileAndOptimizeClosureUnaware.CollectShadowAsts#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if !n.is_script(t) {
            return true;
        }
        if !n.is_closure_unaware_code(t) {
            return false;
        }
        let file_js_doc = n.get_jsdoc_info(t);
        let Some(minification_level) = file_js_doc
            .as_ref()
            .and_then(|info| info.get_per_file_closure_unaware_mode())
        else {
            // TODO: b/475302905 - remove this null check, once the JSC release to always
            // serialize a ClosureUnawareMode lands.
            return true;
        };
        match minification_level {
            PerFileClosureUnawareMode::UNSPECIFIED | PerFileClosureUnawareMode::SIMPLE => true,
            PerFileClosureUnawareMode::WHITESPACE => false,
        }
    }

    // port: TranspileAndOptimizeClosureUnaware.CollectShadowAsts#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let Some(shadow) = n.get_closure_unaware_shadow(t) else {
            return;
        };
        // CALL
        //   GETPROP .call
        //     NAME $jscomp_wrap_closure_unaware_code
        // NAME globalThis
        //   or
        // CALL
        //   GETPROP .call
        //     NAME $jscomp_wrap_closure_unaware_code
        // NAME undefined
        //   or
        // CALL
        //   NAME $jscomp_wrap_closure_unaware_code
        let call_node = if n.get_parent(t).unwrap().is_call(t) {
            n.get_parent(t).unwrap()
        } else {
            n.get_grandparent(t).unwrap()
        };
        check_state!(call_node.is_call(t), "{}", call_node.to_string(t));
        let script = shadow.get_only_child(t);
        let source_file = t.get_input().unwrap().get_source_file_arc();
        self.put(
            source_file,
            ShadowAst {
                original_root: shadow,
                call_node,
                script,
            },
        );
    }
}

struct RuntimeLibInjectionPassConfig {
    delegate: PassConfigDelegate,
    inject_runtime_libs: PassFactory,
}

impl RuntimeLibInjectionPassConfig {
    // port: TranspileAndOptimizeClosureUnaware.RuntimeLibInjectionPassConfig#RuntimeLibInjectionPassConfig
    fn new(options: CompilerOptions, inject_runtime_libs: PassFactory) -> Self {
        Self {
            delegate: PassConfigDelegate::new(Box::new(DefaultPassConfig::new(options))),
            inject_runtime_libs,
        }
    }
}

impl PassConfig for RuntimeLibInjectionPassConfig {
    fn get_options(&self) -> &CompilerOptions {
        self.delegate.get_options()
    }
    fn as_pass_config(&self) -> &dyn PassConfig {
        self
    }
    fn get_delegate(&self) -> Option<&dyn PassConfig> {
        self.delegate.get_delegate()
    }
    fn get_whitespace_only_passes(&self) -> PassListBuilder {
        self.delegate.get_whitespace_only_passes()
    }
    fn get_checks(&self) -> PassListBuilder {
        self.delegate.get_checks()
    }
    fn get_optimizations(&self) -> PassListBuilder {
        self.delegate.get_optimizations()
    }

    // port: TranspileAndOptimizeClosureUnaware.RuntimeLibInjectionPassConfig#getTranspileOnlyPasses
    fn get_transpile_only_passes(&self) -> PassListBuilder {
        let mut passes = self.delegate.get_transpile_only_passes();
        passes.add_after(
            self.inject_runtime_libs.clone(),
            pass_names::MARK_UNNORMALIZED,
        );
        passes
    }

    // port: TranspileAndOptimizeClosureUnaware.RuntimeLibInjectionPassConfig#getFinalizations
    fn get_finalizations(&self) -> PassListBuilder {
        let mut passes = self.delegate.get_finalizations();
        passes.add_after(
            self.inject_runtime_libs.clone(),
            pass_names::MARK_UNNORMALIZED,
        );
        passes
    }
}
