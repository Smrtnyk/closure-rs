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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Result.java.

use crate::{js_error::JSError, source_map::SourceMap, variable_map::VariableMap};
use closure_rhino::fast_hash::IndexSet;
use std::sync::{Arc, Mutex};
pub struct Result {
    pub success: bool,
    pub errors: Vec<JSError>,
    pub warnings: Vec<JSError>,
    pub variable_map: Option<Arc<VariableMap>>,
    pub property_map: Option<Arc<VariableMap>>,
    pub named_anon_function_map: Option<Arc<VariableMap>>,
    pub string_map: Option<Arc<VariableMap>>,
    pub instrumentation_mappings: Option<Arc<VariableMap>>,
    pub source_map: Option<Arc<Mutex<SourceMap>>>,
    pub css_names: Option<IndexSet<String>>,
    pub extern_export: Option<String>,
    pub id_generator_map: Option<String>,
    pub transpiled_files: bool,
}
impl Result {
    // port: Result#Result
    #[allow(clippy::too_many_arguments)] // Java's value constructor has twelve arguments.
    pub fn new(
        errors: Vec<JSError>,
        warnings: Vec<JSError>,
        variable_map: Option<Arc<VariableMap>>,
        property_map: Option<Arc<VariableMap>>,
        named_anon_function_map: Option<Arc<VariableMap>>,
        string_map: Option<Arc<VariableMap>>,
        instrumentation_mappings: Option<Arc<VariableMap>>,
        source_map: Option<Arc<Mutex<SourceMap>>>,
        extern_export: Option<String>,
        css_names: Option<IndexSet<String>>,
        id_generator_map: Option<String>,
        transpiled_files: bool,
    ) -> Self {
        Self {
            success: errors.is_empty(),
            errors,
            warnings,
            variable_map,
            property_map,
            named_anon_function_map,
            string_map,
            instrumentation_mappings,
            source_map,
            extern_export,
            css_names,
            id_generator_map,
            transpiled_files,
        }
    }

    // port: Result#Result(ImmutableList, ImmutableList, VariableMap, VariableMap, VariableMap, SourceMap, String)
    pub fn new_for_testing(
        errors: Vec<JSError>,
        warnings: Vec<JSError>,
        variable_map: Option<Arc<VariableMap>>,
        property_map: Option<Arc<VariableMap>>,
        named_anon_function_map: Option<Arc<VariableMap>>,
        source_map: Option<Arc<Mutex<SourceMap>>>,
        extern_export: Option<String>,
    ) -> Self {
        Self::new(
            errors,
            warnings,
            variable_map,
            property_map,
            named_anon_function_map,
            None,
            None,
            source_map,
            extern_export,
            None,
            None,
            false,
        )
    }

    // port: Result#pruneResultForPartialCompilation
    pub fn prune_result_for_partial_compilation(result: Result) -> Self {
        let empty_variable_map = Arc::new(VariableMap::from_map(
            &closure_rhino::fast_hash::IndexMap::<_, _>::default(),
        ));
        Self::new(
            result.errors,
            result.warnings,
            Some(empty_variable_map.clone()),
            Some(empty_variable_map.clone()),
            Some(empty_variable_map.clone()),
            None,
            Some(empty_variable_map),
            None,
            Some(String::new()),
            None,
            None,
            false,
        )
    }
}
