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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/JsonObject.java.

use super::JsonElement;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsonObject {
    pub members: IndexMap<JsString, JsonElement>,
}
impl JsonObject {
    // port: com.google.gson.JsonObject#JsonObject
    pub fn new() -> Self {
        Self {
            members: IndexMap::<_, _>::default(),
        }
    }

    // port: com.google.gson.JsonObject#add
    pub fn add(&mut self, name: impl Into<JsString>, value: JsonElement) {
        self.members.insert(name.into(), value);
    }
    // port: com.google.gson.JsonObject#has
    pub fn has(&self, name: impl Into<JsString>) -> bool {
        self.members.contains_key(&name.into())
    }
    // port: com.google.gson.JsonObject#get
    pub fn get(&self, name: impl Into<JsString>) -> Option<&JsonElement> {
        self.members.get(&name.into())
    }
    // port: com.google.gson.JsonObject#entrySet
    pub fn entry_set(&self) -> &IndexMap<JsString, JsonElement> {
        &self.members
    }
}
