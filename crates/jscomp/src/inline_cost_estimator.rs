/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InlineCostEstimator.java.

//! Port of `InlineCostEstimator.java`: for use with CodeGenerator to determine the cost of
//! generated code.

use crate::{
    code_consumer::{CodeConsumer, CodeConsumerState},
    code_generator::CodeGenerator,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};

// For now simply assume identifiers are 2 characters.
const ESTIMATED_IDENTIFIER: &str = "ab";
// port: InlineCostEstimator#ESTIMATED_IDENTIFIER_COST
pub const ESTIMATED_IDENTIFIER_COST: i32 = ESTIMATED_IDENTIFIER.len() as i32;

// port: InlineCostEstimator
pub struct InlineCostEstimator;

impl InlineCostEstimator {
    /// Determines the size of the JS code.
    // port: InlineCostEstimator#getCost(Node)
    pub fn get_cost(ast: &Ast, root: NodeId) -> i32 {
        Self::get_cost_with_threshold(ast, root, i32::MAX)
    }

    /// Determines the estimated size of the JS snippet represented by the node.
    // port: InlineCostEstimator#getCost(Node,int)
    pub fn get_cost_with_threshold(ast: &Ast, root: NodeId, cost_threshold: i32) -> i32 {
        let mut estimator = CompiledSizeEstimator::new(cost_threshold);
        estimator.add_root(ast, root);
        estimator.get_cost()
    }
}

/// Code consumer that estimates compiled size by assuming names are shortened and all whitespace
/// is stripped.
// port: InlineCostEstimator.CompiledSizeEstimator
struct CompiledSizeEstimator {
    state: CodeConsumerState,
    max_cost: i32,
    cost: i32,
    last: u16,
    continue_processing: bool,
}

impl CompiledSizeEstimator {
    // port: InlineCostEstimator.CompiledSizeEstimator#CompiledSizeEstimator
    fn new(cost_threshold: i32) -> Self {
        Self {
            state: CodeConsumerState::default(),
            max_cost: cost_threshold,
            cost: 0,
            last: 0,
            continue_processing: true,
        }
    }

    // port: InlineCostEstimator.CompiledSizeEstimator#add
    fn add_root(&mut self, ast: &Ast, root: NodeId) {
        let mut cg = CodeGenerator::for_cost_estimation(self);
        cg.add_node(ast, root);
    }

    // port: InlineCostEstimator.CompiledSizeEstimator#getCost
    fn get_cost(&self) -> i32 {
        self.cost
    }
}

impl CodeConsumer for CompiledSizeEstimator {
    fn state(&self) -> &CodeConsumerState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut CodeConsumerState {
        &mut self.state
    }

    // port: InlineCostEstimator.CompiledSizeEstimator#continueProcessing
    fn continue_processing(&self) -> bool {
        self.continue_processing
    }

    // port: InlineCostEstimator.CompiledSizeEstimator#getLastChar
    fn get_last_char(&self) -> u16 {
        self.last
    }

    // port: InlineCostEstimator.CompiledSizeEstimator#append
    fn append(&mut self, str: &JsString) {
        self.last = str.char_at(str.length() - 1);
        self.cost = self.cost.wrapping_add(str.length() as i32);
        if self.max_cost <= self.cost {
            self.continue_processing = false;
        }
    }

    // port: InlineCostEstimator.CompiledSizeEstimator#addIdentifier
    fn add_identifier(&mut self, _identifier: &JsString) {
        self.add(&ESTIMATED_IDENTIFIER.into());
    }

    /// Constants (true, false, null) are considered basically free, because it's likely that they
    /// will get folded when we're done.
    // port: InlineCostEstimator.CompiledSizeEstimator#addConstant
    fn add_constant(&mut self, _newcode: &JsString) {
        self.add(&"0".into());
    }
}
