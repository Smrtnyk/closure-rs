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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/JsonParseException.java.

use closure_rhino::js_string::JsString;
#[derive(Clone, Debug)]
pub struct JsonParseException {
    pub class: &'static str,
    pub message: JsString,
}
impl JsonParseException {
    // port: com.google.gson.JsonParseException#JsonParseException
    pub fn new(class: &'static str, message: impl Into<JsString>) -> Self {
        let message = message.into();
        Self { class, message }
    }
    pub fn java_to_string(&self) -> JsString {
        crate::java_string::throwable_to_string(self.class, Some(&self.message))
    }
}
impl std::fmt::Display for JsonParseException {
    // Rust Display dispatch to the lossless inherited Throwable body.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.java_to_string())
    }
}
impl std::error::Error for JsonParseException {}
