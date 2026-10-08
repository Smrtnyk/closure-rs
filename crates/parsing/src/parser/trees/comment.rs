/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/trees/Comment.java.

use super::SourceRange;
use crate::parser::JsString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    // /* comment */
    BLOCK,
    // // comment
    LINE,
    // /** comment */
    JSDOC,
    // #!/usr/bin/node
    // Only valid at the start of a file.
    SHEBANG,
    // /*! comment */
    IMPORTANT,
}

/// placeholder class
#[derive(Clone, Debug)]
pub struct Comment {
    pub value: JsString,
    pub location: SourceRange,
    pub type_: Type,
}
impl Comment {
    // port: Comment#<init>
    pub fn new(value: JsString, location: SourceRange, type_: Type) -> Self {
        Self {
            value,
            location,
            type_,
        }
    }
    // port: Comment#isJsDoc
    pub fn is_js_doc(&self) -> bool {
        self.type_ == Type::JSDOC
    }
    // port: Comment#getAbsolutePosition
    pub fn get_absolute_position(&self) -> i32 {
        self.location.start.offset
    }
    // port: Comment#getLength
    pub fn get_length(&self) -> i32 {
        self.location.end.offset - self.location.start.offset
    }
}
