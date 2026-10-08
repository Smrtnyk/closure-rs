/*
 * Copyright (C) 2011 Google Inc.
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
// Ported from Gson 2.9.1 (https://github.com/google/gson):
//   com/google/gson/internal/LazilyParsedNumber.java.

use closure_rhino::js_string::JsString;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LazilyParsedNumber {
    pub value: JsString,
}
impl LazilyParsedNumber {
    // port: com.google.gson.internal.LazilyParsedNumber#LazilyParsedNumber
    pub fn new(value: impl Into<JsString>) -> Self {
        Self {
            value: value.into(),
        }
    }

    // port: com.google.gson.internal.LazilyParsedNumber#intValue
    pub fn int_value(&self) -> i32 {
        match closure_rhino::java_lang::parse_int(self.value.as_units(), 10) {
            Ok(value) => value,
            Err(_) => match crate::java_string::parse_long(&self.value) {
                Ok(value) => value as i32,
                Err(_) => crate::java_math::BigDecimal::new(&self.value).int_value(),
            },
        }
    }
}
impl std::fmt::Display for LazilyParsedNumber {
    // port: com.google.gson.internal.LazilyParsedNumber#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}
