/*
 * Copyright (C) 2008 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/JsonArray.java.

use super::JsonElement;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsonArray {
    pub elements: Vec<JsonElement>,
}
impl JsonArray {
    // port: com.google.gson.JsonArray#JsonArray
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }
    // port: com.google.gson.JsonArray#getAsString
    pub fn get_as_string(&self) -> closure_rhino::js_string::JsString {
        if self.elements.len() == 1 {
            return self.elements[0].get_as_string();
        }
        panic!("java.lang.IllegalStateException");
    }
    // port: com.google.gson.JsonArray#getAsInt
    pub fn get_as_int(&self) -> i32 {
        if self.elements.len() == 1 {
            return self.elements[0].get_as_int();
        }
        panic!("java.lang.IllegalStateException");
    }

    // port: com.google.gson.JsonArray#add
    pub fn add(&mut self, value: JsonElement) {
        self.elements.push(value);
    }
    // port: com.google.gson.JsonArray#size
    pub fn size(&self) -> usize {
        self.elements.len()
    }
    // port: com.google.gson.JsonArray#get
    pub fn get(&self, index: usize) -> &JsonElement {
        &self.elements[index]
    }
}
