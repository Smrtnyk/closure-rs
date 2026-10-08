/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PassConfig.java.

use crate::{
    compiler_options::CompilerOptions,
    graph::{graph::Graph, linked_directed_graph::LinkedDirectedGraph},
    pass_list_builder::PassListBuilder,
};
// port: PassConfig#PassConfig (options are owned by each implementor)
pub trait PassConfig: Send {
    fn get_options(&self) -> &CompilerOptions;
    fn as_pass_config(&self) -> &dyn PassConfig;
    fn get_delegate(&self) -> Option<&dyn PassConfig> {
        None
    }
    fn as_default_pass_config(&self) -> Option<&crate::default_pass_config::DefaultPassConfig> {
        None
    }
    // port: PassConfig#getWhitespaceOnlyPasses
    fn get_whitespace_only_passes(&self) -> PassListBuilder {
        PassListBuilder::new(self.get_options().clone())
    }
    // port: PassConfig#getTranspileOnlyPasses
    fn get_transpile_only_passes(&self) -> PassListBuilder {
        PassListBuilder::new(self.get_options().clone())
    }
    // port: PassConfig#getChecks
    fn get_checks(&self) -> PassListBuilder;
    // port: PassConfig#getOptimizations
    fn get_optimizations(&self) -> PassListBuilder;
    // port: PassConfig#getFinalizations
    fn get_finalizations(&self) -> PassListBuilder;
    // port: PassConfig#getPassGraph
    fn get_pass_graph(&self) -> LinkedDirectedGraph<String, String> {
        let mut graph = LinkedDirectedGraph::create_without_annotations();
        let all_passes = self
            .get_checks()
            .build()
            .into_iter()
            .chain(self.get_optimizations().build());
        let mut last_pass: Option<String> = None;
        let mut loop_start: Option<String> = None;
        for pass in all_passes {
            let mut pass_name = pass.get_name().to_owned();
            let mut i = 1;
            while graph.has_node(&pass_name) {
                pass_name = format!("{}{i}", pass.get_name());
                i += 1;
            }
            graph.create_node(pass_name.clone());
            if loop_start.is_none() && pass.is_run_in_fixed_point_loop() {
                loop_start = Some(pass_name.clone());
            } else if loop_start.is_some() && !pass.is_run_in_fixed_point_loop() {
                graph.connect(
                    last_pass.clone().unwrap(),
                    "loop".into(),
                    loop_start.take().unwrap(),
                );
            }
            if let Some(last) = last_pass {
                graph.connect(last, String::new(), pass_name.clone());
            }
            last_pass = Some(pass_name);
        }
        graph
    }
    // port: PassConfig#getBasePassConfig
    fn get_base_pass_config(&self) -> &dyn PassConfig {
        let mut current = self.as_pass_config();
        while let Some(delegate) = current.get_delegate() {
            current = delegate;
        }
        current
    }
}
pub struct PassConfigDelegate {
    delegate: Box<dyn PassConfig>,
    options: CompilerOptions,
}
impl PassConfigDelegate {
    // port: PassConfig.PassConfigDelegate#PassConfigDelegate
    pub fn new(delegate: Box<dyn PassConfig>) -> Self {
        Self {
            options: delegate.get_options().clone(),
            delegate,
        }
    }
}
impl PassConfig for PassConfigDelegate {
    fn get_options(&self) -> &CompilerOptions {
        &self.options
    }
    fn as_pass_config(&self) -> &dyn PassConfig {
        self
    }
    fn get_delegate(&self) -> Option<&dyn PassConfig> {
        Some(&*self.delegate)
    }
    // port: PassConfig.PassConfigDelegate#getWhitespaceOnlyPasses
    fn get_whitespace_only_passes(&self) -> PassListBuilder {
        self.delegate.get_whitespace_only_passes()
    }
    // port: PassConfig.PassConfigDelegate#getChecks
    fn get_checks(&self) -> PassListBuilder {
        self.delegate.get_checks()
    }
    // port: PassConfig.PassConfigDelegate#getOptimizations
    fn get_optimizations(&self) -> PassListBuilder {
        self.delegate.get_optimizations()
    }
    // port: PassConfig.PassConfigDelegate#getFinalizations
    fn get_finalizations(&self) -> PassListBuilder {
        self.delegate.get_finalizations()
    }
    // port: PassConfig.PassConfigDelegate#getTranspileOnlyPasses
    fn get_transpile_only_passes(&self) -> PassListBuilder {
        self.delegate.get_transpile_only_passes()
    }
}
