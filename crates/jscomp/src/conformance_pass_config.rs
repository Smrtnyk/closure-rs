/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ConformancePassConfig.java.

//! Port of `ConformancePassConfig.java`.

use crate::{
    compiler_options::CompilerOptions,
    pass_config::{PassConfig, PassConfigDelegate},
    pass_list_builder::PassListBuilder,
    pass_names,
};

/// Runs only the user-supplied conformance checks and any earlier passes required by conformance.
// port: ConformancePassConfig
pub struct ConformancePassConfig {
    base: PassConfigDelegate,
}

impl ConformancePassConfig {
    // port: ConformancePassConfig#ConformancePassConfig
    pub fn new(delegate: Box<dyn PassConfig>) -> Self {
        Self {
            base: PassConfigDelegate::new(delegate),
        }
    }

    /// Java's `delegate` field (the same object `PassConfigDelegate` keeps).
    fn delegate(&self) -> &dyn PassConfig {
        self.base.get_delegate().unwrap()
    }
}

impl PassConfig for ConformancePassConfig {
    fn get_options(&self) -> &CompilerOptions {
        self.base.get_options()
    }
    fn as_pass_config(&self) -> &dyn PassConfig {
        self
    }
    fn get_delegate(&self) -> Option<&dyn PassConfig> {
        self.base.get_delegate()
    }
    fn get_whitespace_only_passes(&self) -> PassListBuilder {
        self.base.get_whitespace_only_passes()
    }
    fn get_transpile_only_passes(&self) -> PassListBuilder {
        self.base.get_transpile_only_passes()
    }
    fn get_finalizations(&self) -> PassListBuilder {
        self.base.get_finalizations()
    }

    // port: ConformancePassConfig#getChecks
    fn get_checks(&self) -> PassListBuilder {
        let from_delegate = self.delegate().get_checks();

        // Return every check up to and including the "checkConformance" check. Return empty list
        // if "checkConformance" not found. This list may include some unnecessary checks that run
        // before conformance. However, there's no other reliable way to find a list of all the
        // passes that conformance depends on.
        let mut passes = PassListBuilder::new(self.get_options().clone());
        passes.add_all_up_to(&from_delegate, pass_names::CHECK_CONFORMANCE);
        passes
    }

    // port: ConformancePassConfig#getOptimizations
    fn get_optimizations(&self) -> PassListBuilder {
        PassListBuilder::new(self.get_options().clone())
    }
}
