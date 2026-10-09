/*
 * Copyright 2022 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PassListBuilder.java.

use crate::{compiler_options::CompilerOptions, pass_factory::PassFactory};
pub struct PassListBuilder {
    options: CompilerOptions,
    passes: Vec<PassFactory>,
}
impl PassListBuilder {
    // port: PassListBuilder#PassListBuilder
    pub fn new(options: CompilerOptions) -> Self {
        Self {
            options,
            passes: vec![],
        }
    }
    // port: PassListBuilder#build
    pub fn build(&self) -> Vec<PassFactory> {
        self.passes
            .iter()
            .filter(|p| p.is_ported())
            .cloned()
            .collect()
    }
    pub fn build_including_unported(&self) -> Vec<PassFactory> {
        self.passes.clone()
    }
    // port: PassListBuilder#addAll
    pub fn add_all(&mut self, other: &Self) {
        self.passes.extend(other.build_including_unported());
    }
    // port: PassListBuilder#maybeAdd
    pub fn maybe_add(&mut self, factory: PassFactory) {
        if factory.get_condition()(&self.options) {
            self.passes.push(factory);
        }
    }
    // port: PassListBuilder#addBefore
    pub fn add_before(&mut self, factory: PassFactory, pass_name: &str) {
        let index = self.find_index_by_name(pass_name);
        self.passes.insert(index, factory);
    }
    // port: PassListBuilder#addAfter
    pub fn add_after(&mut self, factory: PassFactory, pass_name: &str) {
        let index = self.find_index_by_name(pass_name);
        self.passes.insert(index + 1, factory);
    }
    // port: PassListBuilder#addAllUpTo
    pub fn add_all_up_to(&mut self, other: &Self, pass_name: &str) {
        let passes = other.build_including_unported();
        if let Some(index) = passes.iter().position(|p| p.get_name() == pass_name) {
            self.passes.extend_from_slice(&passes[..=index]);
        }
    }
    // port: PassListBuilder#findByName
    pub fn find_by_name(&self, name: &str) -> &PassFactory {
        &self.passes[self.find_index_by_name(name)]
    }
    // port: PassListBuilder#removeByName
    pub fn remove_by_name(&mut self, name: &str) {
        if let Some(i) = self.passes.iter().position(|p| p.get_name() == name) {
            self.passes.remove(i);
        }
    }
    // port: PassListBuilder#findIndexByName
    fn find_index_by_name(&self, name: &str) -> usize {
        self.passes
            .iter()
            .position(|p| p.get_name() == name)
            .unwrap_or_else(|| panic!("No factory named '{name}' in the factory list"))
    }
    // port: PassListBuilder#contains
    pub fn contains(&self, factory: &PassFactory) -> bool {
        self.passes.contains(factory)
    }
    // port: PassListBuilder#assertAllOneTimePasses
    pub fn assert_all_one_time_passes(&self) {
        for pass in &self.passes {
            assert!(!pass.is_run_in_fixed_point_loop());
        }
    }
    // port: PassListBuilder#assertAllLoopablePasses
    pub fn assert_all_loopable_passes(&self) {
        for pass in &self.passes {
            assert!(pass.is_run_in_fixed_point_loop());
        }
    }
    // port: PassListBuilder#assertPassOrder
    pub fn assert_pass_order(&self, pass1: &PassFactory, pass2: &PassFactory, msg: &str) {
        if let (Some(a), Some(b)) = (
            self.passes.iter().position(|p| p == pass1),
            self.passes.iter().position(|p| p == pass2),
        ) {
            assert!(a < b, "{msg}");
        }
    }
}
