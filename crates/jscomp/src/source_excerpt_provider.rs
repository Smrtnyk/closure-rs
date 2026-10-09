/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SourceExcerptProvider.java.

use crate::{region::Region, sourcemap_mapping_placeholder::OriginalMapping};
pub trait SourceExcerptProvider: Send + Sync {
    // port: SourceExcerptProvider#getSourceLine
    fn get_source_line(&self, source_name: Option<&str>, line_number: i32) -> Option<String>;
    // port: SourceExcerptProvider#getSourceLines
    fn get_source_lines(
        &self,
        source_name: Option<&str>,
        line_number: i32,
        length: i32,
    ) -> Option<Box<dyn Region>>;
    // port: SourceExcerptProvider#getSourceRegion
    fn get_source_region(
        &self,
        source_name: Option<&str>,
        line_number: i32,
    ) -> Option<Box<dyn Region>>;
    // port: SourceExcerptProvider#getSourceMapping
    fn get_source_mapping(
        &self,
        source_name: Option<&str>,
        line_number: i32,
        column_number: i32,
    ) -> Option<OriginalMapping>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceExcerpt {
    LINE,
    FULL,
    REGION,
}
impl SourceExcerpt {
    // port: SourceExcerpt#get(SourceExcerptProvider,String,int,int,ExcerptFormatter)
    pub fn get(
        &self,
        source: &dyn SourceExcerptProvider,
        source_name: Option<&str>,
        line_number: i32,
        length: i32,
        formatter: &dyn ExcerptFormatter,
    ) -> Option<String> {
        match self {
            Self::LINE => formatter.format_line(
                source.get_source_line(source_name, line_number).as_deref(),
                line_number,
            ),
            Self::FULL => formatter.format_region(
                source
                    .get_source_lines(source_name, line_number, length)
                    .as_deref(),
            ),
            Self::REGION => formatter.format_region(
                source
                    .get_source_region(source_name, line_number)
                    .as_deref(),
            ),
        }
    }
    // port: SourceExcerpt#get(SourceExcerptProvider,String,int,ExcerptFormatter)
    pub fn get_without_length(
        &self,
        source: &dyn SourceExcerptProvider,
        source_name: Option<&str>,
        line_number: i32,
        formatter: &dyn ExcerptFormatter,
    ) -> Option<String> {
        self.get(source, source_name, line_number, -1, formatter)
    }
}
pub trait ExcerptFormatter {
    // port: ExcerptFormatter#formatLine
    fn format_line(&self, line: Option<&str>, line_number: i32) -> Option<String>;
    // port: ExcerptFormatter#formatRegion
    fn format_region(&self, region: Option<&dyn Region>) -> Option<String>;
}
