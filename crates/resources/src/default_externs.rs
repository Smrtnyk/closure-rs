/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DefaultExterns.java.

//! Port of `com.google.javascript.jscomp.DefaultExterns`.

use indexmap::IndexMap;

use crate::compiler_options::Environment;

/// Contains information on default externs files. Provide [`DefaultExterns::prepare_externs`] to
/// filter externs source files.
pub struct DefaultExterns;

// Core language externs. When the environment is CUSTOM, only these externs will be included.
pub const BUILTIN_LANG_EXTERNS: [&str; 5] = [
    "es3.js",
    "es5.js",
    "es6.js",
    "es6_typed_array.js",
    "es6_collections.js",
];

// Ordered browser externs. Externs not included in this list are added last.
pub const BROWSER_EXTERN_DEP_ORDER: [&str; 23] = [
    "intl.js",
    "w3c_event.js",
    "w3c_event3.js",
    "gecko_event.js",
    "ie_event.js",
    "webkit_event.js",
    "w3c_device_sensor_event.js",
    "w3c_dom1.js",
    "w3c_dom2.js",
    "w3c_dom3.js",
    "w3c_dom4.js",
    "gecko_dom.js",
    "ie_dom.js",
    "webkit_dom.js",
    "w3c_css.js",
    "gecko_css.js",
    "ie_css.js",
    "webkit_css.js",
    "w3c_touch_event.js",
    "webstorage.js",
    "whatwg_console.js",
    "nonstandard_console.js",
    // w3d_rtc.js must be evaluated before nonstandard_rtc.js
    "w3c_rtc.js",
];

impl DefaultExterns {
    /// Filters and orders the passed externs for the specified environment.
    ///
    /// `externs` is the flat filename to source externs map (Java's `LinkedHashMap`). It will be
    /// modified: the entries taken in the fixed orders are removed, keeping the order of the rest
    /// (`shift_remove`, like `LinkedHashMap#remove`). Returns the ordered list of externs.
    // port: DefaultExterns#prepareExterns
    pub fn prepare_externs<T: Clone>(
        env: Environment,
        externs: &mut IndexMap<String, T>,
    ) -> Vec<T> {
        let mut out: Vec<T> = Vec::new();

        for key in BUILTIN_LANG_EXTERNS {
            assert!(
                externs.contains_key(key),
                "Externs must contain builtin: {key}"
            );
            out.push(externs.shift_remove(key).unwrap());
        }

        if env == Environment::BROWSER {
            for key in BROWSER_EXTERN_DEP_ORDER {
                assert!(
                    externs.contains_key(key),
                    "Externs must contain builtin for env {env}: {key}"
                );
                out.push(externs.shift_remove(key).unwrap());
            }

            out.extend(externs.values().cloned());
        }

        out
    }
}
