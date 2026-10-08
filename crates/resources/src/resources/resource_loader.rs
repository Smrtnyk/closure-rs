/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/resources/ResourceLoader.java.

//! Port of `com.google.javascript.jscomp.resources.ResourceLoader`.
//!
//! `Class<?> clazz` is the class's fully qualified binary name (see [`crate::jar`]).

use indexmap::IndexMap;

use crate::jar;
use crate::resources::properties_parser::PropertiesParser;

/// Utility class that handles resource loading.
pub struct ResourceLoader;

impl ResourceLoader {
    // port: ResourceLoader#loadTextResource
    pub fn load_text_resource(clazz: &str, path: &str) -> String {
        // Java reads `clazz.getResourceAsStream(path)` and turns the NullPointerException of a
        // missing resource into "No such resource".
        match jar::get_resource_as_stream(clazz, path) {
            Some(stream) => jar::decode_utf8(stream.read_all_bytes()),
            None => {
                if !Self::resource_exists(clazz, path) {
                    panic!("No such resource: {path}");
                }
                panic!("java.lang.NullPointerException");
            }
        }
    }

    // port: ResourceLoader#loadPropertiesMap
    pub fn load_properties_map(clazz: &str, resource_name: &str) -> IndexMap<String, String> {
        PropertiesParser::parse(&Self::load_text_resource(clazz, resource_name))
    }

    // port: ResourceLoader#resourceExists
    pub fn resource_exists(clazz: &str, path: &str) -> bool {
        jar::get_resource(clazz, path).is_some()
    }
}
