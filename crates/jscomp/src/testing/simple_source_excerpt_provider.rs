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
//   src/com/google/javascript/jscomp/testing/SimpleSourceExcerptProvider.java.

use crate::{
    region::Region, source_excerpt_provider::SourceExcerptProvider, source_file::SourceFile,
    sourcemap_mapping_placeholder::OriginalMapping,
};
pub struct SimpleSourceExcerptProvider {
    source_file: SourceFile,
}
impl SimpleSourceExcerptProvider {
    // port: SimpleSourceExcerptProvider#SimpleSourceExcerptProvider
    pub fn new(source: &str) -> Self {
        Self {
            source_file: SourceFile::from_code("input", source),
        }
    }
}
impl SourceExcerptProvider for SimpleSourceExcerptProvider {
    // port: SimpleSourceExcerptProvider#getSourceLine
    fn get_source_line(&self, _name: Option<&str>, line: i32) -> Option<String> {
        self.source_file.get_line(line)
    }
    // port: SimpleSourceExcerptProvider#getSourceLines
    fn get_source_lines(
        &self,
        _name: Option<&str>,
        line: i32,
        length: i32,
    ) -> Option<Box<dyn Region>> {
        self.source_file
            .get_lines(line, length)
            .map(|r| Box::new(r) as Box<dyn Region>)
    }
    // port: SimpleSourceExcerptProvider#getSourceRegion
    fn get_source_region(&self, _name: Option<&str>, line: i32) -> Option<Box<dyn Region>> {
        self.source_file
            .get_region(line)
            .map(|r| Box::new(r) as Box<dyn Region>)
    }
    // port: SimpleSourceExcerptProvider#getSourceMapping
    fn get_source_mapping(
        &self,
        _name: Option<&str>,
        _line: i32,
        _column: i32,
    ) -> Option<OriginalMapping> {
        None
    }
}
