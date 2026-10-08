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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/SimpleRegion.java.

use crate::region::Region;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleRegion {
    beginning_line_number: i32,
    ending_line_number: i32,
    source: String,
}
impl SimpleRegion {
    // port: SimpleRegion#SimpleRegion
    pub fn new(
        beginning_line_number: i32,
        ending_line_number: i32,
        source: impl Into<String>,
    ) -> Self {
        Self {
            beginning_line_number,
            ending_line_number,
            source: source.into(),
        }
    }
}
impl Region for SimpleRegion {
    // port: SimpleRegion#getBeginningLineNumber
    fn get_beginning_line_number(&self) -> i32 {
        self.beginning_line_number
    }
    // port: SimpleRegion#getEndingLineNumber
    fn get_ending_line_number(&self) -> i32 {
        self.ending_line_number
    }
    // port: SimpleRegion#getSourceExcerpt
    fn get_source_excerpt(&self) -> &str {
        &self.source
    }
}
