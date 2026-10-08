/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/debugging/sourcemap/SourceMapGeneratorV3Test.java,
//   test/com/google/debugging/sourcemap/TestJsonBuilder.java.

#![allow(dead_code)]
use closure_sourcemap::gson::{Gson, JsonArray, JsonElement, JsonObject, JsonPrimitive, Target};
#[derive(Default)]
pub struct TestJsonBuilder {
    internal: JsonObject,
    sections: JsonArray,
}
impl TestJsonBuilder {
    // port: TestJsonBuilder#create
    pub fn create() -> Self {
        Self::new()
    }
    // port: TestJsonBuilder#TestJsonBuilder
    fn new() -> Self {
        Self {
            internal: JsonObject::new(),
            sections: JsonArray::new(),
        }
    }
    // port: TestJsonBuilder#setVersion
    pub fn set_version(mut self, version: i32) -> Self {
        self.internal.add("version", number(version));
        self
    }
    // port: TestJsonBuilder#setFile
    pub fn set_file(mut self, file: &str) -> Self {
        self.internal.add("file", string(file));
        self
    }
    // port: TestJsonBuilder#setLineCount
    pub fn set_line_count(mut self, count: i32) -> Self {
        self.internal.add("lineCount", number(count));
        self
    }
    // port: TestJsonBuilder#setMappings
    pub fn set_mappings(mut self, mappings: &str) -> Self {
        self.internal.add("mappings", string(mappings));
        self
    }
    // port: TestJsonBuilder#setSourceRoot
    pub fn set_source_root(mut self, root: &str) -> Self {
        self.internal.add("sourceRoot", string(root));
        self
    }
    // port: TestJsonBuilder#setSources
    pub fn set_sources(mut self, sources: &[&str]) -> Self {
        self.internal.add("sources", array(sources));
        self
    }
    // port: TestJsonBuilder#setSourcesContent
    pub fn set_sources_content(mut self, contents: &[&str]) -> Self {
        self.internal.add("sourcesContent", array(contents));
        self
    }
    // port: TestJsonBuilder#setNames
    pub fn set_names(mut self, names: &[&str]) -> Self {
        self.internal.add("names", array(names));
        self
    }
    // port: TestJsonBuilder#addSection
    pub fn add_section(mut self, line: i32, column: i32, map: TestJsonBuilder) -> Self {
        let _ = (line, column); // Java intentionally puts the literal offset 1,2.
        let mut offset = JsonObject::default();
        offset.add("line", number(1));
        offset.add("column", number(2));
        let mut section = JsonObject::default();
        section.add("offset", JsonElement::Object(offset));
        section.add("map", map.build());
        self.sections.add(JsonElement::Object(section));
        self.internal
            .add("sections", JsonElement::Array(self.sections.clone()));
        self
    }
    // port: TestJsonBuilder#setCustomProperty
    pub fn set_custom_property(mut self, name: &str, value: JsonElement) -> Self {
        self.internal.add(name, value);
        self
    }
    // port: TestJsonBuilder#build
    pub fn build(self) -> JsonElement {
        JsonElement::Object(self.internal)
    }
}
// Trivial conversion of the test builder's Java Object values to Gson trees.
pub fn number(v: i32) -> JsonElement {
    JsonElement::Primitive(JsonPrimitive::Number(
        closure_sourcemap::gson::lazily_parsed_number::LazilyParsedNumber::new(v.to_string()),
    ))
}
// Trivial string value conversion.
pub fn string(s: &str) -> JsonElement {
    JsonElement::Primitive(JsonPrimitive::new_string(s))
}
// Trivial String[] value conversion.
pub fn array(s: &[&str]) -> JsonElement {
    JsonElement::Array(JsonArray {
        elements: s.iter().map(|s| string(s)).collect(),
    })
}
// port: SourceMapGeneratorV3Test#parseJsonObject
pub fn parse_json_object(json: &str) -> JsonObject {
    Gson::new()
        .from_json(json, Target::JsonObject)
        .unwrap()
        .unwrap()
        .get_as_json_object()
        .clone()
}
pub mod source_map_test_case;
