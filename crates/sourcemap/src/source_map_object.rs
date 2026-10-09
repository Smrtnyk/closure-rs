/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/SourceMapObject.java.

use crate::{source_map_generator_v3::ExtensionValue, source_map_section::SourceMapSection};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;
#[derive(Clone, Debug, Default)]
pub struct SourceMapObject {
    pub version: i32,
    pub line_count: i32,
    pub source_root: Option<JsString>,
    pub file: Option<JsString>,
    pub mappings: Option<JsString>,
    pub sources: Option<Vec<Option<JsString>>>,
    pub sources_content: Option<Vec<Option<JsString>>>,
    pub names: Option<Vec<Option<JsString>>>,
    pub sections: Option<Vec<SourceMapSection>>,
    pub extensions: IndexMap<JsString, ExtensionValue>,
}
#[derive(Clone, Debug, Default)]
pub struct Builder {
    version: i32,
    line_count: i32,
    source_root: Option<JsString>,
    file: Option<JsString>,
    mappings: Option<JsString>,
    sources: Option<Vec<Option<JsString>>>,
    sources_content: Option<Vec<Option<JsString>>>,
    names: Option<Vec<Option<JsString>>>,
    sections: Option<Vec<SourceMapSection>>,
    extensions: IndexMap<JsString, ExtensionValue>,
}
impl SourceMapObject {
    // port: SourceMapObject#SourceMapObject
    #[allow(clippy::too_many_arguments)]
    fn new(
        version: i32,
        line_count: i32,
        source_root: Option<JsString>,
        file: Option<JsString>,
        mappings: Option<JsString>,
        sources: Option<Vec<Option<JsString>>>,
        sources_content: Option<Vec<Option<JsString>>>,
        names: Option<Vec<Option<JsString>>>,
        sections: Option<Vec<SourceMapSection>>,
        extensions: IndexMap<JsString, ExtensionValue>,
    ) -> Self {
        Self {
            version,
            line_count,
            source_root,
            file,
            mappings,
            sources,
            sources_content,
            names,
            sections,
            extensions,
        }
    }
    // port: SourceMapObject#builder
    pub fn builder() -> Builder {
        Builder::new()
    }
    // port: SourceMapObject#getVersion
    pub fn get_version(&self) -> i32 {
        self.version
    }
    // port: SourceMapObject#getLineCount
    pub fn get_line_count(&self) -> i32 {
        self.line_count
    }
    // port: SourceMapObject#getSourceRoot
    pub fn get_source_root(&self) -> Option<JsString> {
        self.source_root.clone()
    }
    // port: SourceMapObject#getFile
    pub fn get_file(&self) -> Option<JsString> {
        self.file.clone()
    }
    // port: SourceMapObject#getMappings
    pub fn get_mappings(&self) -> Option<JsString> {
        self.mappings.clone()
    }
    // port: SourceMapObject#getSources
    pub fn get_sources(&self) -> &Option<Vec<Option<JsString>>> {
        &self.sources
    }
    // port: SourceMapObject#getSourcesContent
    pub fn get_sources_content(&self) -> &Option<Vec<Option<JsString>>> {
        &self.sources_content
    }
    // port: SourceMapObject#getNames
    pub fn get_names(&self) -> &Option<Vec<Option<JsString>>> {
        &self.names
    }
    // port: SourceMapObject#getSections
    pub fn get_sections(&self) -> &Option<Vec<SourceMapSection>> {
        &self.sections
    }
    // port: SourceMapObject#getExtensions
    pub fn get_extensions(&self) -> &IndexMap<JsString, ExtensionValue> {
        &self.extensions
    }
}
impl Builder {
    // port: SourceMapObject.Builder#Builder
    fn new() -> Self {
        Self::default()
    }

    // port: SourceMapObject.Builder#setVersion
    pub fn set_version(&mut self, version: i32) -> &mut Self {
        self.version = version;
        self
    }
    // port: SourceMapObject.Builder#setLineCount
    pub fn set_line_count(&mut self, line_count: i32) -> &mut Self {
        self.line_count = line_count;
        self
    }
    // port: SourceMapObject.Builder#setSourceRoot
    pub fn set_source_root(&mut self, source_root: Option<JsString>) -> &mut Self {
        self.source_root = source_root;
        self
    }
    // port: SourceMapObject.Builder#setFile
    pub fn set_file(&mut self, file: Option<JsString>) -> &mut Self {
        self.file = file;
        self
    }
    // port: SourceMapObject.Builder#setMappings
    pub fn set_mappings(&mut self, mappings: Option<JsString>) -> &mut Self {
        self.mappings = mappings;
        self
    }
    // port: SourceMapObject.Builder#setSources
    pub fn set_sources(&mut self, sources: Option<Vec<Option<JsString>>>) -> &mut Self {
        self.sources = sources;
        self
    }
    // port: SourceMapObject.Builder#setSourcesContent
    pub fn set_sources_content(
        &mut self,
        sources_content: Option<Vec<Option<JsString>>>,
    ) -> &mut Self {
        self.sources_content = sources_content;
        self
    }
    // port: SourceMapObject.Builder#setNames
    pub fn set_names(&mut self, names: Option<Vec<Option<JsString>>>) -> &mut Self {
        self.names = names;
        self
    }
    // port: SourceMapObject.Builder#setSections
    pub fn set_sections(&mut self, sections: Option<Vec<SourceMapSection>>) -> &mut Self {
        self.sections = sections;
        self
    }
    // port: SourceMapObject.Builder#setExtensions
    pub fn set_extensions(&mut self, extensions: IndexMap<JsString, ExtensionValue>) -> &mut Self {
        self.extensions = extensions;
        self
    }
    // port: SourceMapObject.Builder#build
    pub fn build(&self) -> SourceMapObject {
        SourceMapObject::new(
            self.version,
            self.line_count,
            self.source_root.clone(),
            self.file.clone(),
            self.mappings.clone(),
            self.sources.clone(),
            self.sources_content.clone(),
            self.names.clone(),
            self.sections.clone(),
            self.extensions.clone(),
        )
    }
}
