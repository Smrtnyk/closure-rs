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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/JsonIOException.java.

use super::JsonParseException;
use closure_rhino::js_string::JsString;
pub struct JsonIOException;
impl JsonIOException {
    // port: com.google.gson.JsonIOException#JsonIOException(Throwable)
    pub fn from_cause(cause: impl Into<JsString>) -> JsonParseException {
        JsonParseException::new("com.google.gson.JsonIOException", cause)
    }

    // port: com.google.gson.JsonIOException#JsonIOException(String)
    pub fn from_message(message: impl Into<JsString>) -> JsonParseException {
        JsonParseException::new("com.google.gson.JsonIOException", message)
    }
}
