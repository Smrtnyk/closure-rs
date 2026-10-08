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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/JsonPrimitive.java.

use super::lazily_parsed_number::LazilyParsedNumber;
use closure_rhino::js_string::JsString;
#[derive(Clone, Debug)]
pub enum JsonPrimitive {
    String(JsString),
    Number(LazilyParsedNumber),
    Boolean(bool),
}
impl JsonPrimitive {
    // port: com.google.gson.JsonPrimitive#JsonPrimitive(Boolean)
    pub fn new_boolean(value: bool) -> Self {
        Self::Boolean(value)
    }
    // port: com.google.gson.JsonPrimitive#JsonPrimitive(Number)
    pub fn new_number(value: LazilyParsedNumber) -> Self {
        Self::Number(value)
    }

    // port: com.google.gson.JsonPrimitive#JsonPrimitive(String)
    pub fn new_string(s: impl Into<JsString>) -> Self {
        Self::String(s.into())
    }
    // port: com.google.gson.JsonPrimitive#isString
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }
    // port: com.google.gson.JsonPrimitive#isBoolean
    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Boolean(_))
    }
    // port: com.google.gson.JsonPrimitive#isNumber
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }
    // port: com.google.gson.JsonPrimitive#getAsBoolean
    pub fn get_as_boolean(&self) -> bool {
        if self.is_boolean() {
            let Self::Boolean(value) = self else {
                unreachable!()
            };
            *value
        } else {
            crate::java_string::parse_boolean(&self.get_as_string())
        }
    }

    // port: com.google.gson.JsonPrimitive#getAsNumber
    pub fn get_as_number(&self) -> LazilyParsedNumber {
        if let Self::String(value) = self {
            LazilyParsedNumber::new(value)
        } else if let Self::Number(value) = self {
            value.clone()
        } else {
            panic!(
                "java.lang.ClassCastException: class java.lang.Boolean cannot be cast to class java.lang.Number (java.lang.Boolean and java.lang.Number are in module java.base of loader 'bootstrap')"
            );
        }
    }
    // port: com.google.gson.JsonPrimitive#getAsString
    pub fn get_as_string(&self) -> JsString {
        if self.is_number() {
            self.get_as_number().value
        } else if self.is_boolean() {
            self.get_as_boolean().to_string().into()
        } else {
            let Self::String(value) = self else {
                unreachable!()
            };
            value.clone()
        }
    }
    // port: com.google.gson.JsonPrimitive#getAsInt
    pub fn get_as_int(&self) -> i32 {
        if self.is_number() {
            self.get_as_number().int_value()
        } else {
            let string = self.get_as_string();
            closure_rhino::java_lang::parse_int(string.as_units(), 10).unwrap_or_else(|_| {
                crate::JavaException::throw(
                    "java.lang.NumberFormatException: For input string: \"",
                    &string,
                    "\"",
                )
            })
        }
    }
}

impl PartialEq for JsonPrimitive {
    // port: com.google.gson.JsonPrimitive#equals
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        // Rust's &Self argument and non-null enum variants enforce Java's
        // null/class/value guards at the type boundary.
        if self.is_number() && other.is_number() {
            let a = self
                .get_as_number()
                .value
                .to_string_lossy()
                .parse::<f64>()
                .unwrap_or(f64::NAN);
            let b = other
                .get_as_number()
                .value
                .to_string_lossy()
                .parse::<f64>()
                .unwrap_or(f64::NAN);
            return a == b || (a.is_nan() && b.is_nan());
        }
        // Java's isIntegral branch cannot be taken: LazilyParsedNumber is the
        // only Number implementation in this tree subset and is not integral.
        match (self, other) {
            (Self::String(value), Self::String(other_value)) => value == other_value,
            (Self::Boolean(value), Self::Boolean(other_value)) => value == other_value,
            _ => false,
        }
    }
}
impl Eq for JsonPrimitive {}
