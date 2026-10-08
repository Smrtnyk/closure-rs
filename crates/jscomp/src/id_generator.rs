/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/IdGenerator.java.

//! Port of `IdGenerator.java`: a unique id generator for cross-chunk method stubbing.

/// An object that generates unique ids for cross-chunk method stubs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdGenerator {
    /// Ids for cross-chunk method stubbing, so that each method has a unique id.
    current_id: i32,
}

impl IdGenerator {
    /// Returns whether we've generated any new ids.
    // port: IdGenerator#hasGeneratedAnyIds
    pub fn has_generated_any_ids(&self) -> bool {
        self.current_id != 0
    }

    /// Creates a new id for stubbing a method.
    // port: IdGenerator#newId
    pub fn new_id(&mut self) -> i32 {
        let id = self.current_id;
        self.current_id = self.current_id.wrapping_add(1);
        id
    }

    // port: IdGenerator#getCurrentId
    pub fn get_current_id(&self) -> i32 {
        self.current_id
    }

    // port: IdGenerator#setCurrentId
    pub fn set_current_id(&mut self, id: i32) {
        self.current_id = id;
    }
}
