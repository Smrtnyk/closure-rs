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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/debugging/sourcemap/FilePosition.java.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FilePosition {
    line: i32,
    column: i32,
}
impl FilePosition {
    // port: FilePosition#FilePosition
    pub const fn new(line: i32, column: i32) -> Self {
        Self { line, column }
    }
    // port: FilePosition#getLine
    pub const fn get_line(&self) -> i32 {
        self.line
    }
    // port: FilePosition#getColumn
    pub const fn get_column(&self) -> i32 {
        self.column
    }
}
impl std::fmt::Display for FilePosition {
    // port: FilePosition#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FilePosition{{line={}, column={}}}",
            self.line, self.column
        )
    }
}
