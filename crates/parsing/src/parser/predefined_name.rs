/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/PredefinedName.java.

use super::JsString;
use std::sync::LazyLock;

static AS: LazyLock<JsString> = LazyLock::new(|| JsString::from("as"));
static FROM: LazyLock<JsString> = LazyLock::new(|| JsString::from("from"));
static GET: LazyLock<JsString> = LazyLock::new(|| JsString::from("get"));
static OF: LazyLock<JsString> = LazyLock::new(|| JsString::from("of"));
static SET: LazyLock<JsString> = LazyLock::new(|| JsString::from("set"));

/// The set of all non-keyword, non-reserved words used in javascript.
pub struct PredefinedName;
impl PredefinedName {
    // port: PredefinedName#<init>
    #[allow(dead_code)] // Java utility class constructor is private.
    fn new() -> Self {
        Self
    }
    pub const AS: &'static LazyLock<JsString> = &AS;
    pub const FROM: &'static LazyLock<JsString> = &FROM;
    pub const GET: &'static LazyLock<JsString> = &GET;
    pub const OF: &'static LazyLock<JsString> = &OF;
    pub const SET: &'static LazyLock<JsString> = &SET;
}
