/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/SortedDependencies.java.

use super::{dependency_info::DependencyInfo, module_names::ModuleNames};
use indexmap::{IndexMap, IndexSet};
use std::{collections::VecDeque, fmt, hash::Hash};
pub struct SortedDependencies<InputT: DependencyInfo + Clone + Eq + Hash> {
    user_ordered_inputs: Vec<InputT>,
    import_ordered_inputs: Vec<InputT>,
    completed_inputs: IndexSet<InputT>,
    non_exporting_inputs: IndexMap<String, InputT>,
    exporting_input_by_symbol_name: IndexMap<String, InputT>,
    imported_input_by_importing_input: IndexMap<InputT, IndexSet<InputT>>,
}
impl<InputT: DependencyInfo + Clone + Eq + Hash> SortedDependencies<InputT> {
    // port: SortedDependencies#SortedDependencies
    pub fn new(user_ordered_inputs: Vec<InputT>) -> Self {
        let mut result = Self {
            user_ordered_inputs,
            import_ordered_inputs: vec![],
            completed_inputs: IndexSet::new(),
            non_exporting_inputs: IndexMap::new(),
            exporting_input_by_symbol_name: IndexMap::new(),
            imported_input_by_importing_input: IndexMap::new(),
        };
        result.process_inputs();
        result
    }
    // port: SortedDependencies#getStrongDependenciesOf
    pub fn get_strong_dependencies_of(&self, root_inputs: &[InputT], sorted: bool) -> Vec<InputT> {
        let mut included_inputs = IndexSet::new();
        let mut worklist: VecDeque<InputT> = root_inputs.iter().cloned().collect();
        while let Some(input) = worklist.pop_front() {
            if included_inputs.insert(input.clone()) {
                for symbol_name in input.get_required_symbols() {
                    if let Some(imported_symbol_name) =
                        self.exporting_input_by_symbol_name.get(&symbol_name)
                    {
                        worklist.push_back(imported_symbol_name.clone());
                    }
                }
            }
        }
        (if sorted {
            &self.import_ordered_inputs
        } else {
            &self.user_ordered_inputs
        })
        .iter()
        .filter(|i| included_inputs.contains(*i))
        .cloned()
        .collect()
    }
    // port: SortedDependencies#getInputProviding
    pub fn get_input_providing(
        &self,
        symbol_name: &str,
    ) -> Result<InputT, MissingProvideException> {
        self.maybe_get_input_providing(symbol_name)
            .cloned()
            .ok_or_else(|| MissingProvideException::new(symbol_name))
    }
    // port: SortedDependencies#getInputsWithoutProvides
    pub fn get_inputs_without_provides(&self) -> Vec<InputT> {
        self.non_exporting_inputs.values().cloned().collect()
    }
    // port: SortedDependencies#getSortedStrongDependenciesOf
    pub fn get_sorted_strong_dependencies_of(&self, roots: &[InputT]) -> Vec<InputT> {
        self.get_strong_dependencies_of(roots, true)
    }
    // port: SortedDependencies#getSortedWeakDependenciesOf
    pub fn get_sorted_weak_dependencies_of(&self, root_inputs: &[InputT]) -> Vec<InputT> {
        let strong_inputs: IndexSet<InputT> = self
            .get_sorted_strong_dependencies_of(root_inputs)
            .into_iter()
            .collect();
        let mut weak_inputs = IndexSet::new();
        let mut worklist: VecDeque<InputT> = strong_inputs.iter().cloned().collect();
        while let Some(input) = worklist.pop_front() {
            let is_strong = strong_inputs.contains(&input);
            let edges = if is_strong {
                input.get_type_requires().to_vec()
            } else {
                input
                    .get_required_symbols()
                    .into_iter()
                    .chain(input.get_type_requires().iter().cloned())
                    .collect()
            };
            if !is_strong && !weak_inputs.insert(input) {
                continue;
            }
            for symbol_name in edges {
                if let Some(imported_symbol_name) =
                    self.exporting_input_by_symbol_name.get(&symbol_name)
                    && !strong_inputs.contains(imported_symbol_name)
                    && !weak_inputs.contains(imported_symbol_name)
                {
                    worklist.push_back(imported_symbol_name.clone());
                }
            }
        }
        self.import_ordered_inputs
            .iter()
            .filter(|i| weak_inputs.contains(*i))
            .cloned()
            .collect()
    }
    // port: SortedDependencies#getSortedList
    pub fn get_sorted_list(&self) -> &[InputT] {
        &self.import_ordered_inputs
    }
    // port: SortedDependencies#maybeGetInputProviding
    pub fn maybe_get_input_providing(&self, symbol: &str) -> Option<&InputT> {
        if let Some(input) = self.exporting_input_by_symbol_name.get(symbol) {
            return Some(input);
        }
        self.non_exporting_inputs
            .get(&ModuleNames::file_to_module_name(symbol))
    }
    // port: SortedDependencies#orderInput
    fn order_input(&mut self, input: InputT) {
        if self.completed_inputs.contains(&input) {
            return;
        }
        self.completed_inputs.insert(input.clone());
        if let Some(imports) = self.imported_input_by_importing_input.get(&input).cloned() {
            for imported_input in imports {
                self.order_input(imported_input);
            }
        }
        self.import_ordered_inputs.push(input);
    }
    // port: SortedDependencies#processInputs
    fn process_inputs(&mut self) {
        for user_ordered_input in &self.user_ordered_inputs {
            let provides = user_ordered_input.get_provides();
            let first_provide = provides.first();
            if first_provide.is_none()
                || provides.len() == 1
                    && first_provide.unwrap().starts_with("module$")
                    && !user_ordered_input.is_es6_module()
            {
                self.non_exporting_inputs.insert(
                    ModuleNames::file_to_module_name(user_ordered_input.get_name()),
                    user_ordered_input.clone(),
                );
            }
            for provided_symbol_name in provides {
                self.exporting_input_by_symbol_name
                    .insert(provided_symbol_name.clone(), user_ordered_input.clone());
            }
        }
        for user_ordered_input in &self.user_ordered_inputs {
            for symbol_name in user_ordered_input.get_required_symbols() {
                if let Some(imported_input) = self.exporting_input_by_symbol_name.get(&symbol_name)
                {
                    self.imported_input_by_importing_input
                        .entry(user_ordered_input.clone())
                        .or_default()
                        .insert(imported_input.clone());
                }
            }
        }
        for user_ordered_input in self.user_ordered_inputs.clone() {
            self.order_input(user_ordered_input);
        }
        self.completed_inputs.clear();
        self.imported_input_by_importing_input.clear();
    }
}
#[derive(Debug)]
pub struct MissingProvideException {
    pub provide: String,
    pub cause: Option<Box<dyn std::error::Error + Send + Sync>>,
}
impl MissingProvideException {
    // port: SortedDependencies.MissingProvideException#MissingProvideException(String)
    pub fn new(provide: &str) -> Self {
        Self {
            provide: provide.into(),
            cause: None,
        }
    }
    // port: SortedDependencies.MissingProvideException#MissingProvideException(String,Exception)
    pub fn with_cause(provide: &str, cause: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            provide: provide.into(),
            cause: Some(cause),
        }
    }
}
impl fmt::Display for MissingProvideException {
    // port: Throwable#getMessage
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.provide)
    }
}
impl std::error::Error for MissingProvideException {}
#[cfg(test)]
mod tests;
