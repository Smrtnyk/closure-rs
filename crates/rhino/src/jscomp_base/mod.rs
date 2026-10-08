/*
 * Copyright (C) 2007 The Guava Authors
 * Copyright (C) 2010 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/base/Preconditions.java, com/google/common/base/Strings.java.

//! com.google.javascript.jscomp.base. The eight base/format/*Exception classes are
//! not ported: nothing outside their own files references them (they serve J2CL
//! super-source only), so they are unreachable from CommandLineRunner.
pub mod js_comp_doubles;
pub mod js_comp_objects;
pub mod tri;
pub use js_comp_doubles::JSCompDoubles;
pub use js_comp_objects::JSCompObjects;
pub use tri::Tri;
// port: Strings#lenientFormat
pub fn guava_format(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut tail = template;
    let mut i = 0;
    while i < args.len() {
        if let Some(p) = tail.find("%s") {
            out.push_str(&tail[..p]);
            out.push_str(&args[i]);
            tail = &tail[p + 2..];
            i += 1;
        } else {
            break;
        }
    }
    out.push_str(tail);
    if i < args.len() {
        out.push_str(" [");
        out.push_str(&args[i..].join(", "));
        out.push(']');
    }
    out
}
// port: Preconditions#checkState
#[macro_export]
macro_rules! check_state {
    ($condition:expr $(,)?) => { assert!($condition, "") };
    ($condition:expr, $message:expr $(, $arg:expr)* $(,)?) => {
        if !$condition { panic!("{}", $crate::jscomp_base::guava_format($message, &[$($arg.to_string()),*])); }
    };
}
// port: Preconditions#checkArgument
#[macro_export]
macro_rules! check_argument {
    ($($args:tt)*) => { $crate::check_state!($($args)*) };
}
// port: Preconditions#checkNotNull
#[macro_export]
macro_rules! check_not_null {
    ($value:expr $(,)?) => { $value.expect("") };
    ($value:expr, $message:expr $(, $arg:expr)* $(,)?) => {
        $value.unwrap_or_else(|| panic!("{}", $crate::jscomp_base::guava_format($message, &[$($arg.to_string()),*])))
    };
}
pub use crate::{check_argument, check_not_null, check_state};
pub mod js_comp_strings;
pub mod linked_identity_hash_map;
pub mod linked_identity_hash_set;
pub use js_comp_strings::JSCompStrings;
pub use linked_identity_hash_map::{JavaIdentity, LinkedIdentityHashMap};
pub use linked_identity_hash_set::LinkedIdentityHashSet;
