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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/JsonElement.java.

use super::{JsonArray, JsonObject, JsonPrimitive};
use closure_rhino::js_string::JsString;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonElement {
    Object(JsonObject),
    Array(JsonArray),
    Primitive(JsonPrimitive),
    Null,
}
impl JsonElement {
    // port: com.google.gson.JsonElement#isJsonArray
    pub fn is_json_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }
    // port: com.google.gson.JsonElement#isJsonObject
    pub fn is_json_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    // port: com.google.gson.JsonElement#isJsonNull
    pub fn is_json_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    // port: com.google.gson.JsonElement#isJsonPrimitive
    pub fn is_json_primitive(&self) -> bool {
        matches!(self, Self::Primitive(_))
    }
    // port: com.google.gson.JsonElement#getAsJsonObject
    pub fn get_as_json_object(&self) -> &JsonObject {
        if let Self::Object(v) = self {
            v
        } else {
            crate::JavaException::throw(
                "java.lang.IllegalStateException: Not a JSON Object: ",
                &self.to_js_string(),
                "",
            );
        }
    }
    // port: com.google.gson.JsonElement#getAsJsonArray
    pub fn get_as_json_array(&self) -> &JsonArray {
        if let Self::Array(v) = self {
            v
        } else {
            crate::JavaException::throw(
                "java.lang.IllegalStateException: Not a JSON Array: ",
                &self.to_js_string(),
                "",
            );
        }
    }
    // port: com.google.gson.JsonElement#getAsJsonPrimitive
    pub fn get_as_json_primitive(&self) -> &JsonPrimitive {
        if let Self::Primitive(v) = self {
            v
        } else {
            crate::JavaException::throw(
                "java.lang.IllegalStateException: Not a JSON Primitive: ",
                &self.to_js_string(),
                "",
            );
        }
    }
    // Rust virtual dispatch to the subclass method, or the Java base body.
    pub fn get_as_string(&self) -> JsString {
        match self {
            Self::Primitive(v) => v.get_as_string(),
            Self::Array(v) => v.get_as_string(),
            _ => self.get_as_string_base(),
        }
    }
    // port: com.google.gson.JsonElement#toString
    // Lossless body used by Display and Java callers before a scalar Rust boundary.
    pub fn to_js_string(&self) -> JsString {
        let mut string_writer = super::stream::json_writer::JsonWriter::new(Vec::new());
        string_writer.set_lenient(true);
        string_writer.write(self);
        string_writer.into_string()
    }
    // Rust virtual dispatch to the subclass method, or the Java base body.
    pub fn get_as_int(&self) -> i32 {
        match self {
            Self::Primitive(v) => v.get_as_int(),
            Self::Array(v) => v.get_as_int(),
            _ => self.get_as_int_base(),
        }
    }
    // port: com.google.gson.JsonElement#getAsString
    fn get_as_string_base(&self) -> JsString {
        panic!(
            "java.lang.UnsupportedOperationException: {}",
            self.class_name().rsplit('.').next().unwrap()
        );
    }
    // port: com.google.gson.JsonElement#getAsInt
    fn get_as_int_base(&self) -> i32 {
        panic!(
            "java.lang.UnsupportedOperationException: {}",
            self.class_name().rsplit('.').next().unwrap()
        );
    }
    // Rust enum dispatch bridge for Java class names.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Object(_) => "com.google.gson.JsonObject",
            Self::Array(_) => "com.google.gson.JsonArray",
            Self::Primitive(_) => "com.google.gson.JsonPrimitive",
            Self::Null => "com.google.gson.JsonNull",
        }
    }
}
impl std::fmt::Display for JsonElement {
    // Rust Display dispatch to the single lossless Java method body.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_js_string())
    }
}
